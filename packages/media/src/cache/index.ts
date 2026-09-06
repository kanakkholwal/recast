import { type CachedFrame, estimateFrameBytes } from "./storage";

// Each frame holds a GPU surface, so an uncapped Map is a leak, not a cache. See REQUIREMENTS.md section 3.
const DEFAULT_MEMORY_CAP_BYTES = 512 * 1024 * 1024;

export interface FrameCacheConfig {
	/** Cap in bytes. Defaults to 512 MB; `frameCacheCapBytes` sizes it per source. */
	memoryCapBytes?: number;
}

export interface CacheStats {
	entryCount: number;
	bytes: number;
	oldestEntryAgeMs: number;
	capBytes: number;
	/** Frames closed by LRU eviction since the cache was created. */
	evictions: number;
}

interface InMemoryEntry {
	frame: CachedFrame;
	lastUsedUs: number;
	size: number;
}

/** Singleton holder. Tests reset this via `resetFrameCache()`. */
let current: FrameCache | null = null;

export function getFrameCache(): FrameCache {
	if (!current) current = new FrameCache();
	return current;
}

export function setFrameCache(cache: FrameCache): void {
	current = cache;
}

export function resetFrameCache(): void {
	current = null;
}

/**
 * In-memory decoded-frame cache, capped in bytes and evicted by distance from
 * the playhead. Memory only: a `VideoFrame` is not structured-cloneable, so
 * every frame the preview decodes was rejected by the persistent layer that
 * used to sit behind this — that layer is gone.
 */
export class FrameCache {
	#memory = new Map<number, InMemoryEntry>();
	/** Keys of `#memory`, ascending. Backs `readNearest`'s binary search. */
	#sorted: number[] = [];
	/** Last timestamp asked for; eviction keeps frames near it. */
	#lastReadUs = -1;
	#stats = { entryCount: 0, bytes: 0, oldestEntryUs: -1, evictions: 0 };
	#memoryCap: number;
	#scope: string | null = null;

	constructor(config: FrameCacheConfig = {}) {
		this.#memoryCap = config.memoryCapBytes ?? DEFAULT_MEMORY_CAP_BYTES;
	}

	/**
	 * Bind the cache to one media source, closing the previous source's frames.
	 * Without this, recording B reads recording A's frame at the same timestamp.
	 */
	setScope(scope: string): void {
		if (this.#scope === scope) return;
		this.#scope = scope;
		this.#dropAll();
	}

	/** Currently-bound source, or null before the first `setScope`. */
	get scope(): string | null {
		return this.#scope;
	}

	/** Cap in bytes. Lowering it evicts on the next write. */
	get memoryCapBytes(): number {
		return this.#memoryCap;
	}

	set memoryCapBytes(value: number) {
		this.#memoryCap = value;
		this.#evictMemoryUntilFits(0);
	}

	/**
	 * Read a frame by exact timestamp. Returns null on miss; callers fall
	 * through to a worker decode.
	 */
	readMemory(tsUs: number): CachedFrame | null {
		const entry = this.#memory.get(tsUs);
		if (!entry) return null;
		entry.lastUsedUs = performance.now() * 1000;
		return entry.frame;
	}

	/**
	 * Newest frame at or before `tsUs`, no older than `floorUs`. This is the
	 * lookup playback needs: frame timestamps land on presentation times, not
	 * on whatever microsecond the render loop happens to ask for, so an exact
	 * match essentially never hits.
	 *
	 * `floorUs` is the start of the current kept segment. Returning anything
	 * before it would step the picture back into content the user cut.
	 */
	readNearest(tsUs: number, floorUs = 0): CachedFrame | null {
		// Record it even on a miss: eviction needs the playhead's position before the first successful read.
		this.#lastReadUs = tsUs;
		const idx = this.#floorIndex(tsUs);
		if (idx < 0) return null;
		const key = this.#sorted[idx];
		if (key === undefined || key < floorUs) return null;
		const entry = this.#memory.get(key);
		if (!entry) return null;
		entry.lastUsedUs = performance.now() * 1000;
		return entry.frame;
	}

	/** Index in `#sorted` of the greatest key ≤ `tsUs`, or -1. */
	#floorIndex(tsUs: number): number {
		let lo = 0;
		let hi = this.#sorted.length - 1;
		let best = -1;
		while (lo <= hi) {
			const mid = (lo + hi) >> 1;
			const k = this.#sorted[mid] as number;
			if (k <= tsUs) {
				best = mid;
				lo = mid + 1;
			} else {
				hi = mid - 1;
			}
		}
		return best;
	}

	/** Insertion point for `key` in the sorted index. */
	#insertIndex(key: number): number {
		let lo = 0;
		let hi = this.#sorted.length;
		while (lo < hi) {
			const mid = (lo + hi) >> 1;
			if ((this.#sorted[mid] as number) < key) lo = mid + 1;
			else hi = mid;
		}
		return lo;
	}

	#indexInsert(key: number): void {
		const at = this.#insertIndex(key);
		if (this.#sorted[at] === key) return;
		this.#sorted.splice(at, 0, key);
	}

	#indexRemove(key: number): void {
		const at = this.#insertIndex(key);
		if (this.#sorted[at] === key) this.#sorted.splice(at, 1);
	}

	write(tsUs: number, frame: CachedFrame): void {
		this.#memoryInsert(tsUs, frame);
	}

	/** Delete a single key. */
	evict(tsUs: number): void {
		const entry = this.#memory.get(tsUs);
		if (!entry) return;
		entry.frame.close();
		this.#memory.delete(tsUs);
		this.#indexRemove(tsUs);
		this.#recomputeStats();
	}

	/** Clear every entry. Used by Settings → reset cache. */
	clear(): void {
		this.#dropAll();
	}

	/**
	 * Explicit eviction hook. Called by `evictCache` (REQUIREMENTS.md §2)
	 * and by idle-callback GC in the editor. Returns the entries dropped.
	 */
	evictCache(): number {
		const before = this.#memory.size;
		this.#dropAll();
		return before;
	}

	cacheStats(): CacheStats {
		const oldest = this.#stats.oldestEntryUs;
		const ageMs = oldest > 0 ? (performance.now() * 1000 - oldest) / 1000 : 0;
		return {
			entryCount: this.#stats.entryCount,
			bytes: this.#stats.bytes,
			oldestEntryAgeMs: ageMs,
			capBytes: this.#memoryCap,
			evictions: this.#stats.evictions,
		};
	}

	/** Close every held surface. Dropping the Map alone leaks all of them. */
	#dropAll(): void {
		for (const entry of this.#memory.values()) entry.frame.close();
		this.#memory.clear();
		this.#sorted = [];
		this.#stats = {
			entryCount: 0,
			bytes: 0,
			oldestEntryUs: -1,
			evictions: this.#stats.evictions,
		};
	}

	#memoryInsert(tsUs: number, frame: CachedFrame): void {
		const size = estimateFrameBytes(frame);
		// Close the outgoing frame or its surface leaks silently.
		const existing = this.#memory.get(tsUs);
		if (existing) {
			if (existing.frame !== frame) existing.frame.close();
			this.#memory.delete(tsUs);
			this.#indexRemove(tsUs);
			this.#stats.bytes = Math.max(0, this.#stats.bytes - existing.size);
		}
		this.#evictMemoryUntilFits(size);
		const entry: InMemoryEntry = {
			frame,
			lastUsedUs: performance.now() * 1000,
			size,
		};
		this.#memory.set(tsUs, entry);
		this.#indexInsert(tsUs);
		this.#stats.entryCount = this.#memory.size;
		this.#stats.bytes += size;
		const us = entry.lastUsedUs;
		if (this.#stats.oldestEntryUs < 0 || us < this.#stats.oldestEntryUs) {
			this.#stats.oldestEntryUs = us;
		}
	}

	/**
	 * Eviction cost, highest goes first. Distance from the playhead, with frames
	 * BEHIND it penalised — forward playback never needs those again.
	 *
	 * Plain LRU is actively wrong here: decode-ahead frames have never been read,
	 * so they were always the oldest-used and got evicted just before the
	 * playhead reached them. The decoder then re-decoded them, and the picture
	 * updated a fraction as often as it should.
	 */
	#evictionCost(tsUs: number): number {
		if (this.#lastReadUs < 0) return -tsUs;
		const delta = tsUs - this.#lastReadUs;
		return delta >= 0 ? delta : -delta * 4;
	}

	/**
	 * Evict until `incomingSize` fits under the cap, closing each frame.
	 * The GC will not reclaim a decoded frame's GPU surface promptly.
	 */
	#evictMemoryUntilFits(incomingSize: number): void {
		if (this.#stats.bytes + incomingSize <= this.#memoryCap) return;
		const byCost = [...this.#memory.entries()].sort(
			(a, b) => this.#evictionCost(b[0]) - this.#evictionCost(a[0]),
		);
		for (const [key, entry] of byCost) {
			if (this.#stats.bytes + incomingSize <= this.#memoryCap) break;
			entry.frame.close();
			this.#memory.delete(key);
			this.#indexRemove(key);
			this.#stats.bytes = Math.max(0, this.#stats.bytes - entry.size);
			this.#stats.evictions++;
		}
		this.#recomputeStats();
	}

	#recomputeStats(): void {
		this.#stats.entryCount = this.#memory.size;
		let bytes = 0;
		let oldest = -1;
		for (const entry of this.#memory.values()) {
			bytes += entry.size;
			if (oldest < 0 || entry.lastUsedUs < oldest) oldest = entry.lastUsedUs;
		}
		this.#stats.bytes = bytes;
		this.#stats.oldestEntryUs = oldest;
	}
}
