// Pure geometry for the camera bubble overlay: where it sits on the canvas,
// its shape's border-radius, and the drag clamp. The .svelte owns the video
// element, sync effects, and pointer wiring.

import type { CanvasGeometry } from "$lib/canvas-geometry";
import type {
	CameraOverlayShape,
	CameraPlacement,
} from "$lib/stores/editor-store.svelte";

/**
 * Inline style placing the bubble as canvas percentages. Bubble UV is in VIDEO
 * space, so it's offset by the video rect inside the padded canvas. Height is
 * omitted — `aspect-ratio: 1` keeps the bubble square regardless of video
 * aspect. Returns `display:none` when geometry isn't ready.
 */
export function bubblePlacementStyle(
	geom: CanvasGeometry | null,
	placement: CameraPlacement,
): string {
	if (!geom) return "display:none;";
	const left = ((geom.videoX + placement.x * geom.videoW) / geom.canvasW) * 100;
	const top = ((geom.videoY + placement.y * geom.videoH) / geom.canvasH) * 100;
	const width = ((placement.width * geom.videoW) / geom.canvasW) * 100;
	return `left:${left}%;top:${top}%;width:${width}%;`;
}

/** CSS border-radius for a bubble shape. square/rectangle → 0; circle → 50% (true circle with the 1:1 aspect); rounded → saved corner radius. */
export function shapeBorderRadius(
	shape: CameraOverlayShape,
	cornerRadius: number | undefined,
): string {
	if (shape === "circle") return "50%";
	if (shape === "square" || shape === "rectangle") return "0";
	return `${(cornerRadius ?? 0.16) * 100}%`;
}

/**
 * New bubble UV position from a CSS-pixel drag delta, or null when the target
 * rect isn't measurable yet. Deltas are relative to the rendered VIDEO rect
 * (not the whole canvas) so padding doesn't bias motion; the result is clamped
 * so the bubble stays fully inside the video.
 */
export function clampCameraDrag(
	geom: CanvasGeometry,
	rectW: number,
	rectH: number,
	dClientX: number,
	dClientY: number,
	dragStartUv: { x: number; y: number },
	placement: { width: number; height: number },
): { x: number; y: number } | null {
	if (rectW <= 0 || rectH <= 0) return null;
	const videoCssW = rectW * (geom.videoW / geom.canvasW);
	const videoCssH = rectH * (geom.videoH / geom.canvasH);
	if (videoCssW <= 0 || videoCssH <= 0) return null;
	const dxUv = dClientX / videoCssW;
	const dyUv = dClientY / videoCssH;
	return {
		x: Math.max(0, Math.min(1 - placement.width, dragStartUv.x + dxUv)),
		y: Math.max(0, Math.min(1 - placement.height, dragStartUv.y + dyUv)),
	};
}
