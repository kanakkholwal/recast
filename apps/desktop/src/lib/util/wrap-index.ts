/** Modular index wrapping for keyboard list navigation (arrow up/down cycling). */

/** Wrap `i` into `[0, len)`, handling negatives. Returns 0 for an empty list. */
export function wrapIndex(i: number, len: number): number {
	if (len <= 0) return 0;
	return ((i % len) + len) % len;
}

/** Next index after `i`, wrapping past the end back to the start. */
export function nextIndex(i: number, len: number): number {
	return wrapIndex(i + 1, len);
}

/** Previous index before `i`, wrapping past the start to the end. */
export function prevIndex(i: number, len: number): number {
	return wrapIndex(i - 1, len);
}
