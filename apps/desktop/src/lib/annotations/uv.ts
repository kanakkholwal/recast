// Pure UV ↔ canvas geometry shared by the 2D annotation overlay and the HTML
// text layer. Both MUST use the same math or they drift when zoom/padding change.

import { computeCanvasGeometry } from "$lib/canvas-geometry";
import {
	framePaddingPixels,
	type AnnotationKind,
	type OutputAspect,
	type VideoMetadata,
} from "$lib/stores/editor-store.svelte";
import { evalZoom, type ZoomRegionLike, type ZoomTransform } from "./eval";

export interface Rect {
	x: number;
	y: number;
	w: number;
	h: number;
}

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
	const geom = computeCanvasGeometry(
		metadata.width,
		metadata.height,
		paddingPercent,
		outputAspect,
	);
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
	const geom = computeCanvasGeometry(
		metadata.width,
		metadata.height,
		paddingPercent,
		outputAspect,
	);
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

/** Annotation UV → container px, applying the shader's zoom transform. */
export function uvToCanvas(
	ux: number,
	uy: number,
	rect: Rect,
	zoom: ZoomTransform,
): { x: number; y: number } {
	const preX = (ux - zoom.cx) * zoom.scale + zoom.cx;
	const preY = (uy - zoom.cy) * zoom.scale + zoom.cy;
	return {
		x: rect.x + preX * rect.w,
		y: rect.y + preY * rect.h,
	};
}

/** Container px → annotation UV (inverse of uvToCanvas). */
export function canvasToUV(
	cx: number,
	cy: number,
	rect: Rect,
	zoom: ZoomTransform,
): { x: number; y: number } {
	if (rect.w <= 0 || rect.h <= 0) return { x: 0, y: 0 };
	const preX = (cx - rect.x) / rect.w;
	const preY = (cy - rect.y) / rect.h;
	return {
		x: (preX - zoom.cx) / zoom.scale + zoom.cx,
		y: (preY - zoom.cy) / zoom.scale + zoom.cy,
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

/**
 * Normalise a kind's bounding box so width/height are positive. Lets the user
 * drag any of the four diagonals while we keep storage canonical.
 */
export function normaliseBox(k: AnnotationKind): Rect {
	if (k.kind === "rect" || k.kind === "ellipse" || k.kind === "image" || k.kind === "text" || k.kind === "blur") {
		const x = Math.min(k.x, k.x + k.w);
		const y = Math.min(k.y, k.y + k.h);
		return { x, y, w: Math.abs(k.w), h: Math.abs(k.h) };
	}
	if (k.kind === "arrow") {
		const x = Math.min(k.x1, k.x2);
		const y = Math.min(k.y1, k.y2);
		return { x, y, w: Math.abs(k.x2 - k.x1), h: Math.abs(k.y2 - k.y1) };
	}
	return { x: 0, y: 0, w: 0, h: 0 };
}
