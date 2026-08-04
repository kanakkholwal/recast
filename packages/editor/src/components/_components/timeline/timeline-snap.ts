import { quantizeToFrame } from "./timeline-helpers";

// Snap resolver for drag/resize on region cards. Returns both the snapped
// value and which target produced it so callers can draw a guide at the snap.

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

export function snapLabel(kind: SnapKind): string {
	switch (kind) {
		case "playhead":
			return "Playhead";
		case "in-point":
			return "In";
		case "out-point":
			return "Out";
		case "origin":
			return "Start";
		case "duration":
			return "End";
		case "region-start":
			return "Region start";
		case "region-end":
			return "Region end";
		case "annotation-start":
			return "Annotation start";
		case "annotation-end":
			return "Annotation end";
		case "frame":
			return "Frame";
	}
}

export interface SnapResult {
	time: number;
	target: SnapTarget | null;
}

// Lock to the nearest target within `toleranceTime`, else fall through to the
// frame grid so writes never land sub-frame. Targets are checked in array order;
// pass the most intentful first (playhead, in/out) so they win ties.
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

export interface BuildTargetsArgs {
	playhead: number;
	inPoint: number;
	outPoint: number;
	duration: number;
	regions: ReadonlyArray<{ id: string; start: number; end: number }>;
	annotations?: ReadonlyArray<{ id: string; start: number; end: number }>;
	/** Id of the moving card so it doesn't snap to its own edges. */
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
