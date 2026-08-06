/**
 * `.srt` / `.vtt` → the editor's `Transcript`. The web playground has no
 * on-device ASR, so importing a sidecar is how captions get in; everything
 * downstream (styling, offset, cut-splitting, burn-in) then works unchanged.
 */

import type { Transcript, TranscriptSegment } from "@recast/editor/services";

/** `HH:MM:SS,mmm`, `MM:SS.mmm` and the VTT variants all land here. */
export function parseTimestamp(raw: string): number | null {
	const m = raw.trim().match(/^(?:(\d+):)?(\d{1,2}):(\d{1,2})[.,](\d{1,3})$/);
	if (!m) return null;
	const [, h, mm, ss, ms] = m;
	return Number(h ?? 0) * 3600 + Number(mm) * 60 + Number(ss) + Number(ms.padEnd(3, "0")) / 1000;
}

const CUE_RE = /^(.+?)\s*-->\s*(.+?)(?:\s+.*)?$/;

/**
 * Parse a subtitle file. Cue numbers, VTT headers, NOTE blocks and cue settings
 * are all tolerated; a cue whose timing doesn't parse is skipped rather than
 * failing the whole import.
 */
export function parseSubtitles(text: string, filename = ""): Transcript {
	const blocks = text
		.replace(/\r\n/g, "\n")
		.replace(/^﻿/, "")
		.replace(/^WEBVTT[^\n]*\n/, "")
		.split(/\n{2,}/);

	const segments: TranscriptSegment[] = [];
	for (const block of blocks) {
		const lines = block.split("\n").filter((l) => l.trim().length > 0);
		if (lines.length === 0 || lines[0].startsWith("NOTE")) continue;
		// A leading cue number (SRT) or cue identifier (VTT) sits above the timing.
		const timingIndex = lines.findIndex((l) => CUE_RE.test(l) && l.includes("-->"));
		if (timingIndex === -1) continue;
		const m = lines[timingIndex].match(CUE_RE);
		if (!m) continue;
		const start = parseTimestamp(m[1]);
		const end = parseTimestamp(m[2]);
		if (start === null || end === null || end < start) continue;
		const body = lines
			.slice(timingIndex + 1)
			.join(" ")
			.replace(/<[^>]+>/g, "")
			.trim();
		if (!body) continue;
		segments.push({
			id: `imported-${segments.length}`,
			start,
			end,
			text: body,
			words: splitWords(body, start, end),
		});
	}

	return {
		engine: filename ? `import:${filename.split(".").pop()}` : "import",
		modelId: "imported",
		language: null,
		segments,
	};
}

/**
 * Even word timings across the cue. A sidecar rarely carries word-level timing,
 * and the progressive-highlight caption styles need something to advance on —
 * an even split reads far better than every word lighting at once.
 */
function splitWords(text: string, start: number, end: number) {
	const words = text.split(/\s+/).filter(Boolean);
	if (words.length === 0) return [];
	const step = (end - start) / words.length;
	return words.map((w, i) => ({
		start: start + i * step,
		end: start + (i + 1) * step,
		text: w,
	}));
}

/** Serialize back to WebVTT, for the export-sidecar path. */
export function transcriptToVtt(transcript: Transcript): string {
	const stamp = (t: number) => {
		const h = Math.floor(t / 3600);
		const m = Math.floor((t % 3600) / 60);
		const s = Math.floor(t % 60);
		const ms = Math.round((t - Math.floor(t)) * 1000);
		const pad = (n: number, w = 2) => String(n).padStart(w, "0");
		return `${pad(h)}:${pad(m)}:${pad(s)}.${pad(ms, 3)}`;
	};
	let out = "WEBVTT\n\n";
	for (const seg of transcript.segments) {
		out += `${stamp(seg.start)} --> ${stamp(seg.end)}\n${seg.text}\n\n`;
	}
	return out;
}
