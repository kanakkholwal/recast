// Snap engine for annotation drag/place. Pure: takes a candidate UV point and
// a list of anchors, returns the snapped point plus whichever anchors fired
// (so the overlay can draw guides). Stores no state; the caller decides when
// to invoke it (e.g. only on move-deltas above a hysteresis threshold).

export type SnapAxis = "x" | "y";

export interface SnapAnchor {
	axis: SnapAxis;
	value: number; // UV (0..1)
	/** Optional label for telemetry / debug; not rendered. */
	label?: string;
}

export interface SnapResult {
	x: number;
	y: number;
	guides: SnapAnchor[];
	snapped: boolean;
}

/**
 * Snap `(ux, uy)` against anchors within `tolerance` (UV units). Returns the
 * nearest anchor on each axis (at most one per axis). When `enabled` is false
 * or the closest anchor is outside tolerance, the input is returned unchanged.
 *
 * Tolerance default 0.005 UV ≈ 5 px at 1080p.
 */
export function snap(
	ux: number,
	uy: number,
	anchors: SnapAnchor[],
	tolerance = 0.005,
	enabled = true,
): SnapResult {
	if (!enabled || anchors.length === 0) {
		return { x: ux, y: uy, guides: [], snapped: false };
	}

	let bestX: SnapAnchor | null = null;
	let bestY: SnapAnchor | null = null;
	let bestDx = tolerance;
	let bestDy = tolerance;

	for (const a of anchors) {
		if (a.axis === "x") {
			const d = Math.abs(a.value - ux);
			if (d <= bestDx) {
				bestDx = d;
				bestX = a;
			}
		} else {
			const d = Math.abs(a.value - uy);
			if (d <= bestDy) {
				bestDy = d;
				bestY = a;
			}
		}
	}

	const guides: SnapAnchor[] = [];
	let outX = ux;
	let outY = uy;
	if (bestX) {
		outX = bestX.value;
		guides.push(bestX);
	}
	if (bestY) {
		outY = bestY.value;
		guides.push(bestY);
	}

	return { x: outX, y: outY, guides, snapped: guides.length > 0 };
}

/** Standard frame-anchor set: 0, 0.5, 1 on both axes. */
export const FRAME_ANCHORS: SnapAnchor[] = [
	{ axis: "x", value: 0, label: "frame-left" },
	{ axis: "x", value: 0.5, label: "frame-center-x" },
	{ axis: "x", value: 1, label: "frame-right" },
	{ axis: "y", value: 0, label: "frame-top" },
	{ axis: "y", value: 0.5, label: "frame-center-y" },
	{ axis: "y", value: 1, label: "frame-bottom" },
];
