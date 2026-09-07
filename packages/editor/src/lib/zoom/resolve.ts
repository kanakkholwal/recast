/**
 * Which zoom region owns a moment in time, and which regions overlap.
 *
 * Only one region can apply at a time. Both TS evaluators (the preview shader's
 * `evaluateZoomAt` and the overlay `evalZoom`) used to take the first match in
 * ARRAY order, so with overlapping regions the winner depended on creation
 * order — invisible to the user, and inconsistent with the start-time order the
 * panel lists them in.
 */

export interface ZoomWindow {
	id?: string;
	start: number;
	end: number;
	hidden?: boolean;
}

function contains(r: ZoomWindow, t: number) {
	return !r.hidden && t > r.start && t < r.end;
}

/**
 * Index of the region that applies at `t`, or -1.
 *
 * With `scaleAt`, the TIGHTEST region wins. That is what keeps an overlap from
 * flickering: the winner can only change where the two scales are equal, so the
 * zoom is continuous across the handover. Picking by latest start instead
 * snapped 2.00x to 1.03x in a single frame, because the incoming region's ramp
 * restarts at 1. Mirrors `active_zoom` in `crates/recast-compositor/src/eval.rs`.
 *
 * Without it (callers that only know the windows, like the overlap warning) the
 * latest-starting region wins: a short region nested inside a long one is the
 * more specific intent, and the rule can't depend on array order. That is also
 * the tiebreak when two scales are equal.
 */
export function activeZoomIndex(
	regions: readonly ZoomWindow[],
	t: number,
	scaleAt?: (index: number) => number,
): number {
	let best = -1;
	for (let i = 0; i < regions.length; i++) {
		if (!contains(regions[i], t)) continue;
		if (best === -1) {
			best = i;
			continue;
		}
		if (scaleAt) {
			const [a, b] = [scaleAt(i), scaleAt(best)];
			if (a > b || (a === b && regions[i].start >= regions[best].start)) best = i;
			continue;
		}
		if (regions[i].start >= regions[best].start) best = i;
	}
	return best;
}

/**
 * Ids of every visible region that shares time with another. Regions that only
 * touch at a boundary don't overlap. Drives the panel's overlap warning —
 * overlap is ambiguous in preview and, worse, the FFmpeg export SUMS the
 * regions' zoom rather than picking one.
 */
export function overlappingZoomIds(regions: readonly ZoomWindow[]): string[] {
	const visible = regions.filter((r) => !r.hidden && r.id !== undefined);
	const hit = new Set<string>();
	for (let i = 0; i < visible.length; i++) {
		for (let j = i + 1; j < visible.length; j++) {
			const a = visible[i];
			const b = visible[j];
			if (a.start < b.end && b.start < a.end) {
				hit.add(a.id as string);
				hit.add(b.id as string);
			}
		}
	}
	return [...hit];
}
