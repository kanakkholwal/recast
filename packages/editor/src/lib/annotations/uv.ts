// Pure UV ↔ canvas geometry shared by the 2D annotation overlay and the HTML
// text layer. Both MUST use the same math or they drift when zoom/padding change.

import { computeCanvasGeometry } from "../canvas-geometry";
import {
	framePaddingPixels,
	type OutputAspect,
	type VideoMetadata,
} from "../../stores/editor-store.svelte";
import { evalZoom, type ZoomRegionLike } from "./eval";
import { canvasToUV, normaliseBox, uvToCanvas, type Rect } from "@recast/render";

// The pure projection (Rect, uvToCanvas, canvasToUV, normaliseBox) now lives in
// @recast/render; re-exported so existing importers of `./uv` keep one site.
export { canvasToUV, normaliseBox, uvToCanvas, type Rect };

/** Composition (frame + padding) width in source pixels. */
export function compositionWidth(
	metadata: Pick<VideoMetadata, "width" | "height"> | null | undefined,
	paddingPercent: number,
): number {
	if (!metadata) return 0;
	const padPx = framePaddingPixels(paddingPercent, metadata);
	return metadata.width + padPx * 2;
}

/**
 * Device-pixel rect of the video region inside a `containerW × containerH`
 * element. The container's aspect tracks `outputAspect`, so source-pixel offsets
 * map through `containerW / canvasW` linearly. `outputAspect` defaults to the v1
 * "source matches input" model for callers that don't pass it.
 */
export function videoRectPx(
	containerW: number,
	containerH: number,
	metadata: Pick<VideoMetadata, "width" | "height"> | null | undefined,
	paddingPercent: number,
	outputAspect: OutputAspect = "source",
): Rect {
	if (!metadata || containerW <= 0 || containerH <= 0) {
		return { x: 0, y: 0, w: containerW, h: containerH };
	}
	const geom = computeCanvasGeometry(metadata.width, metadata.height, paddingPercent, outputAspect);
	if (geom.canvasW <= 0 || geom.canvasH <= 0) {
		return { x: 0, y: 0, w: containerW, h: containerH };
	}
	const sx = containerW / geom.canvasW;
	const sy = containerH / geom.canvasH;
	return {
		x: geom.videoX * sx,
		y: geom.videoY * sy,
		w: geom.videoW * sx,
		h: geom.videoH * sy,
	};
}

/**
 * Device-pixel rect of the full composition FRAME (source + padding) inside the
 * container. This is what frame-anchored annotations (and captions) map onto,
 * so 0..1 spans the padded output frame rather than just the video.
 */
export function compositionRectPx(
	containerW: number,
	containerH: number,
	metadata: Pick<VideoMetadata, "width" | "height"> | null | undefined,
	paddingPercent: number,
	outputAspect: OutputAspect = "source",
): Rect {
	if (!metadata || containerW <= 0 || containerH <= 0) {
		return { x: 0, y: 0, w: containerW, h: containerH };
	}
	const geom = computeCanvasGeometry(metadata.width, metadata.height, paddingPercent, outputAspect);
	if (geom.canvasW <= 0 || geom.canvasH <= 0) {
		return { x: 0, y: 0, w: containerW, h: containerH };
	}
	const sx = containerW / geom.canvasW;
	const sy = containerH / geom.canvasH;
	return {
		x: geom.compX * sx,
		y: geom.compY * sy,
		w: geom.compW * sx,
		h: geom.compH * sy,
	};
}

/** Convenience: evaluate zoom and project a UV point in one call. */
export function projectUv(
	ux: number,
	uy: number,
	t: number,
	rect: Rect,
	zoomRegions: ZoomRegionLike[],
): { x: number; y: number } {
	return uvToCanvas(ux, uy, rect, evalZoom(zoomRegions, t));
}
