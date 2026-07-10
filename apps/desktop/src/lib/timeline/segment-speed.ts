/**
 * Per-segment speed overrides: the data model behind Cap-style "edit this cut
 * differently". A kept segment can play at a speed other than 1×; an override is
 * anchored to the segment's ORIGINAL start time, which is stable under cuts and
 * ripple-deletes (they don't move original times). A trim or split that orphans
 * an anchor drops it, the same forgiving rule deriveSegments uses for stray
 * splits. The time-map (./time-map.ts) turns these into the warped output axis.
 */

import type { Segment } from "./segments";

export const MIN_SEGMENT_SPEED = 0.25;
export const MAX_SEGMENT_SPEED = 4;
/** Tolerance for matching an anchor to a segment start. Matches segments.ts. */
const EPS = 1e-4;

/** A speed pinned to the segment starting at `start` (original seconds). */
export interface SegmentSpeed {
	start: number;
	speed: number;
}

/** Clamp to the supported range; a non-positive or non-finite value resets to 1. */
export function clampSpeed(speed: number): number {
	if (!Number.isFinite(speed) || speed <= 0) return 1;
	return Math.min(MAX_SEGMENT_SPEED, Math.max(MIN_SEGMENT_SPEED, speed));
}

/** Speed for the segment starting at `start`, or 1 when unset. */
export function segmentSpeedAt(
	overrides: ReadonlyArray<SegmentSpeed>,
	start: number,
): number {
	for (const o of overrides) {
		if (Math.abs(o.start - start) <= EPS) return o.speed;
	}
	return 1;
}

/**
 * Speed of the segment that CONTAINS original time `t`, or 1 when none matches.
 * Forward-biased at a seam (t on a boundary resolves to the following segment),
 * and held at the final segment's speed once `t` reaches the last kept frame.
 * The legacy `<video>` preview reads this to set `playbackRate` per segment.
 */
export function segmentSpeedAtTime(
	segments: ReadonlyArray<Segment>,
	overrides: ReadonlyArray<SegmentSpeed>,
	t: number,
): number {
	for (const s of segments) {
		if (t >= s.start - EPS && t < s.end - EPS) {
			return segmentSpeedAt(overrides, s.start);
		}
	}
	const last = segments[segments.length - 1];
	return last ? segmentSpeedAt(overrides, last.start) : 1;
}

/**
 * Upsert a segment's speed, returning a new sorted array. Setting it back to ~1
 * removes the entry so the override list stays sparse (and serializes to
 * nothing). The value is clamped to the supported range.
 */
export function setSegmentSpeed(
	overrides: ReadonlyArray<SegmentSpeed>,
	start: number,
	speed: number,
): SegmentSpeed[] {
	const clamped = clampSpeed(speed);
	const rest = overrides
		.filter((o) => Math.abs(o.start - start) > EPS)
		.map((o) => ({ ...o }));
	if (Math.abs(clamped - 1) <= EPS) {
		return rest.sort((a, b) => a.start - b.start);
	}
	return [...rest, { start, speed: clamped }].sort((a, b) => a.start - b.start);
}

/** Drop overrides whose anchor no longer matches a current segment start. */
export function pruneSegmentSpeeds(
	overrides: ReadonlyArray<SegmentSpeed>,
	segments: ReadonlyArray<Segment>,
): SegmentSpeed[] {
	if (overrides.length === 0) return [];
	return overrides
		.filter((o) => segments.some((s) => Math.abs(s.start - o.start) <= EPS))
		.map((o) => ({ ...o }))
		.sort((a, b) => a.start - b.start);
}

/** A speed lookup by segment index, for `timeMapFromSegments`. */
export function buildSpeedOf(
	segments: ReadonlyArray<Segment>,
	overrides: ReadonlyArray<SegmentSpeed>,
): (segmentIndex: number) => number {
	return (segmentIndex) => {
		const seg = segments[segmentIndex];
		return seg ? segmentSpeedAt(overrides, seg.start) : 1;
	};
}
