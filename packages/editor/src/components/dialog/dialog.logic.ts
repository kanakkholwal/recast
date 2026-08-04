/** Shared pure helpers for the recast Rename/Confirm dialogs. */

/** Normalise a thrown value (string reject, Error, or other) to a message. */
export function toErrorMessage(e: unknown): string {
	return typeof e === "string" ? e : e instanceof Error ? e.message : String(e);
}

/**
 * Selection range for the filename stem (before the extension dot). Returns
 * `null` when there's no usable dot, signalling "select the whole value".
 */
export function stemSelectionRange(seed: string): [number, number] | null {
	const dot = seed.lastIndexOf(".");
	return dot > 0 ? [0, dot] : null;
}
