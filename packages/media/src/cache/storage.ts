/**
 * Storage adapter for the decoded-frame cache. The cache persists
 * `ImageBitmap` values keyed by the original-recording timestamp (µs).
 * Implementations are pluggable so we can swap IndexedDB (default,
 * works everywhere including Tauri's webview) for SQLite (Tauri-only,
 * for cases where the webview quota or platform behavior is undesirable).
 *
 * Contract (packages/media/REQUIREMENTS.md §5):
 * - The cache key is a microsecond timestamp (`ctsUs`).
 * - The cache value is an `ImageBitmap` (structured-cloneable, GPU-ready,
 *   accepted by `texImage2D`).
 * - All async methods accept an `AbortSignal` so callers can cancel.
 * - Storage backends MUST honor the byte cap; over-cap writes trigger LRU
 *   eviction (recency × bytes) on the same call.
 * - Storage backends MUST NOT throw on quota errors during normal use; they
 *   should evict until the write fits.
 *
 * The "recency" timestamp is supplied by the caller (`lastUsedUs` on
 * `put`) so the backend doesn't have to track it itself — this keeps
 * implementations simple and lets the orchestrator coordinate eviction
 * across multiple stores (memory + persisted).
 */

/** A decoded video frame, GPU-ready, structured-cloneable for IndexedDB. */
export type CacheableFrame = ImageBitmap;

/**
 * What the in-memory hot layer may hold. `VideoFrame` is transferable but
 * NOT structured-cloneable, so it can live in memory but can never reach
 * IndexedDB — `isPersistable` is the guard that keeps the two apart.
 */
export type CachedFrame = ImageBitmap | VideoFrame;

/**
 * True when the frame can be written to IndexedDB. `VideoFrame` fails
 * structured-clone with `DataCloneError`; `ImageBitmap` succeeds.
 */
export function isPersistable(frame: CachedFrame): frame is CacheableFrame {
	return typeof VideoFrame === 'undefined' || !(frame instanceof VideoFrame);
}

/** Per-entry byte estimate for budget accounting. */
export function estimateFrameBytes(frame: CachedFrame): number {
	// `ImageBitmap` exposes width/height; `VideoFrame` exposes codedWidth/
	// codedHeight and has NO width/height — reading those yields undefined and
	// poisons every downstream byte total with NaN. Branch on what's present.
	const w = 'codedWidth' in frame ? frame.codedWidth : frame.width;
	const h = 'codedHeight' in frame ? frame.codedHeight : frame.height;
	if (!Number.isFinite(w) || !Number.isFinite(h)) return 0;
	// No public `byteLength`; use nominal pixel size × 4 channels (RGBA).
	// Sub-byte encodings underestimate, but for eviction an over-estimate is
	// the safe side.
	return w * h * 4;
}

/**
 * Storage backend contract. Implementations: `IndexedDBFrameStorage`
 * (default) and `SqliteFrameStorage` (Tauri-only, stub for now).
 */
export interface FrameStorage {
	/** Display name for diagnostics + telemetry. */
	readonly name: string;

	/** Open the underlying store. Resolves when ready for reads/writes. */
	open(): Promise<void>;

	/**
	 * Read a single entry. Returns `null` when the key is missing or the
	 * stored value can't be decoded (corrupted entry — caller should
	 * consider treating it as an eviction).
	 */
	get(key: number, signal?: AbortSignal): Promise<CacheableFrame | null>;

	/**
	 * Write a single entry, then evict the least-recently-used entries
	 * until total size ≤ `capBytes`. No-op if the entry already exists with
	 * a more-recent `lastUsedUs`.
	 *
	 * `lastUsedUs` is recorded for LRU ordering; the orchestrator passes
	 * `performance.now()` (in microseconds) on every read/write.
	 */
	put(key: number, frame: CacheableFrame, lastUsedUs: number, signal?: AbortSignal): Promise<void>;

	/** Delete entries with key in `[startKey, endKey)` (half-open). */
	deleteRange(startKey: number, endKey: number, signal?: AbortSignal): Promise<void>;

	/** Clear every entry. Used by `evictCache` (Settings → reset cache). */
	clear(signal?: AbortSignal): Promise<void>;

	/** Current total bytes across all entries. */
	size(signal?: AbortSignal): Promise<number>;

	/** Configured byte cap. The orchestrator may resize via `capBytes = …`. */
	capBytes: number;

	/** Close the underlying store. Subsequent calls throw. */
	close(): Promise<void>;
}

/**
 * Sentinel marker that storage backends can throw to indicate the call
 * was cancelled. The orchestrator catches this and rethrows as
 * `MediaError(code: 'cancelled')` so callers can use a single try/catch.
 */
export class StorageAbortedError extends Error {
	constructor() {
		super('Storage operation aborted');
		this.name = 'StorageAbortedError';
	}
}
