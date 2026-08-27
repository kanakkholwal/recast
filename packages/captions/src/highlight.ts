/**
 * Progressive highlight: how many words of a chunk are "spoken" at time `t`.
 *
 * This is the Loom model. Every word of the chunk is visible; the first
 * `spokenWordCount` of them are painted in the base colour and the rest are
 * muted. As `t` advances the count only grows within the chunk, so a spoken
 * word stays bright. Mirrored in Rust as the karaoke `\k` boundary count.
 */

import type { TranscriptWord } from "./types";

/**
 * Count of words considered spoken at source-time `t`.
 *
 * A word counts as spoken once `t` reaches its `start` (reaching the start is
 * the moment it lights up, matching how `\k` flips a syllable at its boundary).
 * `holdGaps` does not change the count here: once a word starts it stays
 * counted regardless, so gaps never un-highlight an earlier word.
 *
 * Returns 0 before the first word starts and `words.length` once the last has.
 */
export function spokenWordCount(words: Pick<TranscriptWord, "start" | "end">[], t: number): number {
	let n = 0;
	for (let i = 0; i < words.length; i++) {
		if (t >= words[i].start) n = i + 1;
		else break;
	}
	return n;
}

/**
 * Per-word centisecond durations for ASS `\k` karaoke, in chunk order.
 *
 * ASS `\k` uses cumulative centiseconds, and rounding each word independently
 * accumulates drift across a line. So we round the CUMULATIVE boundary of each
 * word to centiseconds and diff consecutive boundaries; the sum then equals the
 * rounded total exactly. Each word holds until the NEXT word starts (matching
 * the preview's gap-hold), and the last word holds to its own end.
 *
 * `chunkStart` is subtracted so the first boundary is measured from the chunk's
 * own start. All times are seconds.
 */
export function karaokeCentiseconds(
	words: Pick<TranscriptWord, "start" | "end">[],
	chunkStart: number,
): number[] {
	if (words.length === 0) return [];
	const cs = (s: number) => Math.round((s - chunkStart) * 100);
	const durations: number[] = [];
	let prevBoundary = 0;
	for (let i = 0; i < words.length; i++) {
		const nextStart = i + 1 < words.length ? words[i + 1].start : words[i].end;
		const boundary = Math.max(prevBoundary, cs(nextStart));
		durations.push(boundary - prevBoundary);
		prevBoundary = boundary;
	}
	return durations;
}
