/**
 * Caption ↔ cut clipping math. The transcript lives in ORIGINAL (source)
 * time, but the editor's preview shows the EDITED timeline with cuts
 * collapsed. A caption segment that spans a cut must be displayed as
 * the portion that's INSIDE a kept span — showing the whole segment
 * would make the caption appear to last too long (the cut portion is
 * silent on screen, but the caption would still hang around).
 *
 * Pure functions, no DOM. Vitest covers the math (see clip-captions.test.ts).
 */

import type { TranscriptSegment, TranscriptWord } from '$lib/ipc';

export interface KeptSpan {
	/** Original-recording span start (seconds). */
	origStart: number;
	/** Original-recording span end (seconds). */
	origEnd: number;
}

/** Clamp a value to `[lo, hi]`. */
function clamp(x: number, lo: number, hi: number): number {
	return Math.max(lo, Math.min(hi, x));
}

/** Clamp a segment's time range to a single kept span. Returns null if
 *  the segment is fully outside the span. */
export function clipSegmentToSpan(
	segment: TranscriptSegment,
	span: KeptSpan,
): { start: number; end: number } | null {
	const start = Math.max(segment.start, span.origStart);
	const end = Math.min(segment.end, span.origEnd);
	if (end <= start) return null;
	return { start, end };
}

/** Clamp each word's time range to the kept span, dropping words that
 *  are fully outside. Words that span the span boundary are clipped. */
export function clipWordsToSpan(
	words: ReadonlyArray<TranscriptWord>,
	span: KeptSpan,
): TranscriptWord[] {
	const out: TranscriptWord[] = [];
	for (const w of words) {
		const start = clamp(w.start, span.origStart, span.origEnd);
		const end = clamp(w.end, span.origStart, span.origEnd);
		if (end <= start) continue;
		out.push({ start, end, text: w.text });
	}
	return out;
}

/**
 * The active caption at source time `nowOrig`, intersected with the kept
 * span that contains `nowOrig`. Returns null if `nowOrig` is inside a cut
 * (in which case no caption should be on screen) or if no segment
 * overlaps the visible window.
 */
export function activeClippedSegment(
	segments: ReadonlyArray<TranscriptSegment>,
	span: KeptSpan,
	nowOrig: number,
): { segment: TranscriptSegment; visible: { start: number; end: number } } | null {
	// The kept span is fully before `nowOrig` (the player has scrolled past
	// it) or fully after (the player hasn't reached it yet). Either way,
	// no caption is on screen.
	if (nowOrig < span.origStart || nowOrig >= span.origEnd) return null;
	const segment = segments.find((s) => nowOrig >= s.start && nowOrig < s.end);
	if (!segment) return null;
	const visible = clipSegmentToSpan(segment, span);
	if (!visible) return null;
	return { segment, visible };
}
