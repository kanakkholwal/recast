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
 * omitted, so `aspect-ratio: 1` keeps the bubble square regardless of video
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

// --- Resize -----------------------------------------------------------------

export type CameraResizeCorner = "tl" | "tr" | "bl" | "br";

/** Smallest / largest the bubble may be resized to (video-UV fraction). */
export const MIN_CAMERA_SIZE = 0.06;
export const MAX_CAMERA_SIZE = 0.6;

/**
 * New square placement from dragging a corner handle to video-UV point (ux,uy),
 * keeping the diagonally-opposite corner fixed. Size is clamped to
 * [MIN,MAX_CAMERA_SIZE] and to the room available before the frame edge, so the
 * bubble never leaves the video.
 */
export function resizeCameraSquare(
	base: CameraPlacement,
	corner: CameraResizeCorner,
	ux: number,
	uy: number,
): CameraPlacement {
	const anchorRight = corner === "tl" || corner === "bl"; // drag left → right edge fixed
	const anchorBottom = corner === "tl" || corner === "tr"; // drag up → bottom edge fixed
	const anchorX = anchorRight ? base.x + base.width : base.x;
	const anchorY = anchorBottom ? base.y + base.height : base.y;
	const roomX = anchorRight ? anchorX : 1 - anchorX;
	const roomY = anchorBottom ? anchorY : 1 - anchorY;
	const cap = Math.max(MIN_CAMERA_SIZE, Math.min(MAX_CAMERA_SIZE, roomX, roomY));
	let size = Math.max(Math.abs(ux - anchorX), Math.abs(uy - anchorY));
	size = Math.max(MIN_CAMERA_SIZE, Math.min(cap, size));
	const x = anchorRight ? anchorX - size : anchorX;
	const y = anchorBottom ? anchorY - size : anchorY;
	return { x, y, width: size, height: size };
}

// --- Zoom-follow ------------------------------------------------------------

export interface ZoomFollowOpts {
	enabled: boolean;
	strength: number;
}

/** Max video-UV drift per unit of `(scale-1)*strength`. Tuned so a 1.8× zoom at
 *  full strength nudges the bubble ~0.14 UV toward its far corner. */
const DRIFT_MAX = 0.18;

/**
 * Effective camera placement under the zoom-follow effect: as a zoom of `scale`
 * centred at (cx,cy) ramps in, the bubble GROWS and DRIFTS away from the focus
 * so the enlarged camera never covers the zoomed content. Identity when
 * disabled, at rest (scale≈1), or zero strength. Square-preserving, clamped
 * on-screen. SHARED with the export path (Rust mirror) so preview == export.
 */
export function applyZoomFollow(
	base: CameraPlacement,
	zoom: { scale: number; cx: number; cy: number },
	opts: ZoomFollowOpts,
): CameraPlacement {
	const k = Math.max(0, Math.min(1, opts.strength));
	if (!opts.enabled || k <= 0 || zoom.scale <= 1.0001) return base;
	const amount = (zoom.scale - 1) * k; // ramps with the zoom
	const width = Math.min(1, base.width * (1 + amount));
	const height = Math.min(1, base.height * (1 + amount));
	const bcx = base.x + base.width / 2;
	const bcy = base.y + base.height / 2;
	let dx = bcx - zoom.cx;
	let dy = bcy - zoom.cy;
	const len = Math.hypot(dx, dy);
	const drift = amount * DRIFT_MAX;
	if (len > 1e-4) {
		dx = (dx / len) * drift;
		dy = (dy / len) * drift;
	} else {
		dx = 0;
		dy = 0;
	}
	return {
		x: Math.max(0, Math.min(1 - width, bcx + dx - width / 2)),
		y: Math.max(0, Math.min(1 - height, bcy + dy - height / 2)),
		width,
		height,
	};
}
