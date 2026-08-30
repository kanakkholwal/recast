/**
 * WebVTT with word-level inline cue timestamps — the READ side.
 *
 * A cue body may carry `<HH:MM:SS.mmm>` tags between words; a player that
 * ignores them still shows the whole cue text, so this stays backward
 * compatible with any older recast whose VTT has no tags.
 *
 * Rust writes the VTT (`transcription/subtitles.rs`). A TypeScript serializer
 * lived here with no caller and no way to notice when the two drifted, so it
 * was deleted rather than kept in parallel.
 */

import type { TranscriptWord } from "./types";

/** `00:00:12.340` (or `12.340`, `1:02.5`) -> seconds. */
export function parseVttTime(stamp: string): number {
	const parts = stamp.split(":");
	let s = 0;
	for (const p of parts) s = s * 60 + parseFloat(p);
	return Number.isFinite(s) ? s : 0;
}

/**
 * Parse one cue body back into words with timings. The FIRST word's start comes
 * from the caller (the cue's own start line); each subsequent `<stamp>` sets the
 * next word's start, and each word's `end` is the following word's start (the
 * caller sets the last word's end to the cue end). A body with no tags yields a
 * single pseudo-word of the whole text so the transcript panel still has text.
 */
export function parseKaraokeCue(body: string, cueStart: number, cueEnd: number): TranscriptWord[] {
	const tag = /<(\d{1,2}:)?\d{1,2}:\d{2}\.\d{1,3}>|<\d+(\.\d+)?>/g;
	const tokens: { start: number; text: string }[] = [];
	let cursor = 0;
	let pendingStart = cueStart;
	// Text before a tag belongs to the previously opened start, and the tag opens the next word's.
	for (let match = tag.exec(body); match !== null; match = tag.exec(body)) {
		const chunk = body.slice(cursor, match.index).trim();
		if (chunk) tokens.push({ start: pendingStart, text: chunk });
		pendingStart = parseVttTime(match[0].slice(1, -1));
		cursor = tag.lastIndex;
	}
	const tail = body.slice(cursor).trim();
	if (tail) tokens.push({ start: pendingStart, text: tail });

	if (tokens.length === 0) return [];
	return tokens.map((tok, i) => ({
		start: tok.start,
		end: i + 1 < tokens.length ? tokens[i + 1].start : cueEnd,
		text: tok.text,
	}));
}
