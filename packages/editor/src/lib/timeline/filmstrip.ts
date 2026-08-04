/**
 * Filmstrip layout: density-based, virtualized thumbnail tiling for the clip bar.
 *
 * Replaces the old fixed 8–12 stretched strip. Tiles are laid per kept segment
 * block on the OUTPUT axis (each block is cut-free internally), and the count
 * scales with the block's pixel width, so a long clip gets more, sharper tiles
 * instead of a dozen blurry stretched ones, and zooming in adds tiles rather
 * than magnifying the same images.
 *
 * A tile's sample time is interpolated across the block's ORIGINAL range, so it
 * stays correct under per-segment speed for free: speeding a segment narrows its
 * output width (fewer tiles) without touching its original span. Only tiles that
 * intersect the viewport (plus overscan) are emitted; the decoder never works on
 * off-screen frames. See ./time-map.ts for the axis the block widths come from.
 */

/** A kept segment block placed on the output axis. */
export interface FilmstripBlock {
	/** Stable identity (the segment's original start). */
	key: number;
	/** Block start on the output axis, px. */
	leftPx: number;
	/** Block width on the output axis, px (already reflects speed). */
	widthPx: number;
	/** Block start on the original-recording timeline, seconds. */
	originalStart: number;
	/** Block end on the original-recording timeline, seconds. */
	originalEnd: number;
}

export interface FilmstripViewport {
	/** Horizontal scroll offset of the timeline content, px. */
	leftPx: number;
	/** Visible width, px. */
	widthPx: number;
}

export interface FilmstripOptions {
	/** Target tile width, px; tiles per block = ceil(blockWidth / this). */
	tileWidthPx: number;
	/** Tile height, px; part of the cache key, since sharpness depends on it. */
	tileHeightPx: number;
	/** Extra px decoded beyond each viewport edge. Default 0. */
	overscanPx?: number;
}

export interface FilmstripTile {
	/** Owning block's key, for rendering inside the per-block container. */
	blockKey: number;
	/** Offset within the block, px (block containers are overflow-hidden). */
	offsetPx: number;
	/** Tile width, px. */
	widthPx: number;
	/** Original-recording time to decode for this tile, seconds. */
	sampleOriginalSec: number;
	/** Stable LRU key: tile height + quantized sample time. */
	cacheKey: string;
}

/** Cache-key time bucket (seconds): coarse enough to reuse tiles across small
 *  zoom/scroll changes, fine enough that adjacent tiles stay distinct. */
const TIME_QUANTUM = 0.01;

function intersects(leftPx: number, rightPx: number, viewLeft: number, viewRight: number): boolean {
	return rightPx >= viewLeft && leftPx <= viewRight;
}

/**
 * Plan the visible tiles across the given blocks. Off-screen blocks and tiles
 * are dropped (virtualization); each surviving tile carries the original time to
 * decode and a stable cache key.
 */
export function planFilmstrip(
	blocks: FilmstripBlock[],
	viewport: FilmstripViewport,
	opts: FilmstripOptions,
): FilmstripTile[] {
	const overscan = opts.overscanPx ?? 0;
	const tileTarget = Math.max(8, opts.tileWidthPx);
	const viewLeft = viewport.leftPx - overscan;
	const viewRight = viewport.leftPx + viewport.widthPx + overscan;

	const tiles: FilmstripTile[] = [];
	for (const block of blocks) {
		if (block.widthPx <= 0) continue;
		if (!intersects(block.leftPx, block.leftPx + block.widthPx, viewLeft, viewRight)) {
			continue;
		}
		const count = Math.max(1, Math.ceil(block.widthPx / tileTarget));
		const tileW = block.widthPx / count;
		const origSpan = block.originalEnd - block.originalStart;
		for (let i = 0; i < count; i++) {
			const tileLeftAbs = block.leftPx + i * tileW;
			if (!intersects(tileLeftAbs, tileLeftAbs + tileW, viewLeft, viewRight)) {
				continue;
			}
			const centerFrac = (i + 0.5) / count;
			const sampleOriginalSec = block.originalStart + centerFrac * origSpan;
			const bucket = Math.round(sampleOriginalSec / TIME_QUANTUM);
			tiles.push({
				blockKey: block.key,
				offsetPx: i * tileW,
				widthPx: tileW,
				sampleOriginalSec,
				cacheKey: `${opts.tileHeightPx}:${bucket}`,
			});
		}
	}
	return tiles;
}

/**
 * Bounded LRU keyed by string. Reads refresh recency; inserts past `max` evict
 * the least-recently-used entry and call `onEvict`, the hook that closes an
 * ImageBitmap so decoded tiles don't leak GPU memory.
 */
export class LruCache<V> {
	#max: number;
	#map = new Map<string, V>();
	#onEvict?: (value: V) => void;

	constructor(max: number, onEvict?: (value: V) => void) {
		this.#max = Math.max(1, max);
		this.#onEvict = onEvict;
	}

	get size(): number {
		return this.#map.size;
	}

	has(key: string): boolean {
		return this.#map.has(key);
	}

	get(key: string): V | undefined {
		const value = this.#map.get(key);
		if (value === undefined) return undefined;
		// Refresh recency: re-insert moves the key to the newest position.
		this.#map.delete(key);
		this.#map.set(key, value);
		return value;
	}

	set(key: string, value: V): void {
		if (this.#map.has(key)) this.#map.delete(key);
		this.#map.set(key, value);
		while (this.#map.size > this.#max) {
			const oldest = this.#map.keys().next().value as string;
			const evicted = this.#map.get(oldest) as V;
			this.#map.delete(oldest);
			this.#onEvict?.(evicted);
		}
	}

	clear(): void {
		if (this.#onEvict) {
			for (const value of this.#map.values()) this.#onEvict(value);
		}
		this.#map.clear();
	}
}
