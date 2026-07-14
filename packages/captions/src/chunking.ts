/**
 * Chunking and active-word logic. The Rust ASS generator mirrors these exactly
 * (`chunk_words`, `active_word_index` in transcription/subtitles.rs); the shared
 * fixture in src/__fixtures__/caption-parity.json is what proves they agree.
 */

import type { CaptionAnimation, CaptionHighlight, TranscriptWord } from "./types";

/** Static default renders like a plain, un-highlighted caption line. An absent
 *  animation on an OLD project resolves here but with `highlight` filled to
 *  `active` (see resolveCaptionAnimation), preserving its prior look. */
export const DEFAULT_CAPTION_ANIMATION: CaptionAnimation = {
	chunk: "line",
	chunkSize: 3,
	emphasis: "none",
	emphasisColor: "#facc15",
	highlight: "none",
	entrance: "none",
	entranceMs: 220,
	holdGaps: true,
};

/**
 * Resolve a possibly-undefined animation to a concrete spec.
 *
 * Back-compat rule: a project saved before `highlight` existed has the field
 * undefined. Such a project also predates progressive highlight, so it must
 * resolve to `active` (the old per-word behaviour), NOT to the new default.
 * A fully-absent animation is a static line, so it resolves to `none`.
 */
export function resolveCaptionAnimation(a?: CaptionAnimation): CaptionAnimation {
	if (!a) return { ...DEFAULT_CAPTION_ANIMATION };
	const highlight: CaptionHighlight = a.highlight ?? "active";
	return { ...DEFAULT_CAPTION_ANIMATION, ...a, highlight };
}

/** True when the animation has no visible effect (cheap static path). */
export function isStaticAnimation(a: CaptionAnimation): boolean {
	return (
		a.chunk === "line" &&
		a.emphasis === "none" &&
		(a.highlight ?? "none") === "none" &&
		a.entrance === "none"
	);
}

/** A contiguous run of words shown together. `start`/`end` are source-time
 *  seconds spanning the run; `words` keeps each word's own timing. */
export interface CaptionChunkRun {
	start: number;
	end: number;
	words: TranscriptWord[];
}

/**
 * Split a line's words into display chunks per the animation spec.
 *
 * - `line`   -> one chunk with every word.
 * - `word`   -> one chunk per word.
 * - `phrase` -> greedy fixed-size groups of `chunkSize` (min 1).
 */
export function chunkWords(words: TranscriptWord[], anim: CaptionAnimation): CaptionChunkRun[] {
	if (words.length === 0) return [];
	const size =
		anim.chunk === "line"
			? words.length
			: anim.chunk === "word"
				? 1
				: Math.max(1, Math.floor(anim.chunkSize));

	const runs: CaptionChunkRun[] = [];
	for (let i = 0; i < words.length; i += size) {
		const group = words.slice(i, i + size);
		runs.push({
			start: group[0].start,
			end: group[group.length - 1].end,
			words: group,
		});
	}
	return runs;
}

/**
 * Index of the chunk to display at source-time `t`. Holds the previous chunk
 * through the gap before the next one starts, so a single-word style never
 * blinks to empty between words. Returns the first chunk before any has started.
 */
export function activeChunkIndex(runs: CaptionChunkRun[], t: number): number {
	if (runs.length === 0) return -1;
	let idx = 0;
	for (let i = 0; i < runs.length; i++) {
		if (t >= runs[i].start) idx = i;
		else break;
	}
	return idx;
}

/**
 * Index of the active (currently-spoken) word in a chunk at source-time `t`.
 * A word containing `t` wins; in a gap, `holdGaps` keeps the most recently
 * started word lit, otherwise returns -1. -1 before the first word.
 */
export function activeWordIndex(
	words: Pick<TranscriptWord, "start" | "end">[],
	t: number,
	holdGaps: boolean,
): number {
	let last = -1;
	for (let i = 0; i < words.length; i++) {
		if (t >= words[i].start && t < words[i].end) return i;
		if (t >= words[i].start) last = i;
	}
	return holdGaps ? last : -1;
}
