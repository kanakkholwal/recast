// Pure UV ↔ canvas geometry helpers shared between the 2D annotation overlay
// and the HTML text-annotation layer. Both layers must agree on the same math
// to avoid drift when zoom/padding change — that's the only correctness
// invariant that matters here.

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
 * Device-pixel rect of the actual video region inside a canvas/element of
 * dimensions `containerW × containerH`. The container's aspect tracks the
 * configured `outputAspect` (the editor preview keeps the canvas matched
 * to the canvas geometry), so this maps source-pixel offsets through
 * `containerW / canvasW` linearly.
 *
 * `outputAspect` is optional so existing callers that only know the v1
 * "source matches input" model keep working — they pass nothing and get
 * the old uniform-padding behaviour.
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
