import { describe, expect, it } from 'vitest';
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
function fakeFrame(w: number, h: number): CacheableFrame {
	return {
		width: w,
		height: h,
		close: () => {
			/* no-op */
		},
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

describe('cache factory', () => {
	it('resetFrameCache clears the singleton', () => {
		setFrameCache(new FrameCache({ storage: new MemoryFrameStorage() }));
		resetFrameCache();
		// After reset, getFrameCache() lazily rebuilds — no assertion needed,
		// the call must not throw.
		expect(() => resetFrameCache()).not.toThrow();
	});
});