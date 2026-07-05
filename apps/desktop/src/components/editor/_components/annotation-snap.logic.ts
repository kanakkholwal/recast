// Snap-anchor construction shared by the 2D annotation overlay and the HTML
// text layer. Both build the same frame-edge + per-annotation box anchor set,
// so they MUST stay in one place or the two surfaces snap to different guides.

import { normaliseBox } from "$lib/annotations/uv";
import { FRAME_ANCHORS, type SnapAnchor } from "$lib/annotations/snap";
import type { Annotation } from "$lib/stores/editor-store.svelte";

/**
 * Frame edges/centres plus every other annotation's box edges/centres, in UV.
 * `excludeId` drops the annotation currently being dragged so it doesn't snap
 * to itself; hidden annotations are ignored. Arrows contribute their two
 * endpoints; box-shaped kinds contribute left/centre/right and top/middle/
 * bottom via the canonical normalised box.
 */
export function buildAnnotationSnapAnchors(
	annotations: Annotation[],
	excludeId: string | null,
): SnapAnchor[] {
	const anchors: SnapAnchor[] = [...FRAME_ANCHORS];
	for (const a of annotations) {
		if (a.id === excludeId) continue;
		if (a.hidden) continue;
		if (a.kind.kind === "arrow") {
			anchors.push({ axis: "x", value: a.kind.x1 });
			anchors.push({ axis: "y", value: a.kind.y1 });
			anchors.push({ axis: "x", value: a.kind.x2 });
			anchors.push({ axis: "y", value: a.kind.y2 });
			continue;
		}
		const box = normaliseBox(a.kind);
		anchors.push({ axis: "x", value: box.x });
		anchors.push({ axis: "x", value: box.x + box.w / 2 });
		anchors.push({ axis: "x", value: box.x + box.w });
		anchors.push({ axis: "y", value: box.y });
		anchors.push({ axis: "y", value: box.y + box.h / 2 });
		anchors.push({ axis: "y", value: box.y + box.h });
	}
	return anchors;
}
