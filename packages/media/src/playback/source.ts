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
 * Worker ownership: the worker holds MediaBunny `Input` + `CanvasSink` off the
 * main thread and streams frames forward from the last jump. Steady playback
 * sends `playhead` (backpressure only); only a real jump sends `seek`.
 *
 * Frame ownership: `frameAt` returns a frame owned by the cache (upload to
 * WebGL, do NOT close it). The cache evicts LRU against a resolution-adaptive
 * byte budget.
 */

import { getFrameCache } from '../cache';
import { frameCacheCapBytes } from '../cache/frame-budget';
import { isUnsupportedContainer } from '../cache/unsupported-formats';
import type { CachedFrame } from '../cache/storage';
import { MediaError } from '../errors';
import type { FromMediabunnyWorker, ToMediabunnyWorker } from './worker';

/** Backwards tolerance before a request counts as a jump, not jitter. */
const FRAME_SLACK_SEC = 0.05;
/** Forward gap beyond which waiting for the run to arrive would stall. */
const JUMP_SEC = 0.5;

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
	/** Last time asked for, to tell steady playback from a jump. */
	#lastRequestSec = -1;
	#decodedFrames = 0;
	#servedFrames = 0;
	#missedFrames = 0;
	#maxLateMs = 0;
	#startedAtMs = 0;
	#loggedSeq = -1;

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
	/** Aggregate throughput for this source, emitted on `dispose()`. */
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
		this.#startedAtMs = performance.now();
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
		// Reject known-undecodable containers up front rather than spawning a
		// worker to discover it. The caller falls back to <video> either way.
		const ext = url.split('?')[0]?.split('.').pop() ?? '';
		if (ext && isUnsupportedContainer(ext)) {
			throw new MediaError('unsupported', `MediaBunny cannot decode .${ext} files`);
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
			// Frames are GPU surfaces; a cap that is safe at 1080p starves the
			// decoder's pool at 4K, which is the classic ~8fps stall.
			source.#cache.memoryCapBytes = frameCacheCapBytes(meta.width, meta.height);
			return source;
		} catch (err) {
			worker.terminate();
			throw err;
		}
	}

	/**
	 * Live throughput for this source. `avgFps` is decoded frames per second of
	 * wall time; `minFps` is the served/asked ratio scaled to the source rate,
	 * i.e. how often the render loop actually got a fresh picture.
	 */
	stats(): { avgFps: number; minFps: number; maxLateMs: number } {
		const elapsedSec = this.#startedAtMs
			? Math.max(0.001, (performance.now() - this.#startedAtMs) / 1000)
			: 0.001;
		const asked = this.#servedFrames + this.#missedFrames;
		const hitRate = asked > 0 ? this.#servedFrames / asked : 0;
		return {
			avgFps: this.#decodedFrames / elapsedSec,
			minFps: hitRate * this.fps,
			maxLateMs: this.#maxLateMs,
		};
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
			// Frames from a superseded run are still valid pictures for their own
			// timestamp, so cache them. Dropping late frames (the old behavior)
			// threw away work and starved the display.
			const frame = new VideoFrame(msg.canvas, {
				timestamp: Math.round(msg.originalSec * 1_000_000),
				duration: Math.round((1 / this.fps) * 1_000_000),
			});
			const tUs = Math.round(msg.originalSec * 1_000_000);
			// Persistent write is best-effort and off the hot path.
			this.#cache.write(tUs, frame as unknown as CachedFrame, true);
			this.#decodedFrames++;
			if (msg.seq === this.#inFlightSeq) this.#inFlightSeq = -1;
			// Log jumps only: the run streams every frame, so per-frame logging
			// would put 30-60 lines/sec into the dev console.
			if (DIAG && msg.seq !== this.#loggedSeq) {
				this.#loggedSeq = msg.seq;
				console.log(`[mb] run ${msg.seq} @ ${msg.originalSec.toFixed(3)}s (${msg.width}x${msg.height})`);
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
	 * Newest cached frame in `[floorUs, tUs]`. Frames before `floorUs` are in a
	 * removed cut and must not be shown, or the picture steps BACK into
	 * deleted content at every cut.
	 */
	#bestCached(tUs: number, floorUs: number): VideoFrame | null {
		const entry = this.#cache.readNearest(tUs, floorUs);
		return entry instanceof VideoFrame ? entry : null;
	}

	/**
	 * Frame to show at `originalSec` (original-recording clock), floored by
	 * `floorSec` (the start of the current kept segment).
	 *
	 * Returns `null` only until the first frame of a run decodes; after that
	 * the newest in-segment frame is always available. The returned frame is
	 * owned by the cache — upload it, do NOT close it.
	 */
	frameAt(originalSec: number, floorSec = 0): VideoFrame | null {
		if (this.#disposed) return null;
		const tUs = Math.max(0, Math.round(originalSec * 1e6));
		const floorUs = Math.max(0, Math.round(floorSec * 1e6));
		const cached = this.#bestCached(tUs, floorUs);

		// A jump is anything the running decode won't reach on its own: backwards,
		// or so far forward that waiting would stall the picture.
		const delta = originalSec - this.#lastRequestSec;
		const isJump =
			this.#lastRequestSec < 0 || delta < -FRAME_SLACK_SEC || delta > JUMP_SEC;
		this.#lastRequestSec = originalSec;

		if (isJump) {
			const seq = ++this.#seq;
			this.#inFlightSeq = seq;
			this.#post({ type: 'seek', seq, originalSec });
		} else {
			// Steady playback: just release the worker's backpressure. Posting a
			// seek here is what made every request abort the one before it.
			this.#post({ type: 'playhead', originalSec });
		}

		if (cached) {
			this.#servedFrames++;
			const lateMs = (tUs - (cached.timestamp ?? tUs)) / 1000;
			if (lateMs > this.#maxLateMs) this.#maxLateMs = lateMs;
			return cached;
		}
		this.#missedFrames++;
		return null;
	}

	/**
	 * Pre-decode the frame at `originalSec` without disturbing the active run,
	 * so the post-cut frame is warm when the playhead crosses the cut. The
	 * worker dedupes repeat requests for the same target.
	 */
	prefetch(originalSec: number): void {
		if (this.#disposed) return;
		this.#post({ type: 'prefetch', seq: ++this.#seq, originalSec });
	}

	dispose(): void {
		if (this.#disposed) return;
		this.#disposed = true;
		if (this.onStats) this.onStats(this.stats());
		this.#post({ type: 'dispose' });
		// Persistent cache survives on purpose: the next session should hit it.
		// Worker self-closes; terminate is the backstop.
		this.#worker.terminate();
	}
}
