/**
 * Per-clip camera layouts. Same anchoring rule as `./segment-speed.ts`: a
 * layout is pinned to the segment's ORIGINAL start, which cuts and ripple
 * deletes do not move, and an anchor a trim or split orphans is dropped rather
 * than misplaced. Mirrors Rust `CameraClipLayout` and `recast_compositor::layout`.
 */

import type { CameraClipLayout, CameraLayout, LayoutSide } from "../editor/render-state";
import { MAX_SPLIT_FRACTION, MIN_SPLIT_FRACTION } from "../editor/render-state";
import type { Segment } from "./segments";

/** Tolerance for matching an anchor to a segment start. Matches segments.ts. */
const EPS = 1e-4;

export const DEFAULT_SPLIT_FRACTION = 0.35;

/** Clamp a split share so neither half can collapse. Mirrors Rust `split_fraction`. */
export function clampSplitFraction(fraction: number): number {
	if (!Number.isFinite(fraction)) return DEFAULT_SPLIT_FRACTION;
	return Math.min(MAX_SPLIT_FRACTION, Math.max(MIN_SPLIT_FRACTION, fraction));
}

/** The layout anchored at `start`, or the bubble when unset. */
export function layoutAtStart(
	layouts: ReadonlyArray<CameraClipLayout>,
	start: number,
): CameraLayout {
	for (const c of layouts) {
		if (Math.abs(c.start - start) <= EPS) return c.layout;
	}
	return { kind: "pip" };
}

/**
 * The layout of the segment CONTAINING original time `t`. Forward-biased at a
 * seam and held at the last segment once `t` runs past it, the same rule
 * `segmentSpeedAtTime` uses so the two never disagree about which clip the
 * playhead is in.
 */
export function layoutAtTime(
	segments: ReadonlyArray<Segment>,
	layouts: ReadonlyArray<CameraClipLayout>,
	t: number,
): CameraLayout {
	for (const s of segments) {
		if (t >= s.start - EPS && t < s.end - EPS) return layoutAtStart(layouts, s.start);
	}
	const last = segments[segments.length - 1];
	return last ? layoutAtStart(layouts, last.start) : { kind: "pip" };
}

/** The segment containing `t`, forward-biased at a seam; the last one past the end. */
export function segmentStartAt(segments: ReadonlyArray<Segment>, t: number): number | null {
	for (const s of segments) {
		if (t >= s.start - EPS && t < s.end - EPS) return s.start;
	}
	return segments[segments.length - 1]?.start ?? null;
}

/**
 * Upsert a clip's layout, returning a new sorted array. Setting it back to the
 * bubble removes the entry, so an untouched project serializes to nothing and
 * keeps rendering exactly as it did before layouts existed.
 */
export function setClipLayout(
	layouts: ReadonlyArray<CameraClipLayout>,
	start: number,
	layout: CameraLayout,
): CameraClipLayout[] {
	const rest = layouts
		.filter((c) => Math.abs(c.start - start) > EPS)
		.map((c) => ({ start: c.start, layout: { ...c.layout } }));
	if (layout.kind === "pip") {
		return rest.sort((a, b) => a.start - b.start);
	}
	const clamped: CameraLayout =
		layout.kind === "splitH" || layout.kind === "splitV"
			? { ...layout, fraction: clampSplitFraction(layout.fraction) }
			: { ...layout };
	return [...rest, { start, layout: clamped }].sort((a, b) => a.start - b.start);
}

/** Drop layouts whose anchor no longer matches a current segment start. */
export function pruneClipLayouts(
	layouts: ReadonlyArray<CameraClipLayout>,
	segments: ReadonlyArray<Segment>,
): CameraClipLayout[] {
	if (layouts.length === 0) return [];
	return layouts
		.filter((c) => segments.some((s) => Math.abs(s.start - c.start) <= EPS))
		.map((c) => ({ start: c.start, layout: { ...c.layout } }))
		.sort((a, b) => a.start - b.start);
}

/**
 * Which clip the layout controls edit: the selected one when it is still a real
 * clip, otherwise the one under the playhead. Clicking a camera clip and
 * editing its layout has to touch that clip, not whatever the playhead is over.
 */
export function editAnchor(
	segments: ReadonlyArray<Segment>,
	selectedStart: number | null,
	playheadSec: number,
): number | null {
	if (selectedStart !== null && segments.some((s) => Math.abs(s.start - selectedStart) <= EPS)) {
		return selectedStart;
	}
	return segmentStartAt(segments, playheadSec);
}

/** A layout with `side` flipped, for the panel's side toggle. */
export function withSide(layout: CameraLayout, side: LayoutSide): CameraLayout {
	if (layout.kind !== "splitH" && layout.kind !== "splitV") return layout;
	return { ...layout, side };
}

/** A layout with a new split share, clamped. */
export function withFraction(layout: CameraLayout, fraction: number): CameraLayout {
	if (layout.kind !== "splitH" && layout.kind !== "splitV") return layout;
	return { ...layout, fraction: clampSplitFraction(fraction) };
}

/** Human names for each layout, in the order they read as a progression. One
 *  source for the panel picker and the timeline clip labels. */
export const LAYOUT_LABELS: Array<{ kind: CameraLayout["kind"]; label: string }> = [
	{ kind: "pip", label: "Bubble" },
	{ kind: "splitH", label: "Side by side" },
	{ kind: "splitV", label: "Stacked" },
	{ kind: "screenOnly", label: "Screen only" },
	{ kind: "cameraOnly", label: "Camera only" },
];

/** What to call this layout on a timeline clip or in the panel. */
export function layoutLabel(layout: CameraLayout): string {
	return LAYOUT_LABELS.find((l) => l.kind === layout.kind)?.label ?? "Bubble";
}
