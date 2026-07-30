// Edit maths for the video spine: the gestures that move a clip block's own
// start/end, on the original-recording axis.
//
// The spine is DERIVED (kept = [trimStart, trimEnd] − cuts, sliced by splits),
// so a block has no free position to drag to: the output axis is gapless by
// construction. The three edits that are expressible are the three real NLE
// trims, and each one changes the start or end time of a block:
//
//   roll   — an interior split boundary. Left block's end and right block's
//            start move together; total length is unchanged.
//   slide  — a seam (a removed range between two blocks). The removed window
//            moves as a unit: left block's end and right block's start both
//            shift, the amount removed is unchanged.
//   slip   — a block's body, when a removed range sits on BOTH sides. The
//            block's source window shifts inside its slot; it stays put on the
//            output axis while its frames scroll (exactly what slip looks like
//            in Premiere).
//
// Trimming the outer edges is `trimStart`/`trimEnd` and lives in the clip bar's
// in/out handles; changing how much a cut removes is the Cuts lane. Neither is
// duplicated here.

import { quantizeToFrame } from "./timeline-helpers";

/** Boundary-coincidence tolerance. Matches segments.ts. */
const EPS = 1e-4;

export interface SpineSegment {
	start: number;
	end: number;
}

export interface SpineCut {
	id: string;
	start: number;
	end: number;
}

export type SpineHandleKind = "roll" | "slide";

export interface SpineHandle {
	/** Stable across a drag: the boundary index, not a time that moves under it. */
	key: string;
	kind: SpineHandleKind;
	/** Original time the handle currently sits on (the left block's end). */
	at: number;
	/** Clamp range for `at`. */
	min: number;
	max: number;
	/** "slide" only: the cut whose window moves, and its current length. */
	cutId?: string;
	cutLength?: number;
	leftIndex: number;
	rightIndex: number;
}

export interface SpineShape {
	segments: readonly SpineSegment[];
	cuts: readonly SpineCut[];
	/** Shortest a block may be trimmed to. */
	minClip: number;
	/** Shortest a removed range may be. */
	minCut: number;
}

/**
 * Draggable handles for every interior boundary, left to right. A boundary with
 * no room to move is omitted rather than rendered dead, so a handle under the
 * cursor always does something.
 */
export function buildSpineHandles(shape: SpineShape): SpineHandle[] {
	const { segments: segs, minClip } = shape;
	const handles: SpineHandle[] = [];

	for (let i = 0; i < segs.length - 1; i++) {
		const left = segs[i];
		const right = segs[i + 1];
		const gap = right.start - left.end;

		if (gap <= EPS) {
			const min = left.start + minClip;
			const max = right.end - minClip;
			if (max - min > EPS) {
				handles.push({
					key: `roll-${i}`,
					kind: "roll",
					at: left.end,
					min,
					max,
					leftIndex: i,
					rightIndex: i + 1,
				});
			}
			continue;
		}

		// The gap is one removed range. More than one cut inside it means an
		// un-merged pair; leave it alone rather than guess which one to move.
		const inside = shape.cuts.filter(
			(c) => c.start >= left.end - EPS && c.end <= right.start + EPS,
		);
		if (inside.length !== 1) continue;
		const cut = inside[0];
		const cutLength = cut.end - cut.start;
		const min = left.start + minClip;
		const max = right.end - minClip - cutLength;
		if (max - min > EPS) {
			handles.push({
				key: `slide-${i}`,
				kind: "slide",
				at: left.end,
				min,
				max,
				cutId: cut.id,
				cutLength,
				leftIndex: i,
				rightIndex: i + 1,
			});
		}
	}

	return handles;
}

export interface RollResult {
	kind: "roll";
	/** The split point to move, and where it lands. */
	from: number;
	to: number;
}

export interface SlideResult {
	kind: "slide";
	cutId: string;
	start: number;
	end: number;
}

export type SpineEditResult = RollResult | SlideResult;

/**
 * Resolve a handle drag to the store write it implies. `at` is the proposed
 * boundary time (already snapped by the caller); it is clamped and quantised
 * here so a write can never land sub-frame or leave a sliver of a block.
 */
export function applySpineHandle(handle: SpineHandle, at: number, fps: number): SpineEditResult {
	const next = quantizeToFrame(Math.min(handle.max, Math.max(handle.min, at)), fps);
	if (handle.kind === "roll") {
		return { kind: "roll", from: handle.at, to: next };
	}
	return {
		kind: "slide",
		cutId: handle.cutId as string,
		start: next,
		end: next + (handle.cutLength ?? 0),
	};
}

export interface SlipPlan {
	/** Clamped shift actually applied, in original seconds. */
	delta: number;
	/** The two cuts that absorb the shift, with their new bounds. */
	before: { id: string; start: number; end: number };
	after: { id: string; start: number; end: number };
}

/**
 * Plan a slip of block `index` by `delta` seconds. Returns null when the block
 * isn't slippable: it needs a removed range on both sides to shift into, so a
 * block bounded by a split or by the clip's own edge has nowhere to go.
 */
export function planSlip(
	shape: SpineShape,
	index: number,
	delta: number,
	fps: number,
): SlipPlan | null {
	const { segments: segs, minCut } = shape;
	const seg = segs[index];
	const prev = segs[index - 1];
	const next = segs[index + 1];
	if (!seg || !prev || !next) return null;
	if (seg.start - prev.end <= EPS || next.start - seg.end <= EPS) return null;

	const before = findCutBetween(shape.cuts, prev.end, seg.start);
	const after = findCutBetween(shape.cuts, seg.end, next.start);
	if (!before || !after) return null;

	// Both removed ranges must survive the shift at >= minCut.
	const lo = before.start + minCut - seg.start;
	const hi = after.end - minCut - seg.end;
	if (hi - lo <= EPS) return null;

	const shift = quantizeToFrame(Math.min(hi, Math.max(lo, delta)), fps);
	if (Math.abs(shift) <= EPS) return null;

	return {
		delta: shift,
		before: { id: before.id, start: before.start, end: seg.start + shift },
		after: { id: after.id, start: seg.end + shift, end: after.end },
	};
}

/** Whether a block's body can be slipped at all, for the cursor and the label. */
export function canSlip(shape: SpineShape, index: number): boolean {
	const seg = shape.segments[index];
	const prev = shape.segments[index - 1];
	const next = shape.segments[index + 1];
	if (!seg || !prev || !next) return false;
	if (seg.start - prev.end <= EPS || next.start - seg.end <= EPS) return false;
	const before = findCutBetween(shape.cuts, prev.end, seg.start);
	const after = findCutBetween(shape.cuts, seg.end, next.start);
	if (!before || !after) return false;
	const lo = before.start + shape.minCut - seg.start;
	const hi = after.end - shape.minCut - seg.end;
	return hi - lo > EPS;
}

function findCutBetween(cuts: readonly SpineCut[], from: number, to: number): SpineCut | null {
	const inside = cuts.filter((c) => c.start >= from - EPS && c.end <= to + EPS);
	return inside.length === 1 ? inside[0] : null;
}
