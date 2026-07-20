/**
 * IndexedDB-backed implementation of `FrameStorage`. Default storage for
 * the decoded-frame cache. Works in:
 *   - Browsers (apps/web tools, future in-browser editor)
 *   - Tauri webview on Windows (WebView2), macOS (WebKit), Linux (WebKitGTK)
 *
 * Layout:
 *   - One IDB database per recording (recording-scoped, cleaned on dispose)
 *     OR one global DB with a `recordingId` key prefix (simpler — pick this
 *     for PR-E; PR-F can split if recordings get very large).
 *   - Two object stores:
 *       `frames` — `{ ts: number, bitmap: ImageBitmap, size: number, lastUsedUs: number }`,
 *                  keyed by `ts`
 *       `meta`  — singleton `{ id: 'singleton', size: number, cap: number }` for
 *                  cheap size + cap reads.
 *
 * Quota notes (Tauri / webview):
 *   - WebView2 (Windows): ~80% of free disk per origin. Easily handles 2 GB.
 *   - WebKit (macOS): ~1 GB default; the orchestrator can call
 *     `navigator.storage.persist()` once at open time to avoid eviction
 *     under disk pressure.
 *   - WebKitGTK (Linux): smaller default; same `persist()` call helps.
 *
 * Errors: on `QuotaExceededError`, the orchestrator's `put` retries with
 * forced eviction; this implementation always writes atomically and lets
 * the orchestrator handle backpressure.
 */

import { type CacheableFrame, estimateFrameBytes, type FrameStorage } from './storage';

/**
 * `globalThis.indexedDB` is typed in `lib.dom.d.ts`, which isn't loaded
 * when this package runs under Node tests. Re-declare a minimal shape
 * so the implementation type-checks in both environments.
 */
type MinimalIDB = {
	open: (name: string, version: number) => IDBOpenDBRequest;
};

function getIndexedDB(): MinimalIDB | null {
	if (typeof globalThis === 'undefined') return null;
	const idb = (globalThis as { indexedDB?: MinimalIDB }).indexedDB;
	return idb ?? null;
}

const DB_NAME = 'recast-media-cache';
const DB_VERSION = 1;
const STORE_FRAMES = 'frames';
const STORE_META = 'meta';
const _META_KEY = 'singleton';

interface StoredFrame {
	ts: number;
	bitmap: ImageBitmap;
	size: number;
	lastUsedUs: number;
}

interface MetaRecord {
	id: 'singleton';
	size: number;
}

export interface IndexedDBFrameStorageOptions {
	/** Logical namespace, e.g. recording id. Lets one DB hold many recordings. */
	recordingId?: string;
	/** Default 2 GB. The orchestrator may overwrite this after `open()`. */
	capBytes?: number;
}

export class IndexedDBFrameStorage implements FrameStorage {
	readonly name = 'indexeddb';
	#db: IDBDatabase | null = null;
	#recordingId: string;
	#size = 0;
	#cap: number;
	#open: Promise<void> | null = null;

	constructor(options: IndexedDBFrameStorageOptions = {}) {
		this.#recordingId = options.recordingId ?? 'default';
		this.#cap = options.capBytes ?? 2 * 1024 * 1024 * 1024;
	}

	get capBytes(): number {
		return this.#cap;
	}

	set capBytes(value: number) {
		this.#cap = value;
	}

	async open(): Promise<void> {
		if (this.#db) return;
		if (this.#open) return this.#open;
		this.#open = this.#openInternal();
		return this.#open;
	}

	#openInternal(): Promise<void> {
		return new Promise((resolve, reject) => {
			const idb = getIndexedDB();
			if (!idb) {
				reject(new Error('IndexedDB is not available in this environment'));
				return;
			}
			const req = idb.open(DB_NAME, DB_VERSION);
			req.onupgradeneeded = () => {
				const db = req.result;
				if (!db.objectStoreNames.contains(STORE_FRAMES)) {
					db.createObjectStore(STORE_FRAMES, { keyPath: 'ts' });
				}
				if (!db.objectStoreNames.contains(STORE_META)) {
					db.createObjectStore(STORE_META, { keyPath: 'id' });
				}
			};
			req.onsuccess = () => {
				this.#db = req.result;
				this.#db.onversionchange = () => {
					// Another tab upgraded; close our handle so we don't go stale.
					this.#db?.close();
					this.#db = null;
				};
				// Best-effort persistence request (no-op if already granted).
				if (typeof navigator !== 'undefined' && navigator.storage?.persist) {
					void navigator.storage.persist().catch(() => {
						/* persistence is best-effort */
					});
				}
				resolve();
			};
			req.onerror = () => reject(req.error ?? new Error('IndexedDB.open failed'));
			req.onblocked = () => reject(new Error('IndexedDB.open blocked by another tab'));
		});
	}

	async get(key: number, signal?: AbortSignal): Promise<CacheableFrame | null> {
		await this.open();
		if (signal?.aborted) throw new DOMException('Aborted', 'AbortError');
		return new Promise((resolve, reject) => {
			const db = this.#db;
			if (!db) {
				reject(new Error('IndexedDB not open'));
				return;
			}
			const tx = db.transaction(STORE_FRAMES, 'readonly');
			const req = tx.objectStore(STORE_FRAMES).get(key);
			const onAbort = () => {
				reject(new DOMException('Aborted', 'AbortError'));
			};
			signal?.addEventListener('abort', onAbort, { once: true });
			req.onsuccess = () => {
				signal?.removeEventListener('abort', onAbort);
				const row = req.result as StoredFrame | undefined;
				resolve(row?.bitmap ?? null);
			};
			req.onerror = () => {
				signal?.removeEventListener('abort', onAbort);
				reject(req.error ?? new Error('IndexedDB.get failed'));
			};
		});
	}

	async put(
		key: number,
		frame: CacheableFrame,
		lastUsedUs: number,
		signal?: AbortSignal,
	): Promise<void> {
		const _db = await this.open();
		if (signal?.aborted) throw new DOMException('Aborted', 'AbortError');
		const size = estimateFrameBytes(frame);
		// Compute the post-write size, evict LRU entries until it fits.
		const targetSize = this.#size + size;
		if (targetSize > this.#cap) {
			await this.#evictUntilFits(size, signal);
		}
		// Single write, atomic — either the whole entry lands or the store
		// stays untouched (the browser may still throw QuotaExceededError
		// on a too-large value; the orchestrator handles eviction retries).
		await this.#write(key, { ts: key, bitmap: frame, size, lastUsedUs }, signal);
		this.#size += size;
	}

	async deleteRange(startKey: number, endKey: number, signal?: AbortSignal): Promise<void> {
		await this.open();
		if (signal?.aborted) throw new DOMException('Aborted', 'AbortError');
		await new Promise<void>((resolve, reject) => {
			const db = this.#db;
			if (!db) {
				reject(new Error('IndexedDB not open'));
				return;
			}
			const tx = db.transaction(STORE_FRAMES, 'readwrite');
			const store = tx.objectStore(STORE_FRAMES);
			const range = IDBKeyRange.bound(startKey, endKey, false, true);
			const req = store.openCursor(range);
			const onAbort = () => reject(new DOMException('Aborted', 'AbortError'));
			signal?.addEventListener('abort', onAbort, { once: true });
			const removed: number[] = [];
			req.onsuccess = () => {
				const cursor = req.result;
				if (cursor) {
					removed.push((cursor.value as StoredFrame).size);
					cursor.delete();
					cursor.continue();
				} else {
					tx.oncomplete = () => {
						signal?.removeEventListener('abort', onAbort);
						for (const s of removed) this.#size = Math.max(0, this.#size - s);
						resolve();
					};
					tx.onerror = () => {
						signal?.removeEventListener('abort', onAbort);
						reject(tx.error ?? new Error('IndexedDB.deleteRange failed'));
					};
				}
			};
			req.onerror = () => {
				signal?.removeEventListener('abort', onAbort);
				reject(req.error ?? new Error('IndexedDB.openCursor failed'));
			};
		});
	}

	async clear(signal?: AbortSignal): Promise<void> {
		await this.open();
		if (signal?.aborted) throw new DOMException('Aborted', 'AbortError');
		await new Promise<void>((resolve, reject) => {
			const db = this.#db;
			if (!db) {
				reject(new Error('IndexedDB not open'));
				return;
			}
			const tx = db.transaction([STORE_FRAMES, STORE_META], 'readwrite');
			tx.objectStore(STORE_FRAMES).clear();
			tx.objectStore(STORE_META).clear();
			const onAbort = () => reject(new DOMException('Aborted', 'AbortError'));
			signal?.addEventListener('abort', onAbort, { once: true });
			tx.oncomplete = () => {
				signal?.removeEventListener('abort', onAbort);
				this.#size = 0;
				resolve();
			};
			tx.onerror = () => {
				signal?.removeEventListener('abort', onAbort);
				reject(tx.error ?? new Error('IndexedDB.clear failed'));
			};
		});
	}

	async size(_signal?: AbortSignal): Promise<number> {
		await this.open();
		return this.#size;
	}

	async close(): Promise<void> {
		if (this.#db) {
			this.#db.close();
			this.#db = null;
		}
	}

	#write(_key: number, row: StoredFrame, signal?: AbortSignal): Promise<void> {
		return new Promise((resolve, reject) => {
			const db = this.#db;
			if (!db) {
				reject(new Error('IndexedDB not open'));
				return;
			}
			const tx = db.transaction(STORE_FRAMES, 'readwrite');
			const _req = tx.objectStore(STORE_FRAMES).put(row);
			const onAbort = () => reject(new DOMException('Aborted', 'AbortError'));
			signal?.addEventListener('abort', onAbort, { once: true });
			tx.oncomplete = () => {
				signal?.removeEventListener('abort', onAbort);
				resolve();
			};
			tx.onerror = () => {
				signal?.removeEventListener('abort', onAbort);
				reject(tx.error ?? new Error('IndexedDB.put failed'));
			};
		});
	}

	/**
	 * Evict LRU entries (oldest `lastUsedUs` first) until `incomingSize`
	 * bytes would fit. Single read-write transaction so eviction is
	 * atomic with the in-flight `put`.
	 */
	#evictUntilFits(incomingSize: number, signal?: AbortSignal): Promise<void> {
		return new Promise((resolve, reject) => {
			const db = this.#db;
			if (!db) {
				reject(new Error('IndexedDB not open'));
				return;
			}
			const tx = db.transaction(STORE_FRAMES, 'readwrite');
			const store = tx.objectStore(STORE_FRAMES);
			const cursorReq = store.index ? store.index('lastUsedUs').openCursor() : store.openCursor();
			const onAbort = () => reject(new DOMException('Aborted', 'AbortError'));
			signal?.addEventListener('abort', onAbort, { once: true });
			let evicted = 0;
			cursorReq.onsuccess = () => {
				const cursor = cursorReq.result;
				if (cursor && this.#size + incomingSize - evicted > this.#cap) {
					evicted += (cursor.value as StoredFrame).size;
					cursor.delete();
					cursor.continue();
				} else {
					tx.oncomplete = () => {
						signal?.removeEventListener('abort', onAbort);
						this.#size = Math.max(0, this.#size - evicted);
						resolve();
					};
					tx.onerror = () => {
						signal?.removeEventListener('abort', onAbort);
						reject(tx.error ?? new Error('IndexedDB.evict failed'));
					};
				}
			};
			cursorReq.onerror = () => {
				signal?.removeEventListener('abort', onAbort);
				reject(cursorReq.error ?? new Error('IndexedDB.openCursor failed'));
			};
		});
	}
}
