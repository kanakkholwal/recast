/**
 * Draw the whole annotation layer for the export at original time `t` into a
 * comp-native 2D context, reusing the SAME projection + paint the preview does
 * (so preview == export). Blur + text are skipped here: blur is a P1b framebuffer
 * pass, text a P1b rasterize step, and projects containing them still route to
 * the Rust compositor (see browserExportBlockedReason).
 */

import { paintArrow, paintBoxAnnotation, type ShapeImage } from "@recast/render";
import { evalOpacity, evalZoom } from "$lib/annotations/eval";
import { IDENTITY_ZOOM } from "../../components/editor/_components/annotation-draw.logic";
import { compositionRectPx, uvToCanvas, videoRectPx } from "$lib/annotations/uv";
import { normaliseBox } from "@recast/render";
import type { Annotation, OutputAspect, VideoMetadata } from "$lib/stores/editor-store.svelte";
import type { ZoomRegionLike } from "$lib/annotations/eval";

export interface AnnotationLayerInputs {
	/** Z-ordered annotations (store.annotationsByZ). */
	annotations: ReadonlyArray<Annotation>;
	meta: Pick<VideoMetadata, "width" | "height">;
	padding: number;
	outputAspect: OutputAspect;
	zoomRegions: ZoomRegionLike[];
	/** Comp-native render buffer size (px) — the layer canvas dimensions. */
	canvasPxW: number;
	canvasPxH: number;
	/** Decoded image for an image annotation, or null when unavailable. */
	getImage: (path: string) => ShapeImage | null;
}

/** True when a kind is drawn by the P1a painted path (rect/ellipse/arrow/image). */
function isPainted(kind: string): boolean {
	return kind === "rect" || kind === "ellipse" || kind === "arrow" || kind === "image";
}

/** Paint every painted annotation into `ctx` at original time `t`. Caller clears
 *  the canvas first and uploads it as the frame's annotation texture. */
export function drawAnnotationLayerExport(
	ctx: OffscreenCanvasRenderingContext2D,
	t: number,
	i: AnnotationLayerInputs,
): void {
	const videoRect = videoRectPx(i.canvasPxW, i.canvasPxH, i.meta, i.padding, i.outputAspect);
	const compRect = compositionRectPx(i.canvasPxW, i.canvasPxH, i.meta, i.padding, i.outputAspect);
	for (const a of i.annotations) {
		if (a.hidden) continue;
		if (!isPainted(a.kind.kind)) continue;
		const opacity = evalOpacity(a, t);
		if (opacity <= 0) continue;
		const frame = a.anchor === "frame";
		const rect = frame ? compRect : videoRect;
		const zoom = frame ? IDENTITY_ZOOM : evalZoom(i.zoomRegions, t);
		if (a.kind.kind === "arrow") {
			const p1 = uvToCanvas(a.kind.x1, a.kind.y1, rect, zoom);
			const p2 = uvToCanvas(a.kind.x2, a.kind.y2, rect, zoom);
			paintArrow(ctx, a, p1, p2, rect.w, opacity);
		} else {
			const box = normaliseBox(a.kind);
			const tl = uvToCanvas(box.x, box.y, rect, zoom);
			const br = uvToCanvas(box.x + box.w, box.y + box.h, rect, zoom);
			paintBoxAnnotation(
				ctx,
				a,
				{ x: tl.x, y: tl.y, w: br.x - tl.x, h: br.y - tl.y },
				rect.w,
				opacity,
				{ getImage: i.getImage, dpr: 1 },
			);
		}
	}
}
