import { describe, expect, it } from "vitest";
import { type FilmstripBlock, LruCache, planFilmstrip } from "./filmstrip";

function block(
	key: number,
	leftPx: number,
	widthPx: number,
	originalStart: number,
	originalEnd: number,
): FilmstripBlock {
	return { key, leftPx, widthPx, originalStart, originalEnd };
}

const WIDE_VIEW = { leftPx: 0, widthPx: 100000 };

describe("planFilmstrip density", () => {
	it("scales tile count with block width / target", () => {
		const tiles = planFilmstrip([block(0, 0, 1000, 0, 10)], WIDE_VIEW, {
			tileWidthPx: 100,
			tileHeightPx: 48,
		});
		expect(tiles).toHaveLength(10);
	});

	it("rounds the count up so the block is fully covered", () => {
		const tiles = planFilmstrip([block(0, 0, 950, 0, 10)], WIDE_VIEW, {
			tileWidthPx: 100,
			tileHeightPx: 48,
		});
		expect(tiles).toHaveLength(10);
		// Tiles evenly divide the block (no remainder sliver).
		const total = tiles.reduce((s, t) => s + t.widthPx, 0);
		expect(total).toBeCloseTo(950);
	});

	it("always emits at least one tile for a thin block", () => {
		const tiles = planFilmstrip([block(0, 0, 12, 0, 0.5)], WIDE_VIEW, {
			tileWidthPx: 100,
			tileHeightPx: 48,
		});
		expect(tiles).toHaveLength(1);
	});
});

describe("planFilmstrip virtualization", () => {
	const blocks = [block(0, 0, 4000, 0, 40)];

	it("emits only tiles intersecting the viewport", () => {
		const tiles = planFilmstrip(
			blocks,
			{ leftPx: 1000, widthPx: 200 },
			{ tileWidthPx: 100, tileHeightPx: 48 },
		);
		// A 200-wide window over 100px tiles yields the 2 covered tiles plus the two boundary-touchers, never all 40.
		expect(tiles.length).toBeLessThanOrEqual(4);
		for (const t of tiles) {
			expect(t.offsetPx + t.widthPx).toBeGreaterThanOrEqual(1000);
			expect(t.offsetPx).toBeLessThanOrEqual(1200);
		}
	});

	it("widens the window by overscan", () => {
		const tight = planFilmstrip(
			blocks,
			{ leftPx: 1000, widthPx: 200 },
			{ tileWidthPx: 100, tileHeightPx: 48 },
		);
		const padded = planFilmstrip(
			blocks,
			{ leftPx: 1000, widthPx: 200 },
			{ tileWidthPx: 100, tileHeightPx: 48, overscanPx: 300 },
		);
		expect(padded.length).toBeGreaterThan(tight.length);
	});

	it("drops blocks entirely outside the viewport", () => {
		const tiles = planFilmstrip(
			[block(0, 0, 500, 0, 5), block(1, 5000, 500, 50, 55)],
			{ leftPx: 0, widthPx: 600 },
			{ tileWidthPx: 100, tileHeightPx: 48 },
		);
		expect(tiles.every((t) => t.blockKey === 0)).toBe(true);
	});
});

describe("planFilmstrip sample time", () => {
	it("interpolates the sample across the block's original range", () => {
		const tiles = planFilmstrip([block(0, 0, 400, 10, 14)], WIDE_VIEW, {
			tileWidthPx: 100,
			tileHeightPx: 48,
		});
		// 4 tiles, centers at 12.5/37.5/62.5/87.5% of [10,14].
		expect(tiles.map((t) => t.sampleOriginalSec)).toEqual([10.5, 11.5, 12.5, 13.5]);
	});

	it("is speed-agnostic: sample depends on original span, not output width", () => {
		// Same original range [0,4], but a 2x segment is half as wide on output.
		const slow = planFilmstrip([block(0, 0, 400, 0, 4)], WIDE_VIEW, {
			tileWidthPx: 100,
			tileHeightPx: 48,
		});
		const fast = planFilmstrip([block(0, 0, 200, 0, 4)], WIDE_VIEW, {
			tileWidthPx: 100,
			tileHeightPx: 48,
		});
		// Fewer tiles when faster, but each still samples within [0,4].
		expect(fast.length).toBeLessThan(slow.length);
		for (const t of fast) {
			expect(t.sampleOriginalSec).toBeGreaterThanOrEqual(0);
			expect(t.sampleOriginalSec).toBeLessThanOrEqual(4);
		}
	});

	it("buckets the cache key by tile height and quantized time", () => {
		const [tile] = planFilmstrip([block(0, 0, 100, 2, 2.02)], WIDE_VIEW, {
			tileWidthPx: 100,
			tileHeightPx: 48,
		});
		// center = 2.01s → bucket 201 at 10ms quantum.
		expect(tile.cacheKey).toBe("48:201");
	});
});

describe("LruCache", () => {
	it("evicts the least-recently-used entry past max", () => {
		const evicted: string[] = [];
		const lru = new LruCache<string>(2, (v) => evicted.push(v));
		lru.set("a", "A");
		lru.set("b", "B");
		lru.set("c", "C"); // evicts "a"
		expect(lru.has("a")).toBe(false);
		expect(evicted).toEqual(["A"]);
	});

	it("a read refreshes recency so the entry survives eviction", () => {
		const evicted: string[] = [];
		const lru = new LruCache<string>(2, (v) => evicted.push(v));
		lru.set("a", "A");
		lru.set("b", "B");
		lru.get("a"); // "a" now newest
		lru.set("c", "C"); // evicts "b", not "a"
		expect(lru.has("a")).toBe(true);
		expect(lru.has("b")).toBe(false);
		expect(evicted).toEqual(["B"]);
	});

	it("re-setting an existing key updates value without growing size", () => {
		const lru = new LruCache<string>(2);
		lru.set("a", "A");
		lru.set("a", "A2");
		expect(lru.size).toBe(1);
		expect(lru.get("a")).toBe("A2");
	});

	it("clear evicts everything", () => {
		const evicted: string[] = [];
		const lru = new LruCache<string>(4, (v) => evicted.push(v));
		lru.set("a", "A");
		lru.set("b", "B");
		lru.clear();
		expect(lru.size).toBe(0);
		expect(evicted.sort()).toEqual(["A", "B"]);
	});
});
