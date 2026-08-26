// Shared drag/resize/nudge maths for ZoomLayerCard and AnnotationLayerCard.
// Cards differ only in the store method, MIN_DURATION and colours; the pointer
// geometry (output-space delta mapped back through the display map) is identical.

import { frameStep } from "./timeline-helpers";
import { type SnapTarget, snapTime } from "./timeline-snap";

/**
 * Pointer travel before a press becomes a drag. Without it a 1px tremor during
 * a click-to-select moved the card and pushed an undo entry.
 */
export const DRAG_THRESHOLD_PX = 3;

/** Pointer-travel multiplier while the precision modifier (Shift) is held. */
export const PRECISION_SCALE = 0.25;

/** Whether a press has travelled far enough to be a drag rather than a click. */
export function dragEngaged(clientX: number, startClientX: number): boolean {
	return Math.abs(clientX - startClientX) >= DRAG_THRESHOLD_PX;
}

export interface CardBounds {
	start: number;
	end: number;
}

export interface CardDragResult {
	start: number;
	end: number;
	guide: SnapTarget | null;
}

// Everything a pointer gesture needs: the anchor bounds captured at pointer-down,
// the current/start clientX, and the mappers that project through the collapsed axis.
export interface CardDragGeometry {
	origin: CardBounds;
	clientX: number;
	startClientX: number;
	xOf: (t: number) => number;
	tOf: (x: number) => number;
	snapTargets: SnapTarget[];
	tolerance: number;
	fps: number;
	duration: number;
	/** Pointer-travel multiplier; `PRECISION_SCALE` for a fine drag. Callers
	 *  re-seed `origin`/`startClientX` when it changes, so it never jumps. */
	scale?: number;
}

// Move an original-time anchor by the pointer's output-space delta, so the card
// tracks the cursor on the collapsed (post-cut) axis.
//
// The delta stays in PIXELS. It used to be divided by `pps` before being added
// to `xOf(orig)`, which is pixels — so the card advanced by delta/pps² seconds
// and a 150px drag at 100px/s moved it 15ms instead of 1.5s. Dragging a zoom or
// markup card looked like it did nothing.
function projectAnchor(g: CardDragGeometry, orig: number): number {
	const deltaPx = (g.clientX - g.startClientX) * (g.scale ?? 1);
	return g.tOf(g.xOf(orig) + deltaPx);
}

// Translate the whole card, snapping whichever edge is closer to a target so it
// butts against neighbours from either side; span is preserved and clamped in [0, duration].
export function computeCardMove(g: CardDragGeometry): CardDragResult {
	const span = g.origin.end - g.origin.start;
	const proposed = projectAnchor(g, g.origin.start);

	const startSnap = snapTime(proposed, g.snapTargets, g.tolerance, g.fps);
	const endSnap = snapTime(proposed + span, g.snapTargets, g.tolerance, g.fps);
	const startDist = startSnap.target ? Math.abs(startSnap.time - proposed) : Infinity;
	const endDist = endSnap.target ? Math.abs(endSnap.time - (proposed + span)) : Infinity;

	let nextStart: number;
	let guide: SnapTarget | null = null;
	if (startSnap.target && startDist <= endDist) {
		nextStart = startSnap.time;
		guide = startSnap.target;
	} else if (endSnap.target) {
		nextStart = endSnap.time - span;
		guide = endSnap.target;
	} else {
		nextStart = startSnap.time; // frame-quantised fallback
	}

	nextStart = Math.max(0, Math.min(g.duration - span, nextStart));
	return { start: nextStart, end: nextStart + span, guide };
}

// Move one edge, holding the other and keeping at least minDuration between them.
export function computeCardResize(
	g: CardDragGeometry & { edge: "start" | "end"; minDuration: number },
): CardDragResult {
	if (g.edge === "start") {
		const proposed = projectAnchor(g, g.origin.start);
		const snap = snapTime(proposed, g.snapTargets, g.tolerance, g.fps);
		const next = Math.max(0, Math.min(g.origin.end - g.minDuration, snap.time));
		return { start: next, end: g.origin.end, guide: snap.target };
	}
	const proposed = projectAnchor(g, g.origin.end);
	const snap = snapTime(proposed, g.snapTargets, g.tolerance, g.fps);
	const next = Math.min(g.duration, Math.max(g.origin.start + g.minDuration, snap.time));
	return { start: g.origin.start, end: next, guide: snap.target };
}

// Keyboard nudge: Shift = 1s, plain = one frame. Alt resizes the trailing edge
// instead of translating; otherwise span is preserved and clamped in [0, duration].
export function computeCardNudge(p: {
	origin: CardBounds;
	direction: 1 | -1;
	shift: boolean;
	alt: boolean;
	fps: number;
	duration: number;
	minDuration: number;
}): CardBounds {
	const delta = p.direction * (p.shift ? 1 : frameStep(p.fps));
	if (p.alt) {
		const next = Math.min(
			p.duration,
			Math.max(p.origin.start + p.minDuration, p.origin.end + delta),
		);
		return { start: p.origin.start, end: next };
	}
	const span = p.origin.end - p.origin.start;
	let nextStart = p.origin.start + delta;
	nextStart = Math.max(0, Math.min(p.duration - span, nextStart));
	return { start: nextStart, end: nextStart + span };
}
