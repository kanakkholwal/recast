import { describe, expect, it, vi } from 'vitest';
import { FrameCache, resetFrameCache, setFrameCache } from '../src/cache';
import { estimateFrameBytes } from '../src/cache/storage';
import type { CacheableFrame, FrameStorage } from '../src/cache/storage';

/**
 * In-memory `FrameStorage` for tests. Mirrors the production backend's
 * contract (LRU on `put` when over cap, `clear`, `size`) so the
 * orchestrator tests exercise the same control flow.
 */
class MemoryFrameStorage implements FrameStorage {
	readonly name = 'memory';
	#entries = new Map<number, { bitmap: CacheableFrame; lastUsedUs: number }>();
	#bytes = 0;
	#cap: number;

	constructor(capBytes = 1024 * 1024) {
		this.#cap = capBytes;
	}

	get capBytes(): number {
		return this.#cap;
	}

	set capBytes(value: number) {
		this.#cap = value;
	}

	async open(): Promise<void> {
		/* nothing to open */
	}

	async get(key: number): Promise<CacheableFrame | null> {
		return this.#entries.get(key)?.bitmap ?? null;
	}

	async put(key: number, frame: CacheableFrame, lastUsedUs: number): Promise<void> {
		const size = estimateFrameBytes(frame);
		if (this.#bytes + size > this.#cap) {
			// Evict LRU (smallest lastUsedUs) until it fits.
			const sorted = [...this.#entries.entries()].sort(
				(a, b) => a[1].lastUsedUs - b[1].lastUsedUs,
			);
			while (this.#bytes + size > this.#cap && sorted.length > 0) {
				const [evictKey, entry] = sorted.shift()!;
				this.#entries.delete(evictKey);
				this.#bytes -= estimateFrameBytes(entry.bitmap);
			}
		}
		this.#entries.set(key, { bitmap: frame, lastUsedUs });
		this.#bytes += size;
	}

	async deleteRange(startKey: number, endKey: number): Promise<void> {
		for (const key of [...this.#entries.keys()]) {
			if (key >= startKey && key < endKey) {
				const entry = this.#entries.get(key);
				if (entry) {
					this.#entries.delete(key);
					this.#bytes -= estimateFrameBytes(entry.bitmap);
				}
			}
		}
	}

	async clear(): Promise<void> {
		this.#entries.clear();
		this.#bytes = 0;
	}

	async size(): Promise<number> {
		return this.#bytes;
	}

	async close(): Promise<void> {
		/* nothing to close */
	}

	get bytes(): number {
		return this.#bytes;
	}
}

/**
 * Minimal `CacheableFrame` stub. Real frames carry GPU resources; tests
 * just need an object with `width`/`height` and an idempotent `.close()`.
 */
function fakeFrame(w: number, h: number, onClose?: () => void): CacheableFrame {
	return {
		width: w,
		height: h,
		close: () => onClose?.(),
	} as unknown as CacheableFrame;
}

describe('estimateFrameBytes', () => {
	it('uses width × height × 4 (RGBA)', () => {
		expect(estimateFrameBytes(fakeFrame(1920, 1080))).toBe(1920 * 1080 * 4);
		expect(estimateFrameBytes(fakeFrame(640, 480))).toBe(640 * 480 * 4);
	});
});

describe('FrameCache', () => {
	function makeCache(): { cache: FrameCache; storage: MemoryFrameStorage } {
		const storage = new MemoryFrameStorage(4 * 1024 * 1024); // 4 MB cap
		const cache = new FrameCache({ storage });
		return { cache, storage };
	}

	it('writes frames to both memory and storage', async () => {
		const { cache, storage } = makeCache();
		const frame = fakeFrame(640, 360); // 921,600 bytes
		cache.write(1_000_000, frame);
		expect(cache.readMemory(1_000_000)).toBe(frame);
		expect(await storage.get(1_000_000)).toBe(frame);
	});

	it('reads from persisted store and warms the memory cache', async () => {
		const { cache, storage } = makeCache();
		const frame = fakeFrame(640, 360);
		// Pre-seed the persisted store directly.
		await storage.put(2_000_000, frame, performance.now() * 1000);
		expect(cache.readMemory(2_000_000)).toBeNull();
		const got = await cache.readPersisted(2_000_000);
		expect(got).toBe(frame);
		// Now in memory.
		expect(cache.readMemory(2_000_000)).toBe(frame);
	});

	it('evicts on clear across both layers', async () => {
		const { cache, storage } = makeCache();
		cache.write(1, fakeFrame(640, 360));
		cache.write(2, fakeFrame(640, 360));
		await new Promise((r) => setTimeout(r, 10));
		const evicted = await cache.evictCache();
		expect(evicted).toBeGreaterThanOrEqual(2);
		expect(cache.readMemory(1)).toBeNull();
		expect(cache.readMemory(2)).toBeNull();
		expect(await storage.size()).toBe(0);
	});

	it('cacheStats reports combined memory + storage usage', async () => {
		const { cache } = makeCache();
		cache.write(1, fakeFrame(640, 360)); // 921,600
		cache.write(2, fakeFrame(640, 360));
		await new Promise((r) => setTimeout(r, 5));
		const stats = await cache.cacheStats();
		expect(stats.entryCount).toBeGreaterThan(0);
		expect(stats.bytes).toBeGreaterThan(0);
		expect(stats.capBytes).toBeGreaterThan(0);
	});

	it('replaceStorage swaps the backend without leaking the old one', async () => {
		const { cache } = makeCache();
		cache.write(1, fakeFrame(100, 100));
		const newStorage = new MemoryFrameStorage(2 * 1024 * 1024);
		cache.replaceStorage(newStorage);
		expect(cache.readMemory(1)).toBeNull(); // old memory cleared
		expect(newStorage).toBe(cache.storage);
	});
});

/**
 * Frame-lifetime regressions; each fails against the pre-fix cache. They were
 * invisible because every fixture was ImageBitmap-shaped, unlike production.
 */
describe('frame lifetime (REQUIREMENTS.md §3 memory cap, §5 ownership)', () => {
	/** `VideoFrame`-shaped stub: coded* dimensions, no width/height. */
	function fakeVideoFrame(w: number, h: number, onClose?: () => void) {
		return {
			codedWidth: w,
			codedHeight: h,
			displayWidth: w,
			displayHeight: h,
			close: () => onClose?.(),
		};
	}

	it('estimateFrameBytes handles VideoFrame dimensions (was NaN)', () => {
		// `NaN > cap` is false, so one NaN silently disabled both caps.
		const bytes = estimateFrameBytes(fakeVideoFrame(1920, 1080) as never);
		expect(Number.isNaN(bytes)).toBe(false);
		expect(bytes).toBe(1920 * 1080 * 4);
	});

	it('caps the in-memory layer and closes the frames it evicts', () => {
		const frameBytes = 640 * 360 * 4; // 921,600
		const cache = new FrameCache({
			storage: new MemoryFrameStorage(64 * 1024 * 1024),
			// Room for exactly 2 frames.
			memoryCapBytes: frameBytes * 2,
		});
		const closed: number[] = [];
		for (let i = 0; i < 5; i++) {
			cache.write(i, fakeFrame(640, 360, () => closed.push(i)), false);
		}
		// Without a cap the Map grew forever and nothing was ever closed.
		expect(cache.readMemory(0)).toBeNull();
		expect(cache.readMemory(4)).not.toBeNull();
		expect(closed).toContain(0);
		expect(closed.length).toBe(3);
	});

	it('lowering memoryCapBytes evicts immediately', () => {
		const cache = new FrameCache({
			storage: new MemoryFrameStorage(64 * 1024 * 1024),
			memoryCapBytes: 64 * 1024 * 1024,
		});
		for (let i = 0; i < 4; i++) cache.write(i, fakeFrame(640, 360), false);
		expect(cache.readMemory(3)).not.toBeNull();
		cache.memoryCapBytes = 640 * 360 * 4; // room for one
		expect(cache.readMemory(0)).toBeNull();
		expect(cache.readMemory(3)).not.toBeNull();
	});

	it('closes the outgoing frame when a key is overwritten', () => {
		const cache = new FrameCache({ storage: new MemoryFrameStorage() });
		let closed = false;
		cache.write(1, fakeFrame(64, 64, () => (closed = true)), false);
		cache.write(1, fakeFrame(64, 64), false);
		expect(closed).toBe(true);
	});

	it('replaceStorage closes held frames instead of dropping the Map', () => {
		const cache = new FrameCache({ storage: new MemoryFrameStorage() });
		let closed = false;
		cache.write(1, fakeFrame(64, 64, () => (closed = true)), false);
		cache.replaceStorage(new MemoryFrameStorage());
		expect(closed).toBe(true);
	});

	it('does not persist a VideoFrame (it is not structured-cloneable)', async () => {
		// The DataCloneError was swallowed, so nothing ever persisted.
		class FakeVideoFrame {
			codedWidth = 320;
			codedHeight = 240;
			close() {}
		}
		vi.stubGlobal('VideoFrame', FakeVideoFrame);
		try {
			const storage = new MemoryFrameStorage();
			const cache = new FrameCache({ storage });
			cache.write(1, new FakeVideoFrame() as never, true);
			await new Promise((r) => setTimeout(r, 5));
			expect(await storage.get(1)).toBeNull();
			// but the hot layer still serves it this session
			expect(cache.readMemory(1)).not.toBeNull();
		} finally {
			vi.unstubAllGlobals();
		}
	});

	it('reports memory bytes and eviction count in cacheStats', async () => {
		const frameBytes = 640 * 360 * 4;
		const cache = new FrameCache({
			storage: new MemoryFrameStorage(),
			memoryCapBytes: frameBytes * 2,
		});
		for (let i = 0; i < 4; i++) cache.write(i, fakeFrame(640, 360), false);
		const stats = await cache.cacheStats();
		expect(Number.isNaN(stats.bytes)).toBe(false);
		expect(stats.memoryBytes).toBeLessThanOrEqual(stats.memoryCapBytes);
		expect(stats.evictions).toBeGreaterThan(0);
	});
});

describe('cache factory', () => {
	it('resetFrameCache clears the singleton', () => {
		setFrameCache(new FrameCache({ storage: new MemoryFrameStorage() }));
		resetFrameCache();
		// After reset, getFrameCache() lazily rebuilds — no assertion needed,
		// the call must not throw.
		expect(() => resetFrameCache()).not.toThrow();
	});
});
/**
 * `readNearest` is what playback actually calls. Frame timestamps land on
 * presentation times while the render loop asks for arbitrary microseconds, so
 * an exact-match lookup misses every time — the bug that painted 0/120 frames.
 */
describe("FrameCache.readNearest", () => {
	function seeded() {
		const cache = new FrameCache({ storage: new MemoryFrameStorage() });
		// Frames at 0ms, 33ms, 66ms, 99ms (µs keys).
		for (const ms of [0, 33, 66, 99]) cache.write(ms * 1000, fakeFrame(64, 64), false);
		return cache;
	}

	it("returns the newest frame at or before the asked time", () => {
		const cache = seeded();
		expect(cache.readNearest(50_000)).toBe(cache.readMemory(33_000));
		expect(cache.readNearest(99_999)).toBe(cache.readMemory(99_000));
	});

	it("returns an exact hit when the key matches", () => {
		const cache = seeded();
		expect(cache.readNearest(66_000)).toBe(cache.readMemory(66_000));
	});

	it("returns null before the first frame", () => {
		expect(seeded().readNearest(-1)).toBeNull();
	});

	it("never returns a frame older than the segment floor", () => {
		// Playhead just past a cut ending at 66ms: 33ms is inside the removed
		// range, so returning it would step the picture back into cut content.
		const cache = seeded();
		expect(cache.readNearest(70_000, 66_000)).toBe(cache.readMemory(66_000));
		expect(cache.readNearest(50_000, 66_000)).toBeNull();
	});

	it("keeps the index correct after eviction", () => {
		const frameBytes = 64 * 64 * 4;
		const cache = new FrameCache({
			storage: new MemoryFrameStorage(),
			memoryCapBytes: frameBytes * 2,
		});
		for (const ms of [0, 33, 66, 99]) cache.write(ms * 1000, fakeFrame(64, 64), false);
		// Oldest two evicted; a lookup below the survivors must not resurrect them.
		expect(cache.readNearest(10_000)).toBeNull();
		expect(cache.readNearest(99_000)).not.toBeNull();
	});

	it("drops the index when the scope changes", () => {
		const cache = seeded();
		cache.setScope("recording-b");
		expect(cache.readNearest(99_000)).toBeNull();
	});
});
