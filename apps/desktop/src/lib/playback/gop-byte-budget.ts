/**
 * LRU cache accounting for the WebCodecs progressive path, governed by a BYTE
 * budget over the *encoded* GOP bytes we've fetched.
 *
 * Separate from the decoded-frame cache: decoded `VideoFrame`s must be bounded
 * by a small COUNT (each holds one of the decoder's limited output surfaces),
 * whereas encoded GOP bytes are cheap CPU RAM, so a generous byte ceiling lets
 * scrubbing back and forth over the same cut stay re-fetch-free.
 *
 * Pure accounting only. The caller owns the actual bytes (sample `.data`) and
 * frees the keys this returns.
 */
export class GopByteBudget {
	/** Resident GOPs keyed by keyframe decode-index → byte size. Insertion order
	 * is LRU order: least-recently-touched first, most-recent last. */
	#bytesByGop = new Map<number, number>();
	#total = 0;
	readonly #maxBytes: number;

	constructor(maxBytes: number) {
		this.#maxBytes = Math.max(0, maxBytes);
	}

	get totalBytes(): number {
		return this.#total;
	}

	get size(): number {
		return this.#bytesByGop.size;
	}

	has(gopKey: number): boolean {
		return this.#bytesByGop.has(gopKey);
	}

	/**
	 * Record (or refresh) a resident GOP of `bytes` bytes, marking it
	 * most-recently-used, then evict least-recently-used GOPs until back under
	 * budget. Returns the keys evicted so the caller can free their bytes. The
	 * just-touched GOP and an optional `protect` key (e.g. the GOP on screen) are
	 * never evicted, even if that leaves the cache over budget.
	 */
	touch(gopKey: number, bytes: number, protect?: number): number[] {
		// Re-insert to move to the most-recently-used end.
		if (this.#bytesByGop.has(gopKey)) {
			this.#total -= this.#bytesByGop.get(gopKey) ?? 0;
			this.#bytesByGop.delete(gopKey);
		}
		this.#bytesByGop.set(gopKey, bytes);
		this.#total += bytes;

		const evicted: number[] = [];
		for (const key of this.#bytesByGop.keys()) {
			if (this.#total <= this.#maxBytes) break;
			if (key === gopKey || key === protect) continue;
			this.#total -= this.#bytesByGop.get(key) ?? 0;
			this.#bytesByGop.delete(key);
			evicted.push(key);
		}
		return evicted;
	}

	/** Drop a GOP explicitly (e.g. on a decoder reset that invalidates it). */
	delete(gopKey: number): void {
		if (!this.#bytesByGop.has(gopKey)) return;
		this.#total -= this.#bytesByGop.get(gopKey) ?? 0;
		this.#bytesByGop.delete(gopKey);
	}

	/** Forget everything (e.g. on dispose). */
	clear(): void {
		this.#bytesByGop.clear();
		this.#total = 0;
	}
}
