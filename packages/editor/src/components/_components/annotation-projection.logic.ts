import { evalZoom, type ZoomRegionLike } from "$lib/annotations/eval";
import type { AnnotationAnchor } from "$lib/stores/editor-store.svelte";
import { IDENTITY_ZOOM } from "./annotation-draw.logic";

/**
 * Zoom transform an annotation is projected through, shared by the canvas
 * overlay and the HTML text layer so the two can't diverge.
 *
 * Frame-anchored markup ignores zoom by definition. `focusEnabled` off is the
 * subtler case: the composite draws unzoomed (frame-params.ts) and the export
 * drops the regions outright (services/export.ts), so applying zoom here would
 * put preview markup where neither the picture nor the file puts it.
 */
export function annotationZoom(
	anchor: AnnotationAnchor | undefined,
	zoomRegions: ZoomRegionLike[],
	t: number,
	focusEnabled: boolean,
) {
	if (anchor === "frame" || !focusEnabled) return IDENTITY_ZOOM;
	return evalZoom(zoomRegions, t);
}
