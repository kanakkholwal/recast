/**
 * Per-word visual state, shared by the preview overlay and the player overlay
 * (and mirrored by the Rust ASS generator). Keeping this here means all three
 * decide a word's colour from the same rule.
 */

import type { CaptionAnimation, CaptionStyle } from "./types";

export interface WordRenderInput {
	/** Word position within its chunk. */
	index: number;
	/** Currently-spoken word index in the chunk (-1 if none). */
	activeIndex: number;
	/** How many words of the chunk are spoken (progressive highlight). */
	spokenCount: number;
	/** Words in the chunk (scale emphasis is suppressed for a lone word). */
	wordCount: number;
	style: Pick<CaptionStyle, "color" | "mutedColor">;
	anim: Pick<CaptionAnimation, "highlight" | "emphasis" | "emphasisColor">;
}

/**
 * Hex colour for a word.
 *
 * - The active word wins the accent when `emphasis === 'color'`.
 * - Otherwise, `progressive` highlight paints spoken words in the base colour
 *   and unspoken words muted; `active` and `none` paint every word the base
 *   colour (their per-word treatment is the accent above / the scale below).
 */
export function wordColor(input: WordRenderInput): string {
	const { index, activeIndex, spokenCount, style, anim } = input;
	if (index === activeIndex && anim.emphasis === "color") return anim.emphasisColor;
	if ((anim.highlight ?? "none") === "progressive") {
		return index < spokenCount ? style.color : style.mutedColor;
	}
	return style.color;
}

/** Whether this word should scale-up (the `punch`/impact pop). Suppressed for a
 *  single-word chunk, where the entrance pop already carries the emphasis. */
export function wordScaled(
	input: Pick<WordRenderInput, "index" | "activeIndex" | "wordCount" | "anim">,
): boolean {
	return (
		input.anim.emphasis === "scale" && input.index === input.activeIndex && input.wordCount > 1
	);
}
