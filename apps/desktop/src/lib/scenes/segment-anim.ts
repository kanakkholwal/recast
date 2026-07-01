/**
 * Per-segment scene animations — how a clip animates INTO and OUT OF view.
 *
 * Sibling to per-segment speed (../timeline/segment-speed.ts): an override is
 * anchored to the segment's ORIGINAL start time, which is stable under cuts and
 * ripple-deletes. Unlike speed (a time-domain warp on the shared time-map), an
 * animation is a purely visual transform on the video layer — opacity, position
 * and scale over the segment's window — evaluated in ../scenes/eval.ts
 * and mirrored by the Rust export graph. It never changes duration or timing.
 */

import { EASE_OUT, EASE_IN, BOUNCE, type Easing } from "../easing/cubic-bezier";
import type { Segment } from "../timeline/segments";

/** Tolerance for matching an anchor to a segment start. Matches segments.ts. */
const EPS = 1e-4;

export const MIN_ANIM_MS = 100;
export const MAX_ANIM_MS = 2000;
export const DEFAULT_ANIM_MS = 500;

// Per-kind magnitude defaults. Shared by the evaluator (../scenes/eval.ts) and
// the intensity slider (intensityRange, below); mirrored verbatim in the Rust
// export (render/scene_anim.rs) — keep the three in lockstep.
export const DEFAULT_SLIDE = 0.6; // fraction of the canvas travelled
export const DEFAULT_SCALE_DELTA = 0.3; // grow-from / settle-to delta
export const DEFAULT_POP_DELTA = 0.35;
export const DEFAULT_ROTATE_DEG = 15; // start angle for a rotate-in

/** The visual primitive an animation drives. `pop` and `shrink` are scale
 * variants (`pop` grows past 1 via an overshoot ease; `shrink` settles down
 * from larger). `rotate` spins the whole card about its centre. */
export type SceneAnimKind = "fade" | "slide" | "scale" | "shrink" | "pop" | "rotate";
export type SceneAnimDir = "left" | "right" | "up" | "down";

/** UI metadata for a kind's `intensity` control, or null when the kind has no
 * adjustable magnitude (fade). Ranges are generous but clamped to sane bounds. */
export function intensityRange(
	kind: SceneAnimKind,
): { label: string; min: number; max: number; step: number; unit: string; default: number } | null {
	switch (kind) {
		case "slide":
			return { label: "Distance", min: 0.1, max: 1.5, step: 0.05, unit: "×", default: DEFAULT_SLIDE };
		case "scale":
			return { label: "Amount", min: 0.05, max: 0.9, step: 0.05, unit: "", default: DEFAULT_SCALE_DELTA };
		case "pop":
			return { label: "Amount", min: 0.05, max: 0.9, step: 0.05, unit: "", default: DEFAULT_POP_DELTA };
		case "shrink":
			return { label: "Amount", min: 0.05, max: 0.9, step: 0.05, unit: "", default: DEFAULT_SCALE_DELTA };
		case "rotate":
			return { label: "Angle", min: 5, max: 180, step: 5, unit: "°", default: DEFAULT_ROTATE_DEG };
		default:
			return null; // fade
	}
}

/** One side (entrance or exit) of a segment's animation. */
export interface SceneAnimSpec {
	kind: SceneAnimKind;
	/** Ramp length in OUTPUT-time milliseconds (what the viewer perceives). */
	durationMs: number;
	easing: Easing;
	/** Slide only — the direction the clip travels FROM (in) / TO (out). */
	dir?: SceneAnimDir;
	/** Kind-specific magnitude: slide distance (fraction of canvas) or scale
	 *  delta. Omitted → the per-kind default in `presenceTransform`. */
	intensity?: number;
}

/** Entrance and/or exit animation pinned to the segment starting at `start`. */
export interface SegmentAnim {
	/** Segment's original start time (seconds) — the stable anchor. */
	start: number;
	in?: SceneAnimSpec;
	out?: SceneAnimSpec;
}

/** Clamp a ramp length to the supported range. */
export function clampAnimMs(ms: number): number {
	if (!Number.isFinite(ms)) return DEFAULT_ANIM_MS;
	return Math.min(MAX_ANIM_MS, Math.max(MIN_ANIM_MS, ms));
}

/** A ready-to-use spec for `kind`, with sensible defaults for a fresh pick. */
export function defaultSpec(kind: SceneAnimKind, side: "in" | "out"): SceneAnimSpec {
	const easing = kind === "pop" ? BOUNCE : side === "in" ? EASE_OUT : EASE_IN;
	const spec: SceneAnimSpec = { kind, durationMs: DEFAULT_ANIM_MS, easing };
	if (kind === "slide") spec.dir = side === "in" ? "left" : "right";
	return spec;
}

/** The animation anchored at original `start`, or null when unset. */
export function segmentAnimAt(
	overrides: ReadonlyArray<SegmentAnim>,
	start: number,
): SegmentAnim | null {
	for (const o of overrides) {
		if (Math.abs(o.start - start) <= EPS) return o;
	}
	return null;
}

/**
 * Upsert the entrance (`in`) or exit (`out`) side of a segment's animation,
 * returning a new sorted array. Passing `null` clears that side; when both
 * sides end up empty the entry is dropped so the list stays sparse (and
 * serializes to nothing).
 */
export function setSegmentAnim(
	overrides: ReadonlyArray<SegmentAnim>,
	start: number,
	side: "in" | "out",
	spec: SceneAnimSpec | null,
): SegmentAnim[] {
	const existing = segmentAnimAt(overrides, start);
	const rest = overrides
		.filter((o) => Math.abs(o.start - start) > EPS)
		.map((o) => ({ ...o }));
	const next: SegmentAnim = {
		start,
		in: side === "in" ? spec ?? undefined : existing?.in,
		out: side === "out" ? spec ?? undefined : existing?.out,
	};
	if (!next.in && !next.out) {
		return rest.sort((a, b) => a.start - b.start);
	}
	return [...rest, next].sort((a, b) => a.start - b.start);
}

/** Drop overrides whose anchor no longer matches a current segment start. */
export function pruneSegmentAnims(
	overrides: ReadonlyArray<SegmentAnim>,
	segments: ReadonlyArray<Segment>,
): SegmentAnim[] {
	if (overrides.length === 0) return [];
	return overrides
		.filter((o) => segments.some((s) => Math.abs(s.start - o.start) <= EPS))
		.map((o) => ({ ...o }))
		.sort((a, b) => a.start - b.start);
}
