// Clip-block layout and trim-handle clamp maths for TimelineClipBar.

import { quantizeToFrame } from "./timeline-helpers";

export interface ClipBlock {
	key: number;
	start: number;
	end: number;
	left: number;
	width: number;
	stripOffset: number;
}

// One block per kept segment on the OUTPUT (post-cut) axis. A cut occupies zero
// width so later blocks slide left; each block reveals its slice of the original
// strip via stripOffset. `xOf` maps original time onto the output axis.
export function layoutClipBlocks(
	segments: ReadonlyArray<{ start: number; end: number }>,
	xOf: (t: number) => number,
	pps: number,
	inPoint: number,
): ClipBlock[] {
	return segments.map((seg) => ({
		key: seg.start,
		start: seg.start,
		end: seg.end,
		left: xOf(seg.start),
		// -2px leaves a thin seam between adjacent clips so a split reads as two.
		width: Math.max(2, xOf(seg.end) - xOf(seg.start) - 2),
		stripOffset: -(seg.start - inPoint) * pps,
	}));
}

// Trim clamps keep the kept range at least `min` wide against the opposite point.
export function clampTrimIn(t: number, outPoint: number, min: number): number {
	return Math.max(0, Math.min(t, outPoint - min));
}

export function clampTrimOut(t: number, duration: number, inPoint: number, min: number): number {
	return Math.min(duration, Math.max(t, inPoint + min));
}

// Keyboard nudge variants land on the frame grid.
export function nudgeTrimIn(
	inPoint: number,
	outPoint: number,
	delta: number,
	min: number,
	fps: number,
): number {
	return quantizeToFrame(Math.max(0, Math.min(outPoint - min, inPoint + delta)), fps);
}

export function nudgeTrimOut(
	inPoint: number,
	outPoint: number,
	duration: number,
	delta: number,
	min: number,
	fps: number,
): number {
	return quantizeToFrame(Math.max(inPoint + min, Math.min(duration, outPoint + delta)), fps);
}
