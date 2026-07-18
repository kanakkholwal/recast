/**
 * MediabunnyVideoSource: frame-accurate video decode for the editor preview,
 * backed by MediaBunny's `Input` + `CanvasSink` running in a Web Worker.
 *
 * Mirrors the public surface of `WebCodecsVideoSource` (see
 * `webcodecs-source.ts`) so `VideoPreview.svelte` can swap sources by
 * switching the import + a feature flag without touching call sites. The
 * shape:
 *
 *   - `static create(url, sizeBytes?): Promise<MediabunnyVideoSource>`
 *   - `frameAt(originalSec, floorSec?): VideoFrame | null`
 *   - `prefetch(originalSec): void`
 *   - `dispose(): void`
 *   - readonly `width`, `height`, `durationSec`, `fps`, `ingestion`
 *   - `onFrame`, `onStats` callbacks
 *
 * PR-D landing strip: minimal working impl behind the `mbPreview` URL flag.
 *   - The worker holds MediaBunny `Input` + `CanvasSink` (off main thread).
 *   - The main thread caches `VideoFrame`s keyed by ctsUs (microseconds),
 *     mirroring the existing cache strategy. Cut-crossing math is
 *     identical.
 *   - Concurrency model: single in-flight seek at a time; new seeks
 *     supersede the previous one. PR-E layers an LRU cache.
 *
 * Frame ownership: `frameAt` returns a frame owned by the cache (same as
 * `WebCodecsVideoSource.frameAt`). Upload to WebGL, don't close it. The
 * cache evicts on `dispose()` and on resolution-aware eviction in PR-E.
 */

import type { FromMediabunnyWorker, ToMediabunnyWorker } from './mediabunny-worker';

/** Dev-only diagnostics (throughput + first-frame geometry). */
const DIAG = import.meta.env.DEV;

export class MediabunnyVideoSource {
	#worker: Worker;
	/** Decoded frames, keyed by ctsUs. */
	#cache = new Map<number, VideoFrame>();
	#disposed = false;
	/** Monotonic seq for outstanding seek requests so we can ignore stale frames. */
	#seq = 0;
	/** Outstanding seek seq; `-1` when no seek is in flight. */
	#inFlightSeq = -1;

	readonly width: number;
	readonly height: number;
	readonly durationSec: number;
	readonly fps: number;
	/**
	 * Ingestion strategy. MediaBunny handles whole-file and progressive
	 * (HTTP range) fetching transparently via its `UrlSource`; we don't
	 * distinguish at this layer. Reports `progressive` to match the
	 * WebCodecs source's semantic vocabulary for telemetry.
	 */
	readonly ingestion: 'whole' | 'progressive' = 'progressive';

	/** See {@link WebCodecsVideoSource.onFrame}. */
	onFrame: (() => void) | null = null;
	/**
	 * Aggregate throughput for this source, emitted on `dispose()`. The
	 * perf signal is intentionally a stand-in for PR-D (no eviction, no
	 * wall-clock tracking yet); PR-E wires this against real frame-budget
	 * data.
	 */
	onStats: ((s: { avgFps: number; minFps: number; maxLateMs: number }) => void) | null = null;

	private constructor(
		worker: Worker,
		meta: { width: number; height: number; durationSec: number; fps: number },
	) {
		this.#worker = worker;
		this.width = meta.width;
		this.height = meta.height;
		this.durationSec = meta.durationSec;
		this.fps = meta.fps;
		this.#worker.onmessage = (e: MessageEvent<FromMediabunnyWorker>) => this.#onMessage(e.data);
	}

	/**
	 * Spawn the MediaBunny decode worker, open `url`, and resolve once the
	 * source can answer `frameAt`. Rejects if the worker dies or the input is
	 * unreadable; the caller should fall back to the legacy
	 * `WebCodecsVideoSource` or the `<video>` element.
	 *
	 * `sizeBytes` is accepted for parity with `WebCodecsVideoSource.create`
	 * but ignored: MediaBunny's `UrlSource` streams range-requests natively,
	 * so no upfront ingestion decision is needed.
	 */
	static async create(url: string, _sizeBytes?: number): Promise<MediabunnyVideoSource> {
		if (typeof Worker === 'undefined' || typeof VideoFrame === 'undefined') {
			throw new Error('Worker/VideoFrame unavailable in this WebView');
		}
		const worker = new Worker(new URL('./mediabunny-worker.ts', import.meta.url), {
			type: 'module',
		});
		try {
			const meta = await new Promise<{
				width: number;
				height: number;
				durationSec: number;
				fps: number;
			}>((resolve, reject) => {
				worker.onmessage = (e: MessageEvent<FromMediabunnyWorker>) => {
					const msg = e.data;
					if (msg.type === 'ready') {
						resolve(msg);
					} else if (msg.type === 'error') {
						reject(new Error(msg.message));
					}
				};
				worker.onerror = (e) => reject(new Error(e.message || 'worker error'));
				const init: ToMediabunnyWorker = { type: 'init', url };
				worker.postMessage(init);
			});
			return new MediabunnyVideoSource(worker, meta);
		} catch (err) {
			worker.terminate();
			throw err;
		}
	}

	#post(msg: ToMediabunnyWorker): void {
		this.#worker.postMessage(msg);
	}

	#onMessage(msg: FromMediabunnyWorker): void {
		if (this.#disposed) {
			if (msg.type === 'frame') msg.canvas.width = 0; // hint GC; nothing else we can do.
			return;
		}
		if (msg.type === 'frame') {
			// Stale frame (a newer seek superseded us): drop on the floor.
			if (msg.seq !== this.#inFlightSeq) {
				return;
			}
			this.#inFlightSeq = -1;
			// Wrap the OffscreenCanvas in a VideoFrame so callers can use the
			// exact same upload path as the WebCodecs engine. `new VideoFrame(canvas)`
			// snapshots the canvas at construction time; the canvas itself can be
			// reused by the worker after this point.
			const frame = new VideoFrame(msg.canvas, {
				timestamp: Math.round(msg.originalSec * 1_000_000),
				duration: Math.round((1 / this.fps) * 1_000_000),
			});
			const tUs = Math.round(msg.originalSec * 1_000_000);
			// Evict any prior frame at the same timestamp (e.g. a re-seek to the
			// exact same time) so the cache doesn't leak VideoFrames.
			const prior = this.#cache.get(tUs);
			if (prior && prior !== frame) prior.close();
			this.#cache.set(tUs, frame);
			if (DIAG) {
				console.log(
					`[mb] frame @ ${msg.originalSec.toFixed(3)}s (${msg.width}x${msg.height}), cache=${this.#cache.size}`,
				);
			}
			// Paint, since the editor's rAF loop is the only thing that re-renders
			// during a pause, and a freshly-decoded seek target needs to repaint.
			this.onFrame?.();
			return;
		}
		if (msg.type === 'error') {
			console.warn('[mb] worker error:', msg.message);
			// Treat as fatal for this source; the caller will fall back to the
			// legacy path. PR-E wires this through the `MediaError` event surface.
		}
	}

	/**
	 * Best cached frame to show at `tUs`: the greatest timestamp in `[floorUs, tUs]`.
	 * Mirrors the legacy semantics: frames before `floorUs` are in a removed cut
	 * and must not be shown, or the picture steps BACK into deleted content.
	 */
	#bestCached(tUs: number, floorUs: number): VideoFrame | null {
		let best: VideoFrame | null = null;
		let bestTs = -Infinity;
		for (const [ts, frame] of this.#cache) {
			if (ts >= floorUs && ts <= tUs && ts > bestTs) {
				bestTs = ts;
				best = frame;
			}
		}
		return best;
	}

	/**
	 * Seek to `originalSec` (original-recording clock) and return the best
	 * cached frame at or before that timestamp, floored by `floorSec` (the
	 * start of the current kept segment; the editor's cut math computes this).
	 *
	 * Returns `null` when no in-segment frame is decoded yet; the editor
	 * holds the previous frame in that case so the preview is never blank.
	 *
	 * The returned frame is owned by the cache; upload it, do NOT close it.
	 */
	frameAt(originalSec: number, floorSec = 0): VideoFrame | null {
		if (this.#disposed) return null;
		const tUs = Math.max(0, Math.round(originalSec * 1e6));
		const floorUs = Math.max(0, Math.round(floorSec * 1e6));
		// Send a seek for the requested time. The worker supersedes prior
		// in-flight seeks, so we don't bother with an explicit cancel.
		const seq = ++this.#seq;
		this.#inFlightSeq = seq;
		this.#post({ type: 'seek', seq, originalSec });
		return this.#bestCached(tUs, floorUs);
	}

	/**
	 * Pre-decode the frame at `originalSec` without moving the playhead, so
	 * the post-cut frame is warm when the playhead crosses the cut. Mirrors
	 * the WebCodecs source's API.
	 */
	prefetch(originalSec: number): void {
		if (this.#disposed) return;
		this.#post({ type: 'prefetch', originalSec });
	}

	dispose(): void {
		if (this.#disposed) return;
		this.#disposed = true;
		// Best-effort aggregate for the perf signal; PR-E wires the real one.
		if (this.onStats) this.onStats({ avgFps: 0, minFps: 0, maxLateMs: 0 });
		this.#post({ type: 'dispose' });
		for (const frame of this.#cache.values()) frame.close();
		this.#cache.clear();
		// The worker self-closes on dispose; terminate as a backstop.
		this.#worker.terminate();
	}
}
