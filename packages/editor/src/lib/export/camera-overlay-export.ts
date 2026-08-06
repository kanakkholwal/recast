/**
 * Pure geometry for the export camera bubble — the GL twin of the preview's
 * `CameraOverlay` (bubblePlacementStyle + object-fit:cover + shape radius) and of
 * Rust `camera.rs`'s `camera_bubble_rect`. Given an already-resolved placement
 * (base → keyframes → zoom-follow, done by the shared logic upstream), produce
 * the pixel rect, the cover-crop/mirror UV sub-rect, and the corner radius.
 */

import type { FrameGeometry } from "../../components/frame-params";
import type { QuadRect, UvRect } from "../../components/overlay-quad";
import type { CameraOverlayShape, CameraPlacement } from "../../stores/editor-store.svelte";

/** Bubble rect in render-buffer px: square, sized off video width (matching the
 *  preview's `aspect-ratio: 1`), clamped inside the canvas for legacy placements. */
export function cameraBubbleRect(
	placement: CameraPlacement,
	geom: FrameGeometry,
	canvasPxW: number,
	canvasPxH: number,
): QuadRect {
	const sx = canvasPxW / Math.max(1, geom.canvasW);
	const sy = canvasPxH / Math.max(1, geom.canvasH);
	const side = Math.max(2, clamp(placement.width, 0.02, 1) * geom.videoW); // comp px
	const maxX = Math.max(0, geom.canvasW - side);
	const maxY = Math.max(0, geom.canvasH - side);
	const x = Math.min(geom.videoX + clamp(placement.x, 0, 1) * geom.videoW, maxX);
	const y = Math.min(geom.videoY + clamp(placement.y, 0, 1) * geom.videoH, maxY);
	return { x: x * sx, y: y * sy, w: side * sx, h: side * sy };
}

/** UV sub-rect that covers a square bubble with a `camAspect`-shaped source
 *  (object-fit: cover), optionally mirrored horizontally. */
export function coverUvRect(camAspect: number, mirror: boolean): UvRect {
	let u0 = 0;
	let v0 = 0;
	let du = 1;
	let dv = 1;
	if (!(camAspect > 0)) camAspect = 1;
	if (camAspect >= 1) {
		du = 1 / camAspect;
		u0 = (1 - du) / 2;
	} else {
		dv = camAspect;
		v0 = (1 - dv) / 2;
	}
	if (mirror) {
		u0 = u0 + du;
		du = -du;
	}
	return { u0, v0, du, dv };
}

/** Rounded-corner radius in px for the bubble shape: circle → side/2 (a true
 *  circle on the square rect), rounded → `cornerRadius` fraction of the side,
 *  square/rectangle → 0. Matches `shapeBorderRadius` in camera-overlay.logic. */
export function bubbleCornerRadiusPx(
	shape: CameraOverlayShape,
	cornerRadius: number | undefined,
	sidePx: number,
): number {
	if (shape === "circle") return sidePx / 2;
	if (shape === "square" || shape === "rectangle") return 0;
	return (cornerRadius ?? 0.16) * sidePx;
}

function clamp(v: number, lo: number, hi: number): number {
	return Math.max(lo, Math.min(hi, v));
}
