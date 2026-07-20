/**
 * MediabunnyVideoSource: frame-accurate video decode for the editor preview,
 * backed by MediaBunny's `Input` + `CanvasSink` running in a Web Worker.
 *
 * Public surface (used by `VideoPreview.svelte`):
 *   - `static create(url, { createWorker }): Promise<MediabunnyVideoSource>`
 *   - `frameAt(originalSec, floorSec?): VideoFrame | null`
 *   - `prefetch(originalSec): void`
 *   - `dispose(): void`
 *   - readonly `width`, `height`, `durationSec`, `fps`, `ingestion`
 *   - `onFrame`, `onStats` callbacks
 *
 * Worker ownership: the HOST APP spawns the worker and passes it in via
 * `createWorker`; this package only drives it. Spawning from here would mean a
 * `new URL('./worker.ts', import.meta.url)` pointing outside the app's root,
 * which every consuming app then has to whitelist in its bundler. The worker
 * body lives at `@recast/media/playback/worker` — see `startMediabunnyWorker`.
 * It holds MediaBunny `Input` + `CanvasSink` off the main thread and streams
 * frames forward from the last jump. Steady playback sends `playhead`
 * (backpressure only); only a real jump sends `seek`.
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
import { markNow, measureSince } from '../marks';
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

export interface MediabunnySourceOptions {
	/**
	 * Spawns the decode worker. The HOST APP owns this so the worker URL
	 * resolves against its own root — see the class docstring. Called only
	 * once `create` has decided the input is decodable.
	 */
	createWorker: () => Worker;
}

export class MediabunnyVideoSource {
	#worker: Worker;
	#disposed = false;
	/** Monotonic run id, echoed back on each frame; used for DIAG grouping. */
	#seq = 0;
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
	/** Start of the outstanding jump seek, for the seek-latency measure. */
	#seekStartedMs = 0;
	#sawFirstFrame = false;

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
	/**
	 * A decode run died after `create` resolved. The picture is now frozen at
	 * the last cached frame, so the consumer should fall back rather than sit
	 * on a still image.
	 */
	onError: ((err: MediaError) => void) | null = null;

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
	 * Open `url` and resolve once the source can answer `frameAt`. Rejects if
	 * the worker dies or the input is unreadable; the caller should fall back
	 * to the `<video>` element.
	 */
	static async create(
		url: string,
		options: MediabunnySourceOptions,
	): Promise<MediabunnyVideoSource> {
		if (typeof Worker === 'undefined' || typeof VideoFrame === 'undefined') {
			throw new MediaError('unsupported', 'Worker/VideoFrame unavailable in this WebView');
		}
		// Reject known-undecodable containers up front rather than spawning a
		// worker to discover it. The caller falls back to <video> either way.
		const ext = url.split('?')[0]?.split('.').pop() ?? '';
		if (ext && isUnsupportedContainer(ext)) {
			throw new MediaError('unsupported', `MediaBunny cannot decode .${ext} files`);
		}
		const worker = options.createWorker();
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
				// A worker that fails to LOAD fires onerror with an empty message,
				// so name the script — that distinguishes it from a throw inside.
				worker.onerror = (e) =>
					reject(
						new MediaError(
							'worker-died',
							e.message || `worker script failed to load: ${e.filename || 'unknown'}`,
						),
					);
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
			// `new VideoFrame(canvas)` copies the pixels out synchronously, so the
			// canvas backing store is dead weight from here — release it rather
			// than waiting on GC (~1.9 GB/s of garbage at 4K60).
			msg.canvas.width = 0;
			const tUs = Math.round(msg.originalSec * 1_000_000);
			// Memory only: a `VideoFrame` can't be structured-cloned into IndexedDB,
			// and the streaming decoder would attempt a write per frame. Re-decoding
			// is cheap now that the pipeline streams.
			this.#cache.write(tUs, frame as unknown as CachedFrame, false);
			this.#decodedFrames++;
			// §3 time-to-first-frame and scrub-seek rows, visible on the DevTools
			// timeline. Only the first frame of a run closes the seek measure.
			if (!this.#sawFirstFrame) {
				this.#sawFirstFrame = true;
				measureSince('time-to-first-frame', this.#startedAtMs, {
					width: this.width,
					height: this.height,
				});
			}
			if (this.#seekStartedMs > 0) {
				measureSince('seek-latency', this.#seekStartedMs, { seq: msg.seq });
				this.#seekStartedMs = 0;
			}
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
			// A dead run means a frozen picture, not a degraded one — never
			// downgrade this to a warning.
			console.error('[mb] decode run failed:', msg.code, msg.message);
			this.onError?.(new MediaError(msg.code, msg.message));
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
			this.#seekStartedMs = markNow();
			this.#post({ type: 'seek', seq: ++this.#seq, originalSec });
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
