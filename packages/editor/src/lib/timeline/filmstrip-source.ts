/**
 * Filmstrip tile providers: the main-thread side of the clip-bar thumbnails.
 *
 * The clip bar plans virtualized tiles (./filmstrip.ts) and asks the provider
 * for each tile's image URL. The provider decodes per-tile frames in
 * `filmstrip-worker.ts` (MediaBunny-backed) and caches them as object URLs
 * so the bar can repaint cheaply when the playhead sweeps. Only on-screen
 * tiles are ever requested, so decode work tracks what virtualization shows.
 *
 * The worker range-streams the source via MediaBunny's `UrlSource`, so the
 * main thread never holds the whole file. Only multi-GB inputs fall back to the
 * Rust-rendered strip (`MAX_STREAM_BYTES`).
 */

import { type MediaRef, toMediaRef } from "@recast/media";
import { createEditorWorker } from "../host-hooks";
import { type FilmstripTile, LruCache } from "./filmstrip";
import type { FromFilmstripWorker, ToFilmstripWorker } from "./filmstrip-protocol";

/** A built storyboard sprite: one image of `cols`×`rows` cells (`cellW`×`cellH`
 *  each) holding `count` frames evenly spaced across `durationSec`. Cell `i`
 *  (col `i%cols`, row `i/cols`) samples `((i+0.5)/count)·durationSec`. */
export interface Storyboard {
	url: string;
	cols: number;
	rows: number;
	cellW: number;
	cellH: number;
	count: number;
	durationSec: number;
}

export interface TileProvider {
	/** The image URL for a planned tile, or undefined while it's still decoding. */
	get(tile: FilmstripTile): string | undefined;
	/** Ensure these (already virtualized) tiles get decoded. */
	request(tiles: FilmstripTile[]): void;
	/** A decoded frame URL near `originalSec` for hover-scrub, or undefined while
	 *  it decodes (the call also queues the decode). */
	previewAt(originalSec: number): string | undefined;
	/** The storyboard sprite for instant hover-scrub, or undefined until built;
	 *  the first call kicks off the one-time build. */
	storyboard(): Storyboard | undefined;
	/** Pause/resume decoding new tiles. Paused during playback so the filmstrip's
	 *  decoder doesn't compete with the preview decoder for hardware slots;
	 *  requests made while paused are queued and flushed on resume. */
	setDecodePaused(paused: boolean): void;
	dispose(): void;
}

/** Hover-scrub time bucket (seconds), coarser than the filmstrip so dragging the
 *  cursor doesn't decode a frame per pixel. */
const HOVER_QUANTUM = 0.05;

/** Decoded thumbnail URLs kept resident. Covers a wide viewport plus overscan
 *  across a couple of zoom levels; eviction revokes the object URL. */
const MAX_TILES = 240;

/**
 * Hover frames live in their OWN cache. They used to share the tile cache, and a
 * 60s clip has ~1200 hover buckets, so a few seconds of scrubbing evicted every
 * filmstrip tile on screen. The clip bar only re-requests tiles when its plan
 * changes, so the evicted thumbnails stayed grey until you happened to zoom or
 * scroll: "sometimes I can see the thumbnails, sometimes I can't".
 */
const MAX_HOVER_FRAMES = 64;

/**
 * Above this source size, prefer the Rust-rendered strip over the streaming
 * filmstrip. The worker no longer buffers the whole file (it range-streams), so
 * this is not a memory ceiling anymore — just a point past which random-access
 * range decode over a multi-GB file isn't worth it versus the fixed strip.
 */
const MAX_STREAM_BYTES = 4_000_000_000;

class MediabunnyTileProvider implements TileProvider {
	#worker: Worker;
	#cache: LruCache<string>;
	#hoverCache: LruCache<string>;
	/** cacheKeys currently being decoded by the worker. */
	#inflight = new Set<string>();
	/** Worker request id → cacheKey, to file the reply. */
	#idToKey = new Map<number, string>();
	#nextId = 0;
	#onChange: () => void;
	/** cacheKey → sample time, batched and flushed once per frame. */
	#pending = new Map<string, number>();
	#flushScheduled = false;
	#disposed = false;
	/** When true, `#flush` holds requests in `#pending` instead of decoding, so
	 *  the filmstrip decoder is quiet while the preview decoder is busy. */
	#decodePaused = false;
	/** Built storyboard sprite, and whether its one-time build is requested. */
	#storyboard: Storyboard | undefined;
	#storyboardRequested = false;
	#storyboardQueued = false;

	private constructor(worker: Worker, onChange: () => void) {
		this.#worker = worker;
		this.#onChange = onChange;
		this.#cache = new LruCache<string>(MAX_TILES, (url) => URL.revokeObjectURL(url));
		this.#hoverCache = new LruCache<string>(MAX_HOVER_FRAMES, (url) => URL.revokeObjectURL(url));
		this.#worker.onmessage = (e: MessageEvent<FromFilmstripWorker>) => this.#onMessage(e.data);
		// Replaces the init promise's `reject`, which is settled: left in place it swallowed every later worker crash and suppressed the default console report too.
		this.#worker.onerror = (e) => console.error("filmstrip worker crashed:", e.message);
	}

	static async create(
		src: MediaRef | Blob | string,
		tileHeightPx: number,
		onChange: () => void,
		durationSec?: number,
	): Promise<MediabunnyTileProvider> {
		// The worker reads the source lazily, so the main thread never holds the whole recording (~600MB on a 4K clip).
		const worker = createEditorWorker("filmstrip");
		try {
			await new Promise<void>((resolve, reject) => {
				worker.onmessage = (e: MessageEvent<FromFilmstripWorker>) => {
					const m = e.data;
					if (m.type === "ready") resolve();
					else if (m.type === "error") reject(new Error(m.message));
				};
				worker.onerror = (e) => reject(new Error(e.message || "filmstrip worker error"));
				const init: ToFilmstripWorker = {
					type: "init",
					src: toMediaRef(src),
					tileHeightPx,
					durationSec,
				};
				worker.postMessage(init);
			});
		} catch (err) {
			worker.terminate();
			throw err;
		}
		return new MediabunnyTileProvider(worker, onChange);
	}

	get(tile: FilmstripTile): string | undefined {
		return this.#cache.get(tile.cacheKey);
	}

	request(tiles: FilmstripTile[]): void {
		if (this.#disposed) return;
		for (const t of tiles) {
			if (
				this.#cache.has(t.cacheKey) ||
				this.#inflight.has(t.cacheKey) ||
				this.#pending.has(t.cacheKey)
			) {
				continue;
			}
			this.#pending.set(t.cacheKey, t.sampleOriginalSec);
		}
		this.#scheduleFlush();
	}

	previewAt(originalSec: number): string | undefined {
		if (this.#disposed) return undefined;
		// Own cache namespace so hover frames don't collide with filmstrip tiles.
		const cacheKey = `hover:${Math.round(originalSec / HOVER_QUANTUM)}`;
		const cached = this.#hoverCache.get(cacheKey);
		if (cached) return cached;
		if (!this.#inflight.has(cacheKey) && !this.#pending.has(cacheKey)) {
			this.#pending.set(cacheKey, Math.max(0, originalSec));
			this.#scheduleFlush();
		}
		return undefined;
	}

	storyboard(): Storyboard | undefined {
		if (this.#disposed) return undefined;
		// First request kicks off the one-time build; the reply lands in #onMessage.
		if (!this.#storyboard && !this.#storyboardRequested) {
			this.#storyboardRequested = true;
			this.#storyboardQueued = true;
			this.#maybeSendStoryboard();
		}
		return this.#storyboard;
	}

	/** The storyboard is 32 decodes. It used to post straight past `#flush`, so
	 *  the shared `DecoderBudget` lease never applied to it and it ran against the
	 *  preview's own cold init. Held until decoding is allowed. */
	#maybeSendStoryboard(): void {
		if (this.#disposed || !this.#storyboardQueued || this.#decodePaused) return;
		this.#storyboardQueued = false;
		const msg: ToFilmstripWorker = { type: "storyboard" };
		this.#worker.postMessage(msg);
	}

	#scheduleFlush(): void {
		if (this.#pending.size > 0 && !this.#flushScheduled) {
			this.#flushScheduled = true;
			requestAnimationFrame(() => this.#flush());
		}
	}

	setDecodePaused(paused: boolean): void {
		if (this.#decodePaused === paused) return;
		this.#decodePaused = paused;
		// Resuming: drain whatever queued up while paused.
		if (!paused) {
			this.#scheduleFlush();
			this.#maybeSendStoryboard();
		}
	}

	#flush(): void {
		this.#flushScheduled = false;
		if (this.#disposed || this.#pending.size === 0) return;
		// Paused (playback): keep the requests queued; the resume drains them.
		if (this.#decodePaused) return;
		const requests: Array<{ id: number; originalSec: number }> = [];
		for (const [cacheKey, originalSec] of this.#pending) {
			const id = this.#nextId++;
			this.#idToKey.set(id, cacheKey);
			this.#inflight.add(cacheKey);
			requests.push({ id, originalSec });
		}
		this.#pending.clear();
		const msg: ToFilmstripWorker = { type: "decode", requests };
		this.#worker.postMessage(msg);
	}

	/** Clear a request from in-flight and return its cache key. Every reply path
	 *  must go through this or the tile wedges and the id maps grow unbounded. */
	#release(id: number): string | undefined {
		const cacheKey = this.#idToKey.get(id);
		this.#idToKey.delete(id);
		if (cacheKey !== undefined) this.#inflight.delete(cacheKey);
		return cacheKey;
	}

	#onMessage(msg: FromFilmstripWorker): void {
		if (msg.type === "storyboard-error") {
			console.error("filmstrip storyboard:", msg.message);
			// Latch cleared so the next request rebuilds: one failure used to drop hover scrub to per-position decodes for the rest of the session.
			this.#storyboardRequested = false;
			this.#storyboardQueued = false;
			return;
		}
		if (msg.type === "error") {
			console.error("filmstrip worker:", msg.message);
			// Release the id so the tile can be re-requested and the id and inflight maps don't grow without bound.
			if (msg.id !== undefined) this.#release(msg.id);
			return;
		}
		if (msg.type === "drop") {
			// Evicted, not failed: release it so it can be re-requested when it scrolls back in, and stay quiet.
			this.#release(msg.id);
			return;
		}
		if (msg.type === "storyboard") {
			if (this.#disposed) return;
			this.#storyboard = {
				url: URL.createObjectURL(msg.blob),
				cols: msg.cols,
				rows: msg.rows,
				cellW: msg.cellW,
				cellH: msg.cellH,
				count: msg.count,
				durationSec: msg.durationSec,
			};
			this.#onChange();
			return;
		}
		if (msg.type !== "tile") return;
		const cacheKey = this.#release(msg.id);
		if (cacheKey === undefined || this.#disposed) return;
		const target = cacheKey.startsWith("hover:") ? this.#hoverCache : this.#cache;
		target.set(cacheKey, URL.createObjectURL(msg.blob));
		this.#onChange();
	}

	dispose(): void {
		if (this.#disposed) return;
		this.#disposed = true;
		try {
			const msg: ToFilmstripWorker = { type: "dispose" };
			this.#worker.postMessage(msg);
		} catch {
			/* worker already gone */
		}
		this.#worker.terminate();
		this.#cache.clear(); // revokes every object URL
		this.#hoverCache.clear();
		if (this.#storyboard) URL.revokeObjectURL(this.#storyboard.url);
		this.#storyboard = undefined;
		this.#inflight.clear();
		this.#idToKey.clear();
		this.#pending.clear();
	}
}

export interface TileProviderInput {
	/** The source video: a Tauri asset URL on desktop, the picked File on web. */
	src: MediaRef | Blob | string;
	/**
	 * Source size (bytes) from the probe. The worker reads lazily, so this no
	 * longer gates whole-file residency; it stays as a hint for very-large-file
	 * policy (see `MAX_STREAM_BYTES`).
	 */
	sizeBytes?: number;
	/** Known duration (ffprobe) so the worker skips a full container walk. */
	durationSec?: number;
	/** Device-pixel tile height to decode thumbnails at. */
	tileHeightPx: number;
	/** Called when a new tile lands, so the clip bar can repaint. */
	onChange: () => void;
}

/**
 * Build the MediaBunny-backed tile provider. Returns null on environments
 * that lack Worker/OffscreenCanvas, when the fetch fails, or when the
 * MediaBunny worker reports a decode error; the caller falls back to the
 * Rust-strip renderer in those cases. Never throws.
 */
export async function createTileProvider(input: TileProviderInput): Promise<TileProvider | null> {
	if (typeof Worker === "undefined" || typeof OffscreenCanvas === "undefined") {
		console.info("Filmstrip: WebView lacks Worker/OffscreenCanvas; using strip fallback.");
		return null;
	}
	// Only multi-GB sources fall back now; the worker streams the rest.
	if (input.sizeBytes !== undefined && input.sizeBytes > MAX_STREAM_BYTES) {
		console.info(
			`Filmstrip: ${Math.round(input.sizeBytes / 1e6)}MB is past the streaming budget; using strip fallback.`,
		);
		return null;
	}
	try {
		return await MediabunnyTileProvider.create(
			input.src,
			input.tileHeightPx,
			input.onChange,
			input.durationSec,
		);
	} catch (err) {
		console.warn("Filmstrip decoder unavailable, using strip fallback", err);
		return null;
	}
}
