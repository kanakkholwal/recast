/**
 * Caption animation model: the shared spec both renderers derive from.
 *
 * Animated captions have two renderers: the CSS preview overlay and the ASS
 * burn-in (libass) generator in Rust. To keep them from drifting, the animation
 * is plain data here; the preview reads it directly and the Rust ASS generator
 * mirrors the same fields + chunking. This module is dependency-free (no store,
 * no registry) so it can be imported from either side without a cycle.
 */

/** How many words are shown on screen at once. */
export type CaptionChunk = "line" | "phrase" | "word";
/** Treatment of the word currently being spoken. */
export type CaptionEmphasis = "none" | "color" | "scale";
/** Per-chunk / per-word entrance animation. */
export type CaptionEntrance = "none" | "fade" | "pop" | "slide";

export interface CaptionAnimation {
	/** Words shown at once: a full line, a fixed-size phrase, or one word. */
	chunk: CaptionChunk;
	/** Words per chunk when `chunk === 'phrase'`. */
	chunkSize: number;
	/** What happens to the word being spoken. */
	emphasis: CaptionEmphasis;
	/** Accent colour for `emphasis === 'color'` (hex). */
	emphasisColor: string;
	/** Entrance animation applied to each chunk/word as it appears. */
	entrance: CaptionEntrance;
	/** Entrance duration (ms). */
	entranceMs: number;
	/** Keep the active-word emphasis through short silences instead of clearing. */
	holdGaps: boolean;
}

/** Static default (`chunk: line`, no emphasis, no entrance) renders exactly
 *  like a non-animated caption, so an absent animation == today's behaviour. */
export const DEFAULT_CAPTION_ANIMATION: CaptionAnimation = {
	chunk: "line",
	chunkSize: 3,
	emphasis: "none",
	emphasisColor: "#facc15",
	entrance: "none",
	entranceMs: 220,
	holdGaps: true,
};

/** Resolve a possibly-undefined animation to a concrete spec. */
export function resolveCaptionAnimation(a?: CaptionAnimation): CaptionAnimation {
	return a ? { ...DEFAULT_CAPTION_ANIMATION, ...a } : DEFAULT_CAPTION_ANIMATION;
}

/** True when the animation has no visible effect (so renderers can take the
 *  cheap static path). */
export function isStaticAnimation(a: CaptionAnimation): boolean {
	return a.chunk === "line" && a.emphasis === "none" && a.entrance === "none";
}

/** A contiguous run of words shown together. `start`/`end` are source-time
 *  seconds spanning the run; `words` keeps each word's own timing for emphasis. */
export interface CaptionChunkRun {
	start: number;
	end: number;
	words: { start: number; end: number; text: string }[];
}

/**
 * Split a line's words into display chunks per the animation spec. This is the
 * algorithm the Rust ASS generator mirrors exactly. Keep them in sync.
 *
 * - `line`  → one chunk with every word (emphasis still tracks the active word).
 * - `word`  → one chunk per word.
 * - `phrase`→ greedy fixed-size groups of `chunkSize` (min 1).
 */
export function chunkWords(
	words: { start: number; end: number; text: string }[],
	anim: CaptionAnimation,
): CaptionChunkRun[] {
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
 * started word lit, otherwise returns -1 (no emphasis). -1 before the first word.
 */
export function activeWordIndex(
	words: { start: number; end: number }[],
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
