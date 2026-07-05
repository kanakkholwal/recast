// Shared drag/resize/nudge maths for ZoomLayerCard and AnnotationLayerCard.
// Cards differ only in the store method, MIN_DURATION and colours; the pointer
// geometry (output-space delta mapped back through the display map) is identical.

import { frameStep } from "./timeline-helpers";
import { snapTime, type SnapTarget } from "./timeline-snap";

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
	pps: number;
	xOf: (t: number) => number;
	tOf: (x: number) => number;
	snapTargets: SnapTarget[];
	tolerance: number;
	fps: number;
	duration: number;
}

// Move an original-time anchor by the pointer's output-space delta, so the card
// tracks the cursor on the collapsed (post-cut) axis.
function projectAnchor(g: CardDragGeometry, orig: number): number {
	const outDelta = (g.clientX - g.startClientX) / g.pps;
	return g.tOf(g.xOf(orig) + outDelta);
}

// Translate the whole card, snapping whichever edge is closer to a target so it
// butts against neighbours from either side; span is preserved and clamped in [0, duration].
export function computeCardMove(g: CardDragGeometry): CardDragResult {
	const span = g.origin.end - g.origin.start;
	const proposed = projectAnchor(g, g.origin.start);

	const startSnap = snapTime(proposed, g.snapTargets, g.tolerance, g.fps);
	const endSnap = snapTime(proposed + span, g.snapTargets, g.tolerance, g.fps);
	const startDist = startSnap.target
		? Math.abs(startSnap.time - proposed)
		: Infinity;
	const endDist = endSnap.target
		? Math.abs(endSnap.time - (proposed + span))
		: Infinity;

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
		const next = Math.max(
			0,
			Math.min(g.origin.end - g.minDuration, snap.time),
		);
		return { start: next, end: g.origin.end, guide: snap.target };
	}
	const proposed = projectAnchor(g, g.origin.end);
	const snap = snapTime(proposed, g.snapTargets, g.tolerance, g.fps);
	const next = Math.min(
		g.duration,
		Math.max(g.origin.start + g.minDuration, snap.time),
	);
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
