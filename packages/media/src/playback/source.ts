/**
 * MediabunnyVideoSource: frame-accurate video decode for the editor preview,
 * backed by MediaBunny's `Input` + `VideoSampleSink` running in a Web Worker.
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
 * Frame ownership: set `onFrameDecoded` and each frame is handed to you and
 * closed immediately, so the decoder gets its output surface straight back.
 * Holding decoded frames is what starves the decoder into silence at 4K.
 * `frameAt` is the older cache-backed path, kept for consumers that want it.
 */

import { getFrameCache } from "../cache";
import { frameCacheCapBytes } from "../cache/frame-budget";
import { isUnsupportedContainer } from "../cache/unsupported-formats";
import type { CachedFrame } from "../cache/storage";
import { MediaError } from "../errors";
import { markNow, measureSince } from "../marks";
import { type MediaRef, mediaRefExtension, mediaRefKey, toMediaRef } from "../media-ref";
import type { FromMediabunnyWorker, ToMediabunnyWorker } from "./worker";

/** Backwards tolerance before a request counts as a jump, not jitter. */
const FRAME_SLACK_SEC = 0.05;
/** Forward gap beyond which waiting for the run to arrive would stall. */
const JUMP_SEC = 0.5;
/** Generous: demuxing a large file over the asset protocol is legitimately slow. */
const INIT_TIMEOUT_MS = 30_000;
/** ~20 seeks/sec: responsive to drag, without rebuilding a decoder per frame. */
const SEEK_MIN_INTERVAL_MS = 50;

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
	/**
	 * Known-good duration and frame rate, if the host already has them (ours come
	 * from ffprobe). Supplying these skips two container walks that are O(file)
	 * on a fragmented MP4 and were enough to time out opening a 4K recording.
	 */
	durationSec?: number;
	fps?: number;
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
	/** Latest seek target awaiting the rate limiter; the newest always wins. */
	#pendingSeekSec: number | null = null;
	#lastSeekPostedMs = -Infinity;
	#seekTimer: ReturnType<typeof setTimeout> | undefined;
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
	readonly ingestion: "whole" | "progressive" = "progressive";

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
	/**
	 * Take ownership of each decoded frame as it arrives. Consume it
	 * SYNCHRONOUSLY (upload to a texture); it is closed the moment this returns.
	 *
	 * Setting this switches off the frame cache, and that is the point: a cached
	 * `VideoFrame` holds a decoder output surface, and holding several at 4K
	 * starves the decoder until it stops emitting entirely.
	 */
	onFrameDecoded: ((frame: VideoFrame, tsUs: number) => void) | null = null;

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
	 * Open `src` and resolve once the source can answer `frameAt`. Rejects if
	 * the worker dies or the input is unreadable; the caller should fall back
	 * to the `<video>` element. A bare string is treated as a URL ref.
	 */
	static async create(
		src: MediaRef | Blob | string,
		options: MediabunnySourceOptions,
	): Promise<MediabunnyVideoSource> {
		if (typeof Worker === "undefined" || typeof VideoFrame === "undefined") {
			throw new MediaError("unsupported", "Worker/VideoFrame unavailable in this WebView");
		}
		const ref = toMediaRef(src);
		// Reject known-undecodable containers up front rather than spawning a
		// worker to discover it. The caller falls back to <video> either way.
		const ext = mediaRefExtension(ref);
		if (ext && isUnsupportedContainer(ext)) {
			throw new MediaError("unsupported", `MediaBunny cannot decode .${ext} files`);
		}
		const worker = options.createWorker();
		let timer: ReturnType<typeof setTimeout> | undefined;
		try {
			const meta = await new Promise<{
				width: number;
				height: number;
				durationSec: number;
				fps: number;
			}>((resolve, reject) => {
				worker.onmessage = (e: MessageEvent<FromMediabunnyWorker>) => {
					const msg = e.data;
					if (msg.type === "ready") {
						resolve(msg);
					} else if (msg.type === "error") {
						reject(new MediaError("bad-input", msg.message));
					}
				};
				// A worker that fails to LOAD fires onerror with an empty message,
				// so name the script — that distinguishes it from a throw inside.
				worker.onerror = (e) =>
					reject(
						new MediaError(
							"worker-died",
							e.message || `worker script failed to load: ${e.filename || "unknown"}`,
						),
					);
				// Without this a stalled read (an asset-protocol fetch that never
				// settles) leaves the caller waiting forever with no error.
				timer = setTimeout(
					() => reject(new MediaError("worker-died", "Timed out opening the media source")),
					INIT_TIMEOUT_MS,
				);
				const init: ToMediabunnyWorker = {
					type: "init",
					src: ref,
					durationSec: options.durationSec,
					fps: options.fps,
				};
				worker.postMessage(init);
			});
			clearTimeout(timer);
			const source = new MediabunnyVideoSource(worker, meta);
			// Singleton cache keyed by bare timestamp: scope it or another
			// recording's frame answers reads for this one.
			source.#cache.setScope(mediaRefKey(ref));
			// Frames are GPU surfaces; a cap that is safe at 1080p starves the
			// decoder's pool at 4K, which is the classic ~8fps stall.
			source.#cache.memoryCapBytes = frameCacheCapBytes(meta.width, meta.height);
			return source;
		} catch (err) {
			clearTimeout(timer);
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
			// Transferred frames are ours now; dropping one without closing leaks
			// a decode surface.
			if (msg.type === "frame") msg.frame.close();
			return;
		}
		if (msg.type === "frame") {
			// Frames from a superseded run are still valid pictures for their own
			// timestamp, so cache them. Dropping late frames (the old behavior)
			// threw away work and starved the display.
			const frame = msg.frame;
			const tUs = Math.round(msg.originalSec * 1_000_000);
			if (this.onFrameDecoded) {
				// Hand off and release in the same tick, so the surface goes
				// straight back to the decoder's pool.
				try {
					this.onFrameDecoded(frame, tUs);
				} finally {
					frame.close();
				}
			} else {
				// Memory only: a `VideoFrame` can't be structured-cloned into
				// IndexedDB, and the streaming decoder would write per frame.
				this.#cache.write(tUs, frame as unknown as CachedFrame, false);
			}
			this.#decodedFrames++;
			// §3 time-to-first-frame and scrub-seek rows, visible on the DevTools
			// timeline. Only the first frame of a run closes the seek measure.
			if (!this.#sawFirstFrame) {
				this.#sawFirstFrame = true;
				measureSince("time-to-first-frame", this.#startedAtMs, {
					width: this.width,
					height: this.height,
				});
			}
			if (this.#seekStartedMs > 0) {
				measureSince("seek-latency", this.#seekStartedMs, { seq: msg.seq });
				this.#seekStartedMs = 0;
			}
			// Log jumps only: the run streams every frame, so per-frame logging
			// would put 30-60 lines/sec into the dev console.
			if (DIAG && msg.seq !== this.#loggedSeq) {
				this.#loggedSeq = msg.seq;
				console.log(
					`[mb] run ${msg.seq} @ ${msg.originalSec.toFixed(3)}s (${msg.width}x${msg.height})`,
				);
			}
			// Paint, since the editor's rAF loop is the only thing that re-renders
			// during a pause, and a freshly-decoded seek target needs to repaint.
			this.onFrame?.();
			return;
		}
		if (msg.type === "error") {
			// A dead run means a frozen picture, not a degraded one — never
			// downgrade this to a warning.
			console.error("[mb] decode run failed:", msg.code, msg.message);
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
	/**
	 * Tell the worker where the playhead is. A jump starts a new decode run;
	 * steady playback only releases backpressure — posting a seek per frame is
	 * what made every request abort the one before it.
	 *
	 * Call this once per rendered frame, whether or not you read from the cache.
	 */
	advanceTo(originalSec: number): void {
		if (this.#disposed) return;
		const delta = originalSec - this.#lastRequestSec;
		const isJump = this.#lastRequestSec < 0 || delta < -FRAME_SLACK_SEC || delta > JUMP_SEC;
		this.#lastRequestSec = originalSec;
		if (isJump) this.#requestSeek(originalSec);
		else this.#post({ type: "playhead", originalSec });
	}

	/**
	 * Rate-limit seeks. Every seek starts a fresh decode run with its own
	 * decoder, and a drag produces one per pointer move — so an unthrottled
	 * scrub builds and destroys ~60 decoders a second. The latest target always
	 * wins, so the picture still lands where the user let go.
	 */
	#requestSeek(originalSec: number): void {
		this.#pendingSeekSec = originalSec;
		if (this.#seekTimer !== undefined) return;
		const sinceMs = markNow() - this.#lastSeekPostedMs;
		if (sinceMs >= SEEK_MIN_INTERVAL_MS) {
			this.#flushSeek();
			return;
		}
		this.#seekTimer = setTimeout(() => {
			this.#seekTimer = undefined;
			this.#flushSeek();
		}, SEEK_MIN_INTERVAL_MS - sinceMs);
	}

	#flushSeek(): void {
		const target = this.#pendingSeekSec;
		if (target === null || this.#disposed) return;
		this.#pendingSeekSec = null;
		this.#lastSeekPostedMs = markNow();
		this.#seekStartedMs = markNow();
		this.#post({ type: "seek", seq: ++this.#seq, originalSec: target });
	}

	frameAt(originalSec: number, floorSec = 0): VideoFrame | null {
		if (this.#disposed) return null;
		const tUs = Math.max(0, Math.round(originalSec * 1e6));
		const floorUs = Math.max(0, Math.round(floorSec * 1e6));
		const cached = this.#bestCached(tUs, floorUs);
		this.advanceTo(originalSec);

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
		this.#post({ type: "prefetch", seq: ++this.#seq, originalSec });
	}

	dispose(): void {
		if (this.#disposed) return;
		this.#disposed = true;
		clearTimeout(this.#seekTimer);
		this.#seekTimer = undefined;
		if (this.onStats) this.onStats(this.stats());
		this.#post({ type: "dispose" });
		// Persistent cache survives on purpose: the next session should hit it.
		// Worker self-closes; terminate is the backstop.
		this.#worker.terminate();
	}
}
