/**
 * Timeline segment model: the basis for split + ripple-delete editing.
 *
 * Kept content = `[trimStart, trimEnd]` minus the union of `cuts`. `splitPoints`
 * (original-recording seconds) subdivide that kept content into addressable
 * **segments** without removing anything; a split only matters once one of the
 * segments is deleted (which becomes a `manual` cut). All functions are pure.
 * See ./cuts.ts for the time-remapping that closes gaps left by deletes.
 */

import { normalizeCuts, type TimelineCut } from "./cuts";

/** Tolerance for treating two times as the same boundary. Matches cuts.ts. */
const EPS = 1e-4;

/** A contiguous kept sub-range on the original-recording timeline. */
export interface Segment {
	/** Start on the original-recording timeline (seconds). */
	start: number;
	/** End on the original-recording timeline (seconds). */
	end: number;
	/** Position in the ordered, left-to-right segment list. */
	index: number;
}

export interface ClipShape {
	/** Kept-region start (seconds). */
	trimStart: number;
	/** Kept-region end (seconds). */
	trimEnd: number;
	cuts: TimelineCut[];
	/** Division markers, original seconds. Order/duplicates don't matter. */
	splitPoints: number[];
}

/**
 * Derive the ordered list of kept segments. The kept region `[trimStart,
 * trimEnd]` has the cuts removed, then each surviving interval is sliced at any
 * split points that fall strictly inside it. Zero-length results are dropped,
 * and stray split points (outside the clip or inside a cut) are ignored, so
 * the result stays valid even after a trim or cut moves under a split.
 */
export function deriveSegments(shape: ClipShape): Segment[] {
	const { trimStart, trimEnd, splitPoints } = shape;
	if (trimEnd - trimStart <= EPS) return [];

	// Kept intervals = [trimStart, trimEnd] minus the normalized cuts.
	const cuts = normalizeCuts(shape.cuts).filter(
		(c) => c.end > trimStart && c.start < trimEnd,
	);
	const kept: Array<{ start: number; end: number }> = [];
	let cursor = trimStart;
	for (const c of cuts) {
		const cutStart = Math.max(c.start, trimStart);
		const cutEnd = Math.min(c.end, trimEnd);
		if (cutStart - cursor > EPS) kept.push({ start: cursor, end: cutStart });
		cursor = Math.max(cursor, cutEnd);
	}
	if (trimEnd - cursor > EPS) kept.push({ start: cursor, end: trimEnd });

	// Slice each kept interval at the split points strictly inside it.
	const segments: Segment[] = [];
	for (const interval of kept) {
		const inside = splitPoints
			.filter((p) => p > interval.start + EPS && p < interval.end - EPS)
			.sort((a, b) => a - b);
		let from = interval.start;
		for (const p of inside) {
			segments.push({ start: from, end: p, index: segments.length });
			from = p;
		}
		segments.push({ start: from, end: interval.end, index: segments.length });
	}
	return segments;
}

/**
 * A collapsed cut between two kept segments. On the output (post-cut) timeline
 * the removed span has zero width, so the editor renders it as a single seam
 * marker at `gapStart` (== the previous segment's end). `removed` is how much
 * original time was taken out, shown in the restore tooltip.
 */
export interface Seam {
	/** Original time where the gap begins (previous segment's end). */
	gapStart: number;
	/** Original time where the gap ends (next segment's start). */
	gapEnd: number;
	/** Seconds of original content removed across the gap. */
	removed: number;
}

/**
 * Derive the seams between adjacent kept segments: every place a cut was
 * ripple-removed, collapsing two segments together. Segments that merely *touch*
 * (a split, no removed time) yield no seam.
 */
export function deriveSeams(segments: Segment[]): Seam[] {
	const seams: Seam[] = [];
	for (let i = 0; i < segments.length - 1; i++) {
		const gap = segments[i + 1].start - segments[i].end;
		if (gap > EPS) {
			seams.push({
				gapStart: segments[i].end,
				gapEnd: segments[i + 1].start,
				removed: gap,
			});
		}
	}
	return seams;
}

/**
 * The segment containing original time `t`, or null if `t` is in a trimmed-off
 * region or inside a cut. On an exact internal boundary the playhead belongs to
 * the segment to its right (NLE convention); at the very end it belongs to the
 * last segment.
 */
export function segmentAt(segments: Segment[], t: number): Segment | null {
	for (const seg of segments) {
		if (t >= seg.start - EPS && t < seg.end - EPS) return seg;
	}
	const last = segments[segments.length - 1];
	if (last && Math.abs(t - last.end) <= EPS) return last;
	return null;
}

/**
 * Add a split at original time `t`. Returns the new sorted split-point array,
 * or null when the split is a no-op or illegal: at/outside the clip edges,
 * inside a cut, or coincident with an existing split or boundary.
 */
export function planSplit(t: number, shape: ClipShape): number[] | null {
	const { trimStart, trimEnd, splitPoints } = shape;
	// Can't split at or beyond the clip's outer edges.
	if (t <= trimStart + EPS || t >= trimEnd - EPS) return null;
	// Can't split inside a removed range; can't split exactly on a cut edge
	// (that boundary already exists).
	for (const c of normalizeCuts(shape.cuts)) {
		if (t > c.start - EPS && t < c.end + EPS) return null;
	}
	// Already split here.
	if (splitPoints.some((p) => Math.abs(p - t) <= EPS)) return null;
	return [...splitPoints, t].sort((a, b) => a - b);
}

export interface DeletePlan {
	/** The range to remove (becomes a `manual` cut). */
	cut: { start: number; end: number };
	/** Split points that survive the delete (interior ones are pruned). */
	splitPoints: number[];
}

/**
 * Plan a ripple-delete of `seg`: the segment's range becomes a cut, and any
 * split points that sat inside (or on the edges of) the deleted range are
 * pruned, since they no longer separate two kept segments once the range is gone.
 * The caller applies the cut (the existing time-remap closes the gap) and
 * stores the returned split points.
 */
export function planDeleteSegment(
	seg: Segment,
	splitPoints: number[],
): DeletePlan {
	return {
		cut: { start: seg.start, end: seg.end },
		splitPoints: splitPoints.filter(
			(p) => p < seg.start - EPS || p > seg.end + EPS,
		),
	};
}
