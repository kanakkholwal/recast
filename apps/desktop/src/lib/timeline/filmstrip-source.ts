/**
 * Filmstrip tile providers: the main-thread side of the clip-bar thumbnails.
 *
 * The clip bar plans virtualized tiles (./filmstrip.ts) and asks the provider
 * for each tile's image URL. WebCodecsTileProvider decodes per-tile frames in
 * filmstrip-worker.ts, downscaled, cached as object URLs (sharp, density-aware);
 * whole-file inputs only. `createTileProvider` returns null for huge/progressive
 * files or a WebView without WebCodecs; the clip bar then keeps its existing
 * stretched Rust-strip rendering. Only on-screen tiles are ever requested, so
 * decode work tracks what virtualization shows.
 */

import { chooseIngestion } from "../playback/mp4-sample-table";
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
	dispose(): void;
}

/** Hover-scrub time bucket (seconds), coarser than the filmstrip so dragging the
 *  cursor doesn't decode a frame per pixel. */
const HOVER_QUANTUM = 0.05;

/** Decoded thumbnail URLs kept resident. Covers a wide viewport plus overscan
 *  across a couple of zoom levels; eviction revokes the object URL. */
const MAX_TILES = 240;

class WebCodecsTileProvider implements TileProvider {
	#worker: Worker;
	#cache: LruCache<string>;
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
	/** Built storyboard sprite, and whether its one-time build is requested. */
	#storyboard: Storyboard | undefined;
	#storyboardRequested = false;

	private constructor(worker: Worker, onChange: () => void) {
		this.#worker = worker;
		this.#onChange = onChange;
		this.#cache = new LruCache<string>(MAX_TILES, (url) =>
			URL.revokeObjectURL(url),
		);
		this.#worker.onmessage = (e: MessageEvent<FromFilmstripWorker>) =>
			this.#onMessage(e.data);
	}

	static async create(
		url: string,
		tileHeightPx: number,
		onChange: () => void,
	): Promise<WebCodecsTileProvider> {
		const res = await fetch(url);
		if (!res.ok) throw new Error(`fetch failed: HTTP ${res.status}`);
		const buffer = await res.arrayBuffer();
		const worker = new Worker(
			new URL("./filmstrip-worker.ts", import.meta.url),
			{ type: "module" },
		);
		try {
			await new Promise<void>((resolve, reject) => {
				worker.onmessage = (e: MessageEvent<FromFilmstripWorker>) => {
					const m = e.data;
					if (m.type === "ready") resolve();
					else if (m.type === "error") reject(new Error(m.message));
				};
				worker.onerror = (e) =>
					reject(new Error(e.message || "filmstrip worker error"));
				const init: ToFilmstripWorker = { type: "init", buffer, tileHeightPx };
				worker.postMessage(init, [buffer]);
			});
		} catch (err) {
			worker.terminate();
			throw err;
		}
		return new WebCodecsTileProvider(worker, onChange);
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
		const cached = this.#cache.get(cacheKey);
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
			const msg: ToFilmstripWorker = { type: "storyboard" };
			this.#worker.postMessage(msg);
		}
		return this.#storyboard;
	}

	#scheduleFlush(): void {
		if (this.#pending.size > 0 && !this.#flushScheduled) {
			this.#flushScheduled = true;
			requestAnimationFrame(() => this.#flush());
		}
	}

	#flush(): void {
		this.#flushScheduled = false;
		if (this.#disposed || this.#pending.size === 0) return;
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

	#onMessage(msg: FromFilmstripWorker): void {
		if (msg.type === "error") {
			console.error("filmstrip worker:", msg.message);
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
		const cacheKey = this.#idToKey.get(msg.id);
		this.#idToKey.delete(msg.id);
		if (cacheKey === undefined) return;
		this.#inflight.delete(cacheKey);
		if (this.#disposed) return;
		this.#cache.set(cacheKey, URL.createObjectURL(msg.blob));
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
		if (this.#storyboard) URL.revokeObjectURL(this.#storyboard.url);
		this.#storyboard = undefined;
		this.#inflight.clear();
		this.#idToKey.clear();
		this.#pending.clear();
	}
}

export interface TileProviderInput {
	/** Tauri asset URL of the source video. */
	url: string;
	/** Source size (bytes) from the probe; selects whole-file vs progressive. */
	sizeBytes: number | undefined;
	/** Device-pixel tile height to decode thumbnails at. */
	tileHeightPx: number;
	/** Called when a new tile lands, so the clip bar can repaint. */
	onChange: () => void;
}

/**
 * Build the WebCodecs tile provider for whole-file inputs in a capable WebView.
 * Returns null for huge/progressive files or on decoder failure; the caller
 * falls back to the stretched Rust strip. Never throws.
 */
export async function createTileProvider(
	input: TileProviderInput,
): Promise<TileProvider | null> {
	const canDecode =
		chooseIngestion(input.sizeBytes) === "whole" &&
		typeof Worker !== "undefined" &&
		typeof VideoFrame !== "undefined";
	if (!canDecode) return null;
	try {
		return await WebCodecsTileProvider.create(
			input.url,
			input.tileHeightPx,
			input.onChange,
		);
	} catch (err) {
		console.warn("Filmstrip decoder unavailable, using strip fallback", err);
		return null;
	}
}
