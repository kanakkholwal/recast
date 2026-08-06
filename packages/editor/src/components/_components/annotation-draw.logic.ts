/**
 * Editor-only annotation helpers (handle sizes, identity zoom, cursor mapping).
 * The headless draw primitives (arrow/roundRect/dash/tint/withAlpha) now live in
 * `@recast/render` so preview and export share one renderer; re-exported here so
 * existing importers keep one import site.
 */

import type { HandleName } from "../../lib/annotations/hit";
import type { ZoomTransform } from "../../lib/annotations/eval";
export {
	type StrokeStyle,
	type Point,
	type ArrowGeometry,
	strokeDashPattern,
	blurTint,
	withAlpha,
	arrowGeometry,
	roundRectPath,
} from "@recast/render";

/** CSS-px half-size of an annotation resize handle (also the grab-slop base). */
export const HANDLE_RADIUS_PX = 5.5;
/** CSS-px corner radius drawn on annotation resize handles. */
export const HANDLE_CORNER_PX = 2;

/** Zoom transform that maps UV straight through, used for frame-anchored annotations, which ignore zoom. */
export const IDENTITY_ZOOM: ZoomTransform = { scale: 1, cx: 0.5, cy: 0.5 };

/**
 * CSS cursor for a hovered annotation handle. Distinct from the focus overlay's
 * map: annotations add the `tool` (placement) and arrow `p1`/`p2` states, and
 * body hovers read as `grab` rather than `move`.
 */
export function cursorForHandle(h: HandleName | "tool" | null): string {
	if (h === "tool") return "crosshair";
	switch (h) {
		case "nw":
		case "se":
			return "nwse-resize";
		case "ne":
		case "sw":
			return "nesw-resize";
		case "n":
		case "s":
			return "ns-resize";
		case "e":
		case "w":
			return "ew-resize";
		case "p1":
		case "p2":
			return "crosshair";
		case "body":
			return "grab";
		default:
			return "default";
	}
}
