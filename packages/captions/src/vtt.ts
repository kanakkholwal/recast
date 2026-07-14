/**
 * WebVTT with word-level inline cue timestamps.
 *
 * A cue body may carry `<HH:MM:SS.mmm>` tags between words; a player that
 * ignores them still shows the whole cue text, so this stays backward
 * compatible with any older recast whose VTT has no tags. The desktop exporter
 * writes this; the web player parses the tags back to drive progressive
 * highlight.
 */

import type { TranscriptWord } from "./types";

interface Cue {
	start: number;
	end: number;
	words: TranscriptWord[];
	/** Plain text, used when a cue carries no per-word timing. */
	text: string;
}

/** `12.34` seconds -> `00:00:12.340`. */
export function formatVttTime(seconds: number): string {
	const s = Math.max(0, seconds);
	const hh = Math.floor(s / 3600);
	const mm = Math.floor((s % 3600) / 60);
	const ss = Math.floor(s % 60);
	const ms = Math.round((s - Math.floor(s)) * 1000);
	const pad = (n: number, w: number) => String(n).padStart(w, "0");
	return `${pad(hh, 2)}:${pad(mm, 2)}:${pad(ss, 2)}.${pad(ms, 3)}`;
}

/** `00:00:12.340` (or `12.340`, `1:02.5`) -> seconds. */
export function parseVttTime(stamp: string): number {
	const parts = stamp.split(":");
	let s = 0;
	for (const p of parts) s = s * 60 + parseFloat(p);
	return Number.isFinite(s) ? s : 0;
}

/**
 * Serialize cues to a WebVTT string. When a cue has words, the body leads with
 * the cue start tag and inserts a timestamp tag before each subsequent word:
 *   `<00:00:04.120>but <00:00:04.380>it's <00:00:04.600>a`
 */
export function serializeKaraokeVtt(cues: Cue[]): string {
	let out = "WEBVTT\n\n";
	for (const cue of cues) {
		out += `${formatVttTime(cue.start)} --> ${formatVttTime(cue.end)}\n`;
		if (cue.words.length > 0) {
			out += cue.words
				.map((w, i) => (i === 0 ? "" : " ") + `<${formatVttTime(w.start)}>${w.text}`)
				.join("");
		} else {
			out += cue.text.trim();
		}
		out += "\n\n";
	}
	return out;
}

/**
 * Parse one cue body back into words with timings. The FIRST word's start comes
 * from the caller (the cue's own start line); each subsequent `<stamp>` sets the
 * next word's start, and each word's `end` is the following word's start (the
 * caller sets the last word's end to the cue end). A body with no tags yields a
 * single pseudo-word of the whole text so the transcript panel still has text.
 */
export function parseKaraokeCue(
	body: string,
	cueStart: number,
	cueEnd: number,
): TranscriptWord[] {
	const tag = /<(\d{1,2}:)?\d{1,2}:\d{2}\.\d{1,3}>|<\d+(\.\d+)?>/g;
	const tokens: { start: number; text: string }[] = [];
	let cursor = 0;
	let pendingStart = cueStart;
	let match: RegExpExecArray | null;
	// Walk the body, splitting on timestamp tags. Text before a tag belongs to
	// the previously-opened start; the tag opens the next word's start.
	while ((match = tag.exec(body)) !== null) {
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
