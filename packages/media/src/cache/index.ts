import { IndexedDBFrameStorage } from './indexeddb-storage';
import {
	type CacheableFrame,
	type CachedFrame,
	estimateFrameBytes,
	type FrameStorage,
	isPersistable,
} from './storage';

/**
 * Tunables. The persistent cap is configurable in Settings (REQUIREMENTS.md §3).
 */
const DEFAULT_PERSISTENT_CAP_BYTES = 2 * 1024 * 1024 * 1024;

/**
 * Hard cap on the in-memory hot layer (REQUIREMENTS.md §3, "Decoded-frame
 * memory ≤ 512 MB"). Every decoded frame holds a GPU-backed surface, so an
 * uncapped Map starves the decoder and grows without bound across a long
 * editing session. Insertion evicts LRU until the incoming frame fits.
 */
const DEFAULT_MEMORY_CAP_BYTES = 512 * 1024 * 1024;

export interface FrameCacheConfig {
	storage: FrameStorage;
	/** Optional cap override (bytes). Defaults to the storage's own cap. */
	capBytes?: number;
	/** In-memory hot-layer cap (bytes). Defaults to 512 MB. */
	memoryCapBytes?: number;
}

export interface CacheStats {
	entryCount: number;
	bytes: number;
	capBytes: number;
	oldestEntryAgeMs: number;
	/** In-memory hot-layer bytes only (excludes the persistent store). */
	memoryBytes: number;
	/** In-memory hot-layer cap. */
	memoryCapBytes: number;
	/** Frames closed by LRU eviction since the cache was created. */
	evictions: number;
}

interface InMemoryEntry {
	frame: CachedFrame;
	lastUsedUs: number;
	size: number;
}

/**
 * Singleton holder. Tests reset this via `resetFrameCache()`.
 */
let current: FrameCache | null = null;

/**
 * Build the default cache. IndexedDB is the standard offline-state mechanism
 * for both browsers and Tauri webviews (WebView2 / WebKit / WebKitGTK). The
 * orchestrator auto-detects availability and falls back to an in-memory
 * no-op store if `indexedDB` is absent (Node tests).
 */
function createDefaultCache(): FrameCache {
	const hasIndexedDB = typeof globalThis !== 'undefined' && 'indexedDB' in globalThis;
	if (!hasIndexedDB) {
		// Tests / non-browser environments: ship a no-op storage so the
		// orchestrator still works. Real consumers in apps/desktop will get
		// the IDB path.
		return new FrameCache({
			storage: makeInMemoryStorage(),
			capBytes: DEFAULT_PERSISTENT_CAP_BYTES,
		});
	}
	return new FrameCache({
		storage: new IndexedDBFrameStorage({ capBytes: DEFAULT_PERSISTENT_CAP_BYTES }),
	});
}

/**
 * No-op storage used when `indexedDB` is unavailable (Node tests).
 * In-memory only; resets when the cache instance is GC'd.
 */
function makeInMemoryStorage(): FrameStorage {
	const map = new Map<number, { frame: CacheableFrame; size: number; lastUsedUs: number }>();
	let bytes = 0;
	let cap = DEFAULT_PERSISTENT_CAP_BYTES;
	return {
		name: 'memory',
		async open() {
			/* no-op */
		},
		async get(key) {
			return map.get(key)?.frame ?? null;
		},
		async put(key, frame, lastUsedUs) {
			const size = estimateFrameBytes(frame);
			map.set(key, { frame, size, lastUsedUs });
			bytes += size;
		},
		async deleteRange(start, end) {
			for (const key of [...map.keys()]) {
				if (key >= start && key < end) {
					const entry = map.get(key);
					if (entry) {
						map.delete(key);
						bytes -= entry.size;
					}
				}
			}
		},
		async clear() {
			map.clear();
			bytes = 0;
		},
		async size() {
			return bytes;
		},
		async close() {
			/* no-op */
		},
		get capBytes() {
			return cap;
		},
		set capBytes(v: number) {
			cap = v;
		},
	};
}

export function getFrameCache(): FrameCache {
	if (!current) current = createDefaultCache();
	return current;
}

export function setFrameStorage(storage: FrameStorage): void {
	if (current) {
		current.replaceStorage(storage);
		return;
	}
	current = new FrameCache({ storage });
}

export function setFrameCache(cache: FrameCache): void {
	current = cache;
}

export function resetFrameCache(): void {
	current = null;
}

export class FrameCache {
	#memory = new Map<number, InMemoryEntry>();
	#stats = { entryCount: 0, bytes: 0, oldestEntryUs: -1, evictions: 0 };
	#storage: FrameStorage;
	#memoryCap: number;
	#scope: string | null = null;

	constructor(config: FrameCacheConfig) {
		this.#storage = config.storage;
		this.#memoryCap = config.memoryCapBytes ?? DEFAULT_MEMORY_CAP_BYTES;
		if (config.capBytes !== undefined) this.#storage.capBytes = config.capBytes;
	}

	/**
	 * Bind the cache to one media source.
	 *
	 * Entries are keyed by bare presentation timestamp, and `getFrameCache()`
	 * is a process-wide singleton — so without a scope, opening recording B
	 * after A makes A's frame at t=5s answer B's read at t=5s, painting the
	 * wrong video. Changing scope closes and drops the previous source's
	 * frames. Re-setting the same scope is a no-op, so repeated calls from
	 * multiple sources over one recording stay cheap.
	 */
	setScope(scope: string): void {
		if (this.#scope === scope) return;
		this.#scope = scope;
		for (const entry of this.#memory.values()) entry.frame.close();
		this.#memory.clear();
		this.#stats = {
			entryCount: 0,
			bytes: 0,
			oldestEntryUs: -1,
			evictions: this.#stats.evictions,
		};
	}

	/** Currently-bound source, or null before the first `setScope`. */
	get scope(): string | null {
		return this.#scope;
	}

	/** In-memory hot-layer cap (bytes). Lowering it evicts on the next write. */
	get memoryCapBytes(): number {
		return this.#memoryCap;
	}

	set memoryCapBytes(value: number) {
		this.#memoryCap = value;
		this.#evictMemoryUntilFits(0);
	}

	replaceStorage(storage: FrameStorage): void {
		void this.#storage.close().catch(() => {
			/* best-effort */
		});
		// Close before dropping the Map, or every held surface leaks.
		for (const entry of this.#memory.values()) entry.frame.close();
		this.#memory.clear();
		this.#stats = { entryCount: 0, bytes: 0, oldestEntryUs: -1, evictions: 0 };
		this.#storage = storage;
	}

	get storage(): FrameStorage {
		return this.#storage;
	}

	/**
	 * Read a frame from the in-memory cache (fast path). Returns null on
	 * miss; callers fall through to the persistent store or a worker
	 * decode.
	 */
	readMemory(tsUs: number): CachedFrame | null {
		const entry = this.#memory.get(tsUs);
		if (!entry) return null;
		entry.lastUsedUs = performance.now() * 1000;
		return entry.frame;
	}

	/**
	 * Read from the persistent store and warm the in-memory cache on hit.
	 */
	async readPersisted(tsUs: number, signal?: AbortSignal): Promise<CacheableFrame | null> {
		const bitmap = await this.#storage.get(tsUs, signal);
		if (!bitmap) return null;
		// Warm the in-memory cache so the next read is instant.
		this.#memoryInsert(tsUs, bitmap);
		return bitmap;
	}

	/**
	 * Store a frame both in memory and on disk. The persistent write is
	 * best-effort and isolated from the read path: failures are logged
	 * and the in-memory entry still wins for the current session.
	 */
	write(tsUs: number, frame: CachedFrame, persist = true): void {
		this.#memoryInsert(tsUs, frame);
		if (!persist) return;
		// A `VideoFrame` is transferable but not structured-cloneable, so an
		// IndexedDB put rejects with DataCloneError. That rejection used to be
		// swallowed by the catch below, which meant the persistent layer
		// silently stored nothing on the desktop path. Skip it honestly
		// instead: the hot layer still serves the frame this session.
		if (!isPersistable(frame)) return;
		// Fire and forget — the orchestrator awaits this when callers need
		// durability (e.g. before opening another recording).
		void this.#storage.put(tsUs, frame, performance.now() * 1000).catch((err) => {
			console.warn('[recast/media] persist frame failed:', err);
		});
	}

	/** Delete a single key from both stores. */
	async evict(tsUs: number): Promise<void> {
		const entry = this.#memory.get(tsUs);
		if (entry) {
			entry.frame.close();
			this.#memory.delete(tsUs);
			this.#recomputeStats();
		}
		try {
			await this.#storage.deleteRange(tsUs, tsUs + 1);
		} catch {
			/* best-effort */
		}
	}

	/** Clear every entry in both stores. Used by Settings → reset cache. */
	async clear(): Promise<void> {
		for (const entry of this.#memory.values()) entry.frame.close();
		this.#memory.clear();
		this.#stats = { entryCount: 0, bytes: 0, oldestEntryUs: -1, evictions: this.#stats.evictions };
		await this.#storage.clear();
	}

	/**
	 * Explicit eviction hook. Called by `evictCache` (REQUIREMENTS.md §2)
	 * and by idle-callback GC in the editor.
	 */
	async evictCache(): Promise<number> {
		const before = this.#memory.size;
		for (const entry of this.#memory.values()) entry.frame.close();
		this.#memory.clear();
		await this.#storage.clear();
		this.#stats = { entryCount: 0, bytes: 0, oldestEntryUs: -1, evictions: this.#stats.evictions };
		return before;
	}

	async cacheStats(): Promise<CacheStats> {
		const bytes = this.#stats.bytes + (await this.#storage.size());
		const oldest = this.#stats.oldestEntryUs;
		const ageMs = oldest > 0 ? (performance.now() * 1000 - oldest) / 1000 : 0;
		return {
			entryCount: this.#stats.entryCount,
			bytes,
			capBytes: this.#storage.capBytes,
			oldestEntryAgeMs: ageMs,
			memoryBytes: this.#stats.bytes,
			memoryCapBytes: this.#memoryCap,
			evictions: this.#stats.evictions,
		};
	}

	#memoryInsert(tsUs: number, frame: CachedFrame): void {
		const size = estimateFrameBytes(frame);
		// Replacing an existing key: close the outgoing frame first, or its
		// surface leaks silently.
		const existing = this.#memory.get(tsUs);
		if (existing) {
			if (existing.frame !== frame) existing.frame.close();
			this.#memory.delete(tsUs);
			this.#stats.bytes = Math.max(0, this.#stats.bytes - existing.size);
		}
		this.#evictMemoryUntilFits(size);
		const entry: InMemoryEntry = {
			frame,
			lastUsedUs: performance.now() * 1000,
			size,
		};
		this.#memory.set(tsUs, entry);
		this.#stats.entryCount = this.#memory.size;
		this.#stats.bytes += size;
		const us = entry.lastUsedUs;
		if (this.#stats.oldestEntryUs < 0 || us < this.#stats.oldestEntryUs) {
			this.#stats.oldestEntryUs = us;
		}
	}

	/**
	 * Evict least-recently-used entries until `incomingSize` fits under the
	 * memory cap. Every evicted frame is closed — per Chrome's WebCodecs
	 * guidance, a decoded frame holds a GPU surface that the GC will not
	 * reclaim promptly on its own.
	 */
	#evictMemoryUntilFits(incomingSize: number): void {
		if (this.#stats.bytes + incomingSize <= this.#memoryCap) return;
		// Oldest-first by last read. Map iteration order is insertion order,
		// which is not read order, so sort explicitly.
		const byAge = [...this.#memory.entries()].sort(
			(a, b) => a[1].lastUsedUs - b[1].lastUsedUs,
		);
		for (const [key, entry] of byAge) {
			if (this.#stats.bytes + incomingSize <= this.#memoryCap) break;
			entry.frame.close();
			this.#memory.delete(key);
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