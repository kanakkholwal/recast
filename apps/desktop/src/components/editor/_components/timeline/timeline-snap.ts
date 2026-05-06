import { quantizeToFrame } from "./timeline-helpers";

// Snap resolver shared by drag/resize on zoom region cards. Pure module —
// returns both the snapped value and which target produced it, so callers
// can render a guide line at the active snap.

export type SnapKind =
	| "playhead"
	| "in-point"
	| "out-point"
	| "duration"
	| "origin"
	| "region-start"
	| "region-end"
	| "annotation-start"
	| "annotation-end"
	| "frame";

export interface SnapTarget {
	time: number;
	kind: SnapKind;
}

export interface SnapResult {
	time: number;
	target: SnapTarget | null;
}

// Resolve a candidate time against the supplied targets. Within `toleranceTime`
// of any target we lock to that target; otherwise we fall through to the
// frame grid so writes never land at sub-frame fractions.
//
// Targets are checked in array order — pass them with the most "intentful"
// first (playhead, in/out) so they win ties against neighbour edges.
export function snapTime(
	candidate: number,
	targets: SnapTarget[],
	toleranceTime: number,
	fps: number,
): SnapResult {
	let best: SnapTarget | null = null;
	let bestDist = toleranceTime;
	for (const t of targets) {
		const d = Math.abs(t.time - candidate);
		if (d <= bestDist) {
			best = t;
			bestDist = d;
		}
	}
	if (best) return { time: best.time, target: best };
	return { time: quantizeToFrame(candidate, fps), target: null };
}

// Convenience: produce the standard target list a zoom-card drag wants to
// snap against. Pass `excludeRegionId` to keep a card from snapping to its
// own edges while it's being moved.
export interface BuildTargetsArgs {
	playhead: number;
	inPoint: number;
	outPoint: number;
	duration: number;
	regions: ReadonlyArray<{ id: string; start: number; end: number }>;
	annotations?: ReadonlyArray<{ id: string; start: number; end: number }>;
	/**
	 * Range identifier(s) to omit from the target list — pass the moving
	 * card's id so it doesn't snap to its own edges. Accepts either a
	 * single id (zoom case) or a structured exclude (annotation case).
	 */
	excludeRegionId?: string | null;
	excludeAnnotationId?: string | null;
}

export function buildSnapTargets(args: BuildTargetsArgs): SnapTarget[] {
	const targets: SnapTarget[] = [
		{ time: args.playhead, kind: "playhead" },
		{ time: args.inPoint, kind: "in-point" },
		{ time: args.outPoint, kind: "out-point" },
		{ time: 0, kind: "origin" },
		{ time: args.duration, kind: "duration" },
	];
	for (const r of args.regions) {
		if (r.id === args.excludeRegionId) continue;
		targets.push({ time: r.start, kind: "region-start" });
		targets.push({ time: r.end, kind: "region-end" });
	}
	for (const a of args.annotations ?? []) {
		if (a.id === args.excludeAnnotationId) continue;
		targets.push({ time: a.start, kind: "annotation-start" });
		targets.push({ time: a.end, kind: "annotation-end" });
	}
	return targets;
}
