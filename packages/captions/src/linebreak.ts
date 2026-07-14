/**
 * Deterministic, measurement-free line breaking.
 *
 * The DOM preview and the libass export use different text shapers, so relying
 * on either one to wrap would drift. Instead both HONOUR a break computed here
 * from character counts alone: the preview renders explicit lines, the exporter
 * emits `\N` at the same indices. Because it never measures glyphs, TS and Rust
 * produce identical output. (Pill WIDTH still uses real measurement; a few px
 * of rasterizer difference there is invisible and cannot move a break.)
 */

import type { TranscriptWord } from "./types";

/**
 * Greedily group `words` into lines no wider than `maxChars` characters,
 * counting a single space between words. A word longer than `maxChars` on its
 * own occupies its own line rather than being split (never break inside a word).
 * `maxLines` caps the number of lines; overflow words are dropped from the tail
 * (the caller decides chunk sizes, so this is a hard safety clamp, not the
 * primary limiter).
 *
 * Returns arrays of indices INTO `words`, so callers keep each word's timing.
 */
export function breakIntoLines(
	words: Pick<TranscriptWord, "text">[],
	maxChars: number,
	maxLines: number,
): number[][] {
	const limit = Math.max(1, Math.floor(maxChars));
	const cap = Math.max(1, Math.floor(maxLines));
	const lines: number[][] = [];
	let current: number[] = [];
	let currentLen = 0;

	for (let i = 0; i < words.length; i++) {
		const wordLen = words[i].text.length;
		const added = current.length === 0 ? wordLen : currentLen + 1 + wordLen;
		if (current.length > 0 && added > limit) {
			lines.push(current);
			current = [i];
			currentLen = wordLen;
		} else {
			current.push(i);
			currentLen = added;
		}
		if (lines.length === cap) break;
	}
	if (current.length > 0 && lines.length < cap) lines.push(current);
	return lines.slice(0, cap);
}
