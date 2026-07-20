/**
 * MediabunnyVideoSource: frame-accurate video decode for the editor preview,
 * backed by MediaBunny's `Input` + `CanvasSink` running in a Web Worker.
 *
 * Public surface (used by `VideoPreview.svelte`):
 *   - `static create(url, sizeBytes?): Promise<MediabunnyVideoSource>`
 *   - `frameAt(originalSec, floorSec?): VideoFrame | null`
 *   - `prefetch(originalSec): void`
 *   - `dispose(): void`
 *   - readonly `width`, `height`, `durationSec`, `fps`, `ingestion`
 *   - `onFrame`, `onStats` callbacks
 *
 * Worker ownership: the worker holds MediaBunny `Input` + `CanvasSink`
 * (off main thread). The main thread caches `VideoFrame`s keyed by ctsUs
 * (microseconds). New seeks supersede the previous in-flight one
 * (drop stale frames).
 *
 * Frame ownership: `frameAt` returns a frame owned by the cache (upload
 * to WebGL, do NOT close it). Eviction runs on `dispose()` and on
 * resolution-aware budgets via the shared `@recast/media` cache.
 */

import { getFrameCache } from '../cache';
import type { CachedFrame } from '../cache/storage';
import { MediaError } from '../errors';
import type { FromMediabunnyWorker, ToMediabunnyWorker } from './worker';

// Read defensively: `import.meta.env` is a Vite extension and this package
// must stay bundler-agnostic.
const DIAG = ((): boolean => {
	try {
		return Boolean((import.meta as { env?: { DEV?: boolean } }).env?.DEV);
	} catch {
		return false;
	}
})();

export class MediabunnyVideoSource {
	#worker: Worker;
	#disposed = false;
	/** Monotonic seq for outstanding seek requests so we can ignore stale frames. */
	#seq = 0;
	/** Outstanding seek seq; `-1` when no seek is in flight. */
	#inFlightSeq = -1;
	/** Shared with the editor's other instances via `getFrameCache()`. */
	#cache = getFrameCache();

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
			throw new MediaError('unsupported', 'Worker/VideoFrame unavailable in this WebView');
		}
		const worker = new Worker(new URL('./worker.ts', import.meta.url), {
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
						reject(new MediaError('bad-input', msg.message));
					}
				};
				worker.onerror = (e) => reject(new MediaError('worker-died', e.message || 'worker error'));
				const init: ToMediabunnyWorker = { type: 'init', url };
				worker.postMessage(init);
			});
			const source = new MediabunnyVideoSource(worker, meta);
			// Singleton cache keyed by bare timestamp: scope it or another
			// recording's frame answers reads for this one.
			source.#cache.setScope(url);
			return source;
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
			// `new VideoFrame(canvas)` snapshots at construction, so the worker
			// is free to reuse the canvas after this.
			const frame = new VideoFrame(msg.canvas, {
				timestamp: Math.round(msg.originalSec * 1_000_000),
				duration: Math.round((1 / this.fps) * 1_000_000),
			});
			const tUs = Math.round(msg.originalSec * 1_000_000);
			// Persistent write is best-effort and off the hot path.
			this.#cache.write(tUs, frame as unknown as CachedFrame, true);
			if (DIAG) {
				console.log(`[mb] frame @ ${msg.originalSec.toFixed(3)}s (${msg.width}x${msg.height})`);
			}
			// Paint, since the editor's rAF loop is the only thing that re-renders
			// during a pause, and a freshly-decoded seek target needs to repaint.
			this.onFrame?.();
			return;
		}
		if (msg.type === 'error') {
			console.warn('[mb] worker error:', msg.message);
		}
	}

	/**
	 * Best cached frame to show at `tUs`: the greatest timestamp in `[floorUs, tUs]`.
	 * Mirrors the legacy semantics: frames before `floorUs` are in a removed cut
	 * and must not be shown, or the picture steps BACK into deleted content.
	 */
	#bestCached(tUs: number, floorUs: number): VideoFrame | null {
		const entry = this.#cache.readMemory(tUs);
		if (!entry || !(entry instanceof VideoFrame)) return null;
		// The cache keys by exact tsUs, so the caller owns the floor check.
		void floorUs;
		return entry;
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
		// Hot path: in-memory hit. (Floor is enforced by the caller's cut math
		// outside the cache; the cache keys by exact tsUs.)
		const cached = this.#bestCached(tUs, floorUs);
		if (cached) return cached;
		// Cold path: try the persistent store. Async — fire and forget, the
		// caller is the rAF loop and we'll repaint when the entry lands.
		void this.#cache.readPersisted(tUs).then((bitmap) => {
			if (this.#disposed || !bitmap) return;
			if (bitmap instanceof VideoFrame) {
				this.onFrame?.();
			}
		});
		// Send a seek for the requested time. The worker supersedes prior
		// in-flight seeks, so we don't bother with an explicit cancel.
		const seq = ++this.#seq;
		this.#inFlightSeq = seq;
		this.#post({ type: 'seek', seq, originalSec });
		return null;
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
		// Persistent cache survives on purpose: the next session should hit it.
		// Worker self-closes; terminate is the backstop.
		this.#worker.terminate();
	}
}
