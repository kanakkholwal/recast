/** Pure helpers for ZoomSuggestionsPopover: identity, labelling, centre norm. */

import type { ZoomSuggestion, ZoomSuggestionReason } from "$lib/ipc";

/** Stable list key for a suggestion (timestamp + reason). */
export function keyOf(sug: ZoomSuggestion): string {
	return sug.timestampUs + "-" + sug.reason;
}

export function reasonLabel(r: ZoomSuggestionReason): string {
	return r === "click" ? "Click" : "Settle";
}

/**
 * Cursor position normalised to [0,1] against the frame size, or `undefined`
 * when the frame dimensions are unknown (nothing to normalise against).
 */
export function normalizeCenter(
	x: number,
	y: number,
	w: number,
	h: number,
): { x: number; y: number } | undefined {
	if (w <= 0 || h <= 0) return undefined;
	return {
		x: Math.min(1, Math.max(0, x / w)),
		y: Math.min(1, Math.max(0, y / h)),
	};
}
