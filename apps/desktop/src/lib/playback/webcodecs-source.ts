/**
 * WebCodecsVideoSource: frame-accurate video decode for the editor preview.
 *
 * Fixes "playback freezes at a cut". The old `<video>`-element preview crossed a
 * cut via `video.currentTime = cut.end`, a native seek whose decode-from-keyframe
 * latency is the visible stall and which we can't control. Here we own the decode
 * pipeline instead:
 *   - mp4box.js demuxes into encoded samples + the codec config, with a
 *     presentation-time index.
 *   - a WebCodecs `VideoDecoder` decodes on demand into `VideoFrame`s that WebGL
 *     uploads directly (`texImage2D(..., videoFrame)`).
 *   - decoded frames live in a bounded cache so frames around the playhead (and
 *     across an upcoming cut) are already in memory; a "seek" becomes a cache
 *     lookup + a short decode scheduled ahead of time, not a black-box stall.
 *
 * Threading: demux, decoder, and decode-ahead all run in `webcodecs-worker.ts`.
 * This class is the main-thread proxy: it owns the bounded decoded-frame cache
 * (frames transferred from the worker), answers the render loop synchronously,
 * and tells the worker where the playhead is.
 *
 * Ownership: `frameAt()` returns a frame owned by the cache, so upload it
 * synchronously, do NOT close it. `VideoFrame`s are GPU-backed and refcounted;
 * leaking them exhausts the decoder, so every cached frame has exactly one close
 * path (eviction or `dispose()`).
 */

import { frameBudget } from "./frame-budget";
import { chooseIngestion } from "./mp4-sample-table";
import type { FromWorker, ToWorker } from "./webcodecs-protocol";

// The cache caps (primary `#cacheMax`, scout `#holdoutMax`) are resolution-
// adaptive (see `frame-budget.ts`): each held VideoFrame keeps a decoder output
// surface checked out, and at 4K/5K those are 4–7× larger, so the 1080p count
// would starve the pool. The scout holdout is SEPARATE from the main cache so
// the primary's eviction can't drop the pre-warmed post-cut frames before
// playback reaches the cut. See `#evict` for the retention policy.

/** Dev-only diagnostics (throughput + first-frame geometry). */
const DIAG = import.meta.env.DEV;

export class WebCodecsVideoSource {
	#worker: Worker;
	/** Decoded frames, keyed by ctsUs. */
	#cache = new Map<number, VideoFrame>();
	/** Protected scout-decoded frames for an upcoming post-cut GOP. Never touched
	 * by `#evict`. Bounded by `#holdoutMax`. */
	#prefetchCache = new Map<number, VideoFrame>();
	/** Resolution-adaptive caps (set in the constructor from `frameBudget`). */
	#cacheMax: number;
	#holdoutMax: number;
	/** Last requested presentation time (µs) that drives eviction. */
	#currentUs = 0;
	#disposed = false;

	readonly width: number;
	readonly height: number;
	readonly durationSec: number;
	readonly fps: number;
	/** Ingestion strategy this source was created with (telemetry/diagnostics). */
	readonly ingestion: "whole" | "progressive";

	/**
	 * Called after a newly-decoded frame lands in the cache. The render loop is
	 * driven by playback, so while PAUSED (a scrub) nothing would repaint when
	 * the worker finishes decoding the seeked-to frame, so wire this to a redraw.
	 */
	onFrame: (() => void) | null = null;

	/**
	 * Emitted once on `dispose()` with aggregate decode throughput for this
	 * source, the production `webcodecs_preview_perf` signal that gates the
	 * default-on decision. Not called if playback never ran long enough to
	 * sample a full window (e.g. the user only opened and closed the editor).
	 */
	onStats:
		| ((s: { avgFps: number; minFps: number; maxLateMs: number }) => void)
		| null = null;

	#loggedDims = false;
	// Aggregate decode throughput over 1s windows. Idle windows (scrub/pause,
	// ≈0 fps) are dropped so the average reflects real playback. `#perfMaxLateMs`
	// = worst frame lateness vs the clock. Emitted once via `onStats` on dispose.
	#perfFrames = 0;
	#perfWindowStart = 0;
	#perfWindowFrames = 0;
	#perfFpsSum = 0;
	#perfFpsSamples = 0;
	#perfMinFps = Infinity;
	#perfMaxLateMs = 0;

	private constructor(
		worker: Worker,
		meta: { width: number; height: number; durationSec: number; fps: number },
		ingestion: "whole" | "progressive",
	) {
		this.#worker = worker;
		this.width = meta.width;
		this.height = meta.height;
		this.durationSec = meta.durationSec;
		this.fps = meta.fps;
		this.ingestion = ingestion;
		// Size the decoded-frame caches to the resolution so 4K/5K sources don't
		// starve the decoder's output-surface pool (the keystone stall).
		const budget = frameBudget(meta.width, meta.height);
		this.#cacheMax = budget.cacheMax;
		this.#holdoutMax = budget.holdoutMax;
		if (DIAG) {
			console.log(
				`[wc] frame budget ${meta.width}x${meta.height} → cache=${budget.cacheMax} holdout=${budget.holdoutMax} decodeAhead=${budget.decodeAhead}`,
			);
		}
		// Take over message handling for the steady state (frames + late errors).
		this.#worker.onmessage = (e: MessageEvent<FromWorker>) =>
			this.#onMessage(e.data);
	}

	/**
	 * Spawn the decode worker, demux `url`, and resolve once the source can
	 * answer `frameAt`. Rejects if the file has no decodable video track or the
	 * codec/WebView isn't supported by WebCodecs, so the caller should fall back
	 * to the `<video>` path.
	 *
	 * `sizeBytes` (from the backend probe) selects the ingestion strategy: small
	 * files load whole into memory (zero-network steady-state playback); large
	 * 4K/5K recordings that would blow the WebView heap switch to progressive
	 * HTTP-range ingestion (flat memory, the worker fetches the moov + GOPs on
	 * demand). See `chooseIngestion`.
	 */
	static async create(url: string, sizeBytes?: number): Promise<WebCodecsVideoSource> {
		if (typeof Worker === "undefined" || typeof VideoFrame === "undefined") {
			throw new Error("WebCodecs/Worker unavailable in this WebView");
		}
		const strategy = chooseIngestion(sizeBytes);

		// Build the init message. Whole-file fetches the bytes on the main thread
		// (the asset protocol is reliably reachable here) and transfers them in;
		// progressive hands the worker the URL so it can range-fetch lazily.
		let initMsg: ToWorker;
		let transfer: Transferable[] = [];
		if (strategy === "progressive") {
			initMsg = { type: "init-progressive", url, sizeBytes: sizeBytes as number };
		} else {
			const res = await fetch(url);
			if (!res.ok) throw new Error(`fetch failed: HTTP ${res.status}`);
			const buffer = await res.arrayBuffer();
			initMsg = { type: "init", buffer };
			transfer = [buffer];
		}

		const worker = new Worker(
			new URL("./webcodecs-worker.ts", import.meta.url),
			{ type: "module" },
		);
		try {
			const meta = await new Promise<{
				width: number;
				height: number;
				durationSec: number;
				fps: number;
			}>((resolve, reject) => {
				worker.onmessage = (e: MessageEvent<FromWorker>) => {
					const msg = e.data;
					if (msg.type === "ready") {
						resolve(msg);
					} else if (msg.type === "error") {
						reject(new Error(msg.message));
					} else if (msg.type === "frame") {
						// A frame arriving before `ready` shouldn't happen; don't leak it.
						msg.frame.close();
					}
				};
				worker.onerror = (e) => reject(new Error(e.message || "worker error"));
				worker.postMessage(initMsg, transfer);
			});
			return new WebCodecsVideoSource(worker, meta, strategy);
		} catch (err) {
			worker.terminate();
			throw err;
		}
	}

	#post(msg: ToWorker): void {
		this.#worker.postMessage(msg);
	}

	#onMessage(msg: FromWorker): void {
		if (msg.type === "frame") {
			if (this.#disposed) {
				msg.frame.close();
				return;
			}
			if (DIAG && !this.#loggedDims) {
				this.#loggedDims = true;
				const f = msg.frame;
				const vr = f.visibleRect;
				console.log(
					`[wc] geometry coded=${f.codedWidth}x${f.codedHeight} display=${f.displayWidth}x${f.displayHeight} ` +
						`visible=${vr ? `${vr.x},${vr.y} ${vr.width}x${vr.height}` : "none"} ` +
						`format=${f.format} declaredMeta=${this.width}x${this.height}`,
				);
			}
			// Throughput sampling (always on: a few int ops per frame).
			this.#perfFrames++;
			this.#perfMaxLateMs = Math.max(
				this.#perfMaxLateMs,
				(this.#currentUs - msg.frame.timestamp) / 1000,
			);
			{
				const nowMs = performance.now();
				if (this.#perfWindowStart === 0) {
					this.#perfWindowStart = nowMs;
					this.#perfWindowFrames = this.#perfFrames;
				} else if (nowMs - this.#perfWindowStart >= 1000) {
					const fps =
						(this.#perfFrames - this.#perfWindowFrames) /
						((nowMs - this.#perfWindowStart) / 1000);
					if (fps > 1) {
						this.#perfFpsSum += fps;
						this.#perfFpsSamples++;
						if (fps < this.#perfMinFps) this.#perfMinFps = fps;
						if (DIAG) {
							console.log(
								`[wc] decode ${fps.toFixed(0)} fps · cache=${this.#cache.size} · maxLate=${this.#perfMaxLateMs.toFixed(0)}ms`,
							);
						}
					}
					this.#perfWindowStart = nowMs;
					this.#perfWindowFrames = this.#perfFrames;
				}
			}

			const ts = msg.frame.timestamp;
			if (msg.fromScout) {
				// Scout frame for an upcoming post-cut GOP. If the primary already
				// has this timestamp, the scout copy is redundant, so drop it.
				if (this.#cache.has(ts)) {
					msg.frame.close();
					return;
				}
				this.#prefetchCache.get(ts)?.close();
				this.#prefetchCache.set(ts, msg.frame);
				// Bound the holdout by count (oldest first); it's protected from #evict.
				while (this.#prefetchCache.size > this.#holdoutMax) {
					const oldest = this.#prefetchCache.keys().next().value as number;
					this.#prefetchCache.get(oldest)?.close();
					this.#prefetchCache.delete(oldest);
				}
				this.onFrame?.();
				return;
			}
			this.#cache.get(ts)?.close(); // never leak a replaced frame
			this.#cache.set(ts, msg.frame);
			// The primary now owns this timestamp for real, so discard any scout copy.
			this.#prefetchCache.get(ts)?.close();
			this.#prefetchCache.delete(ts);
			this.#evict();
			this.onFrame?.();
		} else if (msg.type === "error") {
			console.error("WebCodecs worker error:", msg.message);
		}
	}

	/**
	 * The decoded frame to show at original-media time `originalSec`, or null if
	 * nothing close enough is decoded yet (caller should hold the previous
	 * frame). Nudges the worker to decode toward the requested time.
	 *
	 * The returned frame is owned by the cache; upload it, don't close it.
	 */
	frameAt(originalSec: number, floorSec = 0): VideoFrame | null {
		if (this.#disposed) return null;
		const tUs = Math.max(0, Math.round(originalSec * 1e6));
		const floorUs = Math.max(0, Math.round(floorSec * 1e6));
		this.#currentUs = tUs;
		this.#post({ type: "request", originalSec });
		return this.#bestCached(tUs, floorUs);
	}

	/**
	 * Pre-decode the GOP at `originalSec` without moving the playhead, used to
	 * warm the frames just after a cut so the jump is seamless.
	 */
	prefetch(originalSec: number): void {
		if (this.#disposed) return;
		this.#post({ type: "prefetch", originalSec });
	}

	/**
	 * Best cached frame to show at `tUs`: the greatest timestamp in [floorUs, tUs].
	 * `floorUs` is the start of the current kept segment; frames before it are in
	 * a removed cut and must NOT be shown, or the picture steps BACK into deleted
	 * content right after a cut. Returns the closest at-or-before frame even if a
	 * little stale; null only when no in-segment frame is decoded yet (caller holds
	 * the last frame).
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
		// Also consider the scout holdout: right after a cut the only in-segment
		// frame yet decoded is the one the scout pre-warmed, which is what makes
		// the crossing seamless instead of a hold.
		for (const [ts, frame] of this.#prefetchCache) {
			if (ts >= floorUs && ts <= tUs && ts > bestTs) {
				bestTs = ts;
				best = frame;
			}
		}
		return best;
	}

	/**
	 * Evict around the on-screen frame, never by a fixed time window. The display
	 * frame (greatest timestamp at-or-before the playhead) is NEVER evicted, even
	 * if it arrived late. This is the fix for the old window-based eviction that
	 * dropped late frames on arrival (ts < playhead − 100ms): the clock free-runs
	 * at realtime while a costly frame (a large P-frame) decodes slower and lands
	 * behind, so heavy on-screen changes silently vanished. Now a late frame just
	 * becomes the newest display frame and is shown a touch late rather than lost.
	 */
	#evict(): void {
		// The frame we'd show right now.
		let displayTs = -Infinity;
		for (const ts of this.#cache.keys()) {
			if (ts <= this.#currentUs && ts > displayTs) displayTs = ts;
		}
		// Frames strictly before the display frame are spent; forward playback
		// won't revisit them, and a backward seek resets + refills the decoder.
		for (const [ts, frame] of this.#cache) {
			if (ts < displayTs) {
				frame.close();
				this.#cache.delete(ts);
			}
		}
		// Bound forward lookahead: if still over cap, drop the farthest-ahead
		// frames first so the display frame and the nearest upcoming frames stay.
		if (this.#cache.size > this.#cacheMax) {
			const ahead = [...this.#cache.keys()]
				.filter((ts) => ts > displayTs)
				.sort((a, b) => b - a); // farthest future first
			for (const ts of ahead) {
				if (this.#cache.size <= this.#cacheMax) break;
				this.#cache.get(ts)?.close();
				this.#cache.delete(ts);
			}
		}
	}

	dispose(): void {
		if (this.#disposed) return;
		this.#disposed = true;
		// Emit the one aggregate throughput sample for this source before tearing
		// down (the `webcodecs_preview_perf` signal). Only if playback actually ran.
		if (this.#perfFpsSamples > 0) {
			this.onStats?.({
				avgFps: this.#perfFpsSum / this.#perfFpsSamples,
				minFps: this.#perfMinFps,
				maxLateMs: this.#perfMaxLateMs,
			});
		}
		this.#post({ type: "dispose" });
		for (const frame of this.#cache.values()) frame.close();
		this.#cache.clear();
		for (const frame of this.#prefetchCache.values()) frame.close();
		this.#prefetchCache.clear();
		// The worker self-closes on dispose; terminate as a backstop.
		this.#worker.terminate();
	}
}
