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

import type { TranscriptSegment, TranscriptWord } from "../wire-types";

export interface KeptSpan {
	/** Original-recording span start (seconds). */
	origStart: number;
	/** Original-recording span end (seconds). */
	origEnd: number;
}

const SPAN_EPS = 1e-3;

/**
 * Merge a time map's spans into caption-clipping spans: adjacent spans that are
 * contiguous in ORIGINAL time (a split or a speed change — no removed content)
 * collapse into one, so a caption only ever breaks at a REAL cut. The time map
 * carries one span PER SEGMENT, so without this a caption spanning a split loses
 * the words on the far side of the split boundary.
 */
/**
 * Memoized on the `spans` array identity. This is called once per rendered
 * caption frame — i.e. at rAF rate — and rebuilt a merged array over EVERY span
 * each time. The time map is a `$derived.by`, so its spans array is a new object
 * only when the cuts actually change, which is exactly the invalidation we want.
 */
const mergedSpanCache = new WeakMap<object, KeptSpan[]>();

export function keptCaptionSpans(map: {
	spans: ReadonlyArray<{ origStart: number; origEnd: number }>;
}): KeptSpan[] {
	const cached = mergedSpanCache.get(map.spans);
	if (cached) return cached;
	const merged: KeptSpan[] = [];
	for (const s of map.spans) {
		const last = merged[merged.length - 1];
		if (last && s.origStart <= last.origEnd + SPAN_EPS) {
			last.origEnd = Math.max(last.origEnd, s.origEnd);
		} else {
			merged.push({ origStart: s.origStart, origEnd: s.origEnd });
		}
	}
	mergedSpanCache.set(map.spans, merged);
	return merged;
}

/** The merged kept span containing original time `t`, or null when `t` is inside
 *  a real cut. Mirrors `spanAtOriginal`'s boundary handling. */
export function captionSpanAt(spans: ReadonlyArray<KeptSpan>, t: number): KeptSpan | null {
	for (const s of spans) {
		if (t >= s.origStart - 1e-4 && t < s.origEnd - 1e-4) return s;
	}
	const last = spans[spans.length - 1];
	if (last && Math.abs(t - last.origEnd) <= 1e-4) return last;
	return null;
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

/** One piece of a caption segment that survived the cuts, still in SOURCE time. */
export interface SegmentPiece {
	/** Clipped source range. */
	start: number;
	end: number;
	/** Words clipped to the same span; empty when the segment had no per-word timing. */
	words: TranscriptWord[];
	/** Index into the span list this piece belongs to. */
	spanIndex: number;
	/** True when the parent segment produced more than one piece. */
	split: boolean;
}

/**
 * Split a caption segment across the kept spans, dropping the parts that fall
 * inside cuts. A segment that straddles a cut yields one piece per side.
 *
 * This is the batch form of what the preview does per-frame
 * ({@link activeClippedSegment}). Emitting a straddling caption as ONE cue
 * spanning the seam is wrong twice over: it carries text for audio the export
 * removed, and its output range covers time the cue is not actually spoken
 * over. Both the sidecar writer and the burn-in go through here so all three
 * surfaces (preview, burned pixels, sidecar) agree on cue content.
 */
export function splitSegmentAcrossSpans(
	segment: TranscriptSegment,
	spans: ReadonlyArray<KeptSpan>,
): SegmentPiece[] {
	const pieces: SegmentPiece[] = [];
	for (let i = 0; i < spans.length; i++) {
		const visible = clipSegmentToSpan(segment, spans[i]);
		if (!visible) continue;
		pieces.push({
			start: visible.start,
			end: visible.end,
			words: clipWordsToSpan(segment.words, spans[i]),
			spanIndex: i,
			split: false,
		});
	}
	if (pieces.length > 1) for (const p of pieces) p.split = true;
	return pieces;
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
