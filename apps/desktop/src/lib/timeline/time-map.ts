/**
 * General piecewise-linear time map: original-recording time ↔ output time.
 *
 * Generalizes the cut model (./cuts.ts) so a kept span can play at a speed other
 * than 1×. Each kept span maps original→output with slope `1/speed`; removed
 * (trimmed or cut) original time has no output image and collapses to the seam.
 * With every span at speed 1 this reduces exactly to the cut translation map,
 * proven in ./time-map.test.ts against the shared cut-parity fixtures.
 *
 * This is the substrate per-segment speed sits on: build the map from the kept
 * segments (./segments.ts) plus a speed per segment, then route playhead,
 * filmstrip, and export through the same arithmetic so they can't drift.
 */

import { normalizeCuts, type TimelineCut } from "./cuts";
import type { Segment } from "./segments";

/** Tolerance for treating two times as the same boundary. Matches cuts.ts. */
const EPS = 1e-4;

/** A kept original range to play at `speed`. */
export interface TimeSpan {
	/** Kept original range start (seconds). */
	origStart: number;
	/** Kept original range end (seconds). */
	origEnd: number;
	/** Playback speed (>0). 2 = twice as fast = half the output width. */
	speed: number;
}

/** A kept span placed on both axes. */
export interface MappedSpan extends TimeSpan {
	/** Output range start (seconds). */
	outStart: number;
	/** Output range end (seconds). */
	outEnd: number;
}

export interface TimeMap {
	/** Kept spans in original order, each annotated with its output range. */
	spans: MappedSpan[];
	/** Total output duration (seconds). */
	outputDuration: number;
}

/** Speed lookup keyed by segment index; defaults to 1× for any missing entry. */
export type SpeedOf = (segmentIndex: number) => number;

/**
 * Build a time map from kept spans. Spans are sorted by original start and laid
 * end-to-end on the output axis; a non-positive or non-finite speed falls back
 * to 1 so a bad override can never produce a zero-width or NaN output range.
 */
export function buildTimeMap(spans: TimeSpan[]): TimeMap {
	const ordered = spans
		.filter((s) => s.origEnd - s.origStart > EPS)
		.sort((a, b) => a.origStart - b.origStart);

	const mapped: MappedSpan[] = [];
	let out = 0;
	for (const s of ordered) {
		const speed = s.speed > 0 && Number.isFinite(s.speed) ? s.speed : 1;
		const outStart = out;
		out += (s.origEnd - s.origStart) / speed;
		mapped.push({ ...s, speed, outStart, outEnd: out });
	}
	return { spans: mapped, outputDuration: out };
}

/** Build a time map from kept segments, applying a per-segment speed. */
export function timeMapFromSegments(
	segments: Segment[],
	speedOf: SpeedOf = () => 1,
): TimeMap {
	return buildTimeMap(
		segments.map((seg) => ({
			origStart: seg.start,
			origEnd: seg.end,
			speed: speedOf(seg.index),
		})),
	);
}

/** Kept sub-intervals of [lo, hi] with the cuts removed. */
function keptIntervals(
	lo: number,
	hi: number,
	normalized: TimelineCut[],
): Array<{ start: number; end: number }> {
	if (hi - lo <= EPS) return [];
	const out: Array<{ start: number; end: number }> = [];
	let cursor = lo;
	for (const c of normalized) {
		if (c.end <= lo || c.start >= hi) continue;
		const cutStart = Math.max(c.start, lo);
		if (cutStart - cursor > EPS) out.push({ start: cursor, end: cutStart });
		cursor = Math.max(cursor, Math.min(c.end, hi));
	}
	if (hi - cursor > EPS) out.push({ start: cursor, end: hi });
	return out;
}

/**
 * The FULL-recording axis used transiently WHILE TRIMMING: the whole [0,
 * durationSec] with the trimmed head/tail shown at 1× (so the handles can move
 * across the entire source and reveal what's trimmed), kept segments warped by
 * speed, and cuts collapsed. Resting state uses the kept-only map
 * (`timeMapFromSegments`); this un-collapses so a trim drag isn't degenerate at
 * the clip's left edge. At all-1× it's the cut translation map over [0,duration].
 */
export function displayTimeMap(args: {
	trimStart: number;
	trimEnd: number;
	durationSec: number;
	segments: Segment[];
	cuts: TimelineCut[];
	speedOf?: SpeedOf;
}): TimeMap {
	const { trimStart, trimEnd, durationSec, segments, cuts } = args;
	const speedOf = args.speedOf ?? (() => 1);
	const normalized = normalizeCuts(cuts);
	const spans: TimeSpan[] = [];
	for (const k of keptIntervals(0, trimStart, normalized)) {
		spans.push({ origStart: k.start, origEnd: k.end, speed: 1 });
	}
	for (const seg of segments) {
		spans.push({ origStart: seg.start, origEnd: seg.end, speed: speedOf(seg.index) });
	}
	for (const k of keptIntervals(trimEnd, durationSec, normalized)) {
		spans.push({ origStart: k.start, origEnd: k.end, speed: 1 });
	}
	return buildTimeMap(spans);
}

/**
 * Re-space a collapsed map so removed time between kept spans shows as a GAP,
 * for the opt-in "show cut gaps" timeline view. Each kept span keeps its
 * (speed-warped) output width but is pushed right by the original duration
 * removed before it, so cuts read as empty space instead of a zero-width seam.
 * Playback/export never use this — only rendering. With no cuts (contiguous
 * spans) it returns an equivalent map, so the view is a no-op until you cut.
 */
export function buildGapMap(map: TimeMap): TimeMap {
	const spans: MappedSpan[] = [];
	let out = 0;
	let prevOrigEnd: number | null = null;
	for (const s of map.spans) {
		if (prevOrigEnd !== null && s.origStart - prevOrigEnd > EPS) {
			out += s.origStart - prevOrigEnd; // gap = removed original duration
		}
		const width = s.outEnd - s.outStart; // keep the collapsed (warped) width
		spans.push({ ...s, outStart: out, outEnd: out + width });
		out += width;
		prevOrigEnd = s.origEnd;
	}
	return { spans, outputDuration: out };
}

/**
 * Map an original-timeline time to output time. A time in a removed gap (or
 * before the first span) collapses onto the next kept span's output start (the
 * seam), matching cuts.ts, where a time inside a cut maps to the cut's start.
 */
export function originalToOutput(map: TimeMap, t: number): number {
	// Binary search for the first span with `origEnd >= t` — identical to the
	// linear "first span where t <= origEnd" (a time before that span's start
	// collapses onto its seam). Spans are ordered and non-overlapping, so origEnd
	// is non-decreasing. The waveform lane evaluates this per bucket over ~2000
	// buckets, so at high cut counts the old O(spans) scan dominated a zoom.
	const spans = map.spans;
	let lo = 0;
	let hi = spans.length;
	while (lo < hi) {
		const mid = (lo + hi) >> 1;
		if (spans[mid].origEnd >= t) hi = mid;
		else lo = mid + 1;
	}
	if (lo >= spans.length) return map.outputDuration;
	const s = spans[lo];
	if (t < s.origStart) return s.outStart;
	return s.outStart + (t - s.origStart) / s.speed;
}

/**
 * Map an output time back to original time. Output is clamped to the kept range;
 * on an exact internal seam the right-hand span wins (NLE convention, matching
 * segmentAt), so seeking onto a seam lands at the start of the next kept span.
 */
export function outputToOriginal(map: TimeMap, t: number): number {
	const spans = map.spans;
	if (spans.length === 0) return 0;
	for (const s of spans) {
		if (t <= s.outStart + EPS) return s.origStart;
		if (t < s.outEnd - EPS) return s.origStart + (t - s.outStart) * s.speed;
	}
	return spans[spans.length - 1].origEnd;
}

/** The kept span covering original time `t`, or null if `t` is removed. */
export function spanAtOriginal(map: TimeMap, t: number): MappedSpan | null {
	for (const s of map.spans) {
		if (t >= s.origStart - EPS && t < s.origEnd - EPS) return s;
	}
	const last = map.spans[map.spans.length - 1];
	if (last && Math.abs(t - last.origEnd) <= EPS) return last;
	return null;
}
