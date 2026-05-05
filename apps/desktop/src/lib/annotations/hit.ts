// Hit-testing primitives for annotations: handle picking, body picking, and
// topmost selection. All functions take container-pixel inputs and return
// container-pixel positions or string handle IDs — no DOM dependencies.

import type { Annotation } from "$lib/stores/editor-store.svelte";
import { type ZoomRegionLike } from "./eval";
import { evalZoom } from "./eval";
import { normaliseBox, uvToCanvas, type Rect } from "./uv";

export type HandleName =
	| "nw" | "n" | "ne" | "e" | "se" | "s" | "sw" | "w"
	| "body"
	| "p1" | "p2"; // arrow endpoints

export interface Point {
	x: number;
	y: number;
}

/** Shortest pixel distance from `p` to the segment `a→b`. */
export function pointToSegmentDist(p: Point, a: Point, b: Point): number {
	const dx = b.x - a.x;
	const dy = b.y - a.y;
	const lenSq = dx * dx + dy * dy;
	if (lenSq === 0) return Math.hypot(p.x - a.x, p.y - a.y);
	const t = Math.max(
		0,
		Math.min(1, ((p.x - a.x) * dx + (p.y - a.y) * dy) / lenSq),
	);
	const cx = a.x + t * dx;
	const cy = a.y + t * dy;
	return Math.hypot(p.x - cx, p.y - cy);
}

/** 8 selection handles for a box. */
export function handlePositions(
	x: number,
	y: number,
	w: number,
	h: number,
): Record<"nw" | "n" | "ne" | "e" | "se" | "s" | "sw" | "w", Point> {
	return {
		nw: { x, y },
		n: { x: x + w / 2, y },
		ne: { x: x + w, y },
		e: { x: x + w, y: y + h / 2 },
		se: { x: x + w, y: y + h },
		s: { x: x + w / 2, y: y + h },
		sw: { x, y: y + h },
		w: { x, y: y + h / 2 },
	};
}

export interface HitOptions {
	rect: Rect;
	zoomRegions: ZoomRegionLike[];
	t: number;
	/** Slop in container pixels (already includes DPR scaling if relevant). */
	handleSlop: number;
	/** Tolerance for treating a click on the arrow line as a body hit. */
	lineSlop: number;
	/** Tolerance for selecting an annotation by clicking on its body. */
	annotationSlop: number;
}

/**
 * Returns which handle (or "body") of the given annotation is under `pt`, or
 * `null` if the pointer is outside.
 */
export function hitTestHandle(
	pt: Point,
	a: Annotation,
	opts: HitOptions,
): HandleName | null {
	if (a.hidden) return null;
	const zoom = evalZoom(opts.zoomRegions, opts.t);

	if (a.kind.kind === "arrow") {
		const p1 = uvToCanvas(a.kind.x1, a.kind.y1, opts.rect, zoom);
		const p2 = uvToCanvas(a.kind.x2, a.kind.y2, opts.rect, zoom);
		if (
			Math.abs(pt.x - p1.x) <= opts.handleSlop &&
			Math.abs(pt.y - p1.y) <= opts.handleSlop
		) {
			return "p1";
		}
		if (
			Math.abs(pt.x - p2.x) <= opts.handleSlop &&
			Math.abs(pt.y - p2.y) <= opts.handleSlop
		) {
			return "p2";
		}
		if (pointToSegmentDist(pt, p1, p2) <= opts.lineSlop) return "body";
		return null;
	}

	const box = normaliseBox(a.kind);
	const topLeft = uvToCanvas(box.x, box.y, opts.rect, zoom);
	const bottomRight = uvToCanvas(box.x + box.w, box.y + box.h, opts.rect, zoom);
	const x = topLeft.x;
	const y = topLeft.y;
	const w = bottomRight.x - topLeft.x;
	const h = bottomRight.y - topLeft.y;

	const handles = handlePositions(x, y, w, h);
	for (const [name, p] of Object.entries(handles)) {
		if (
			Math.abs(pt.x - p.x) <= opts.handleSlop &&
			Math.abs(pt.y - p.y) <= opts.handleSlop
		) {
			return name as HandleName;
		}
	}
	if (pt.x >= x && pt.x <= x + w && pt.y >= y && pt.y <= y + h) return "body";
	return null;
}

/**
 * Returns the topmost annotation under `pt`, respecting hidden + locked. Text
 * annotations are skipped (they live in a separate HTML layer with its own
 * hit-test).
 *
 * Caller passes the annotations list ordered by z (lowest first). The function
 * iterates in reverse so the highest-z hit wins, matching draw order.
 */
export function hitTestAnnotation(
	pt: Point,
	annotations: Annotation[],
	opts: HitOptions,
): Annotation | null {
	const zoom = evalZoom(opts.zoomRegions, opts.t);
	for (let i = annotations.length - 1; i >= 0; i--) {
		const a = annotations[i];
		if (a.hidden) continue;
		if (a.locked) continue;
		if (a.kind.kind === "text") continue;
		// Hit-test against the visibility *window* rather than the per-frame
		// opacity. The fade-in ramp briefly drops opacity below the old 0.05
		// threshold right after creation, which previously made fresh
		// annotations un-selectable until the ramp finished. Skipping by window
		// keeps selection responsive while still ignoring annotations that
		// haven't started or have already ended.
		if (opts.t < a.start || opts.t > a.end) continue;

		if (a.kind.kind === "arrow") {
			const p1 = uvToCanvas(a.kind.x1, a.kind.y1, opts.rect, zoom);
			const p2 = uvToCanvas(a.kind.x2, a.kind.y2, opts.rect, zoom);
			if (pointToSegmentDist(pt, p1, p2) <= opts.annotationSlop) return a;
			continue;
		}

		const box = normaliseBox(a.kind);
		const topLeft = uvToCanvas(box.x, box.y, opts.rect, zoom);
		const bottomRight = uvToCanvas(
			box.x + box.w,
			box.y + box.h,
			opts.rect,
			zoom,
		);
		if (
			pt.x >= topLeft.x &&
			pt.x <= bottomRight.x &&
			pt.y >= topLeft.y &&
			pt.y <= bottomRight.y
		) {
			return a;
		}
	}
	return null;
}
