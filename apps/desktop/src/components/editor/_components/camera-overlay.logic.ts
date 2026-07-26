// Pure geometry for the camera bubble overlay: where it sits on the canvas,
// its shape's border-radius, and the drag clamp. The .svelte owns the video
// element, sync effects, and pointer wiring.

import { bezierY, type Easing } from "$lib/easing/cubic-bezier";
import type { CanvasGeometry } from "$lib/canvas-geometry";
import type {
	CameraKeyframe,
	CameraOverlayShape,
	CameraPlacement,
} from "$lib/stores/editor-store.svelte";

export type { CameraKeyframe };

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

// Drop-shadow geometry as FRACTIONS of the bubble's size, so it's resolution-
// independent and the Rust export can mirror it exactly (see camera.rs
// CAMERA_SHADOW_* — these MUST stay in lockstep). Strength scales all three.
export const CAMERA_SHADOW_BLUR_FRACTION = 0.14;
export const CAMERA_SHADOW_OFFSET_FRACTION = 0.05;
export const CAMERA_SHADOW_MAX_OPACITY = 0.6;

/**
 * `box-shadow` value for the bubble, sized in `cqmin` so it tracks the bubble
 * (the overlay's outer div is a size container). `none` when strength ≤ 0.
 * Mirrored in export by `render_camera_shadow`.
 */
export function cameraShadowStyle(strength: number): string {
	const s = Math.max(0, Math.min(1, strength ?? 0));
	if (s <= 0) return "none";
	const blur = (CAMERA_SHADOW_BLUR_FRACTION * s * 100).toFixed(2);
	const offset = (CAMERA_SHADOW_OFFSET_FRACTION * s * 100).toFixed(2);
	const opacity = (CAMERA_SHADOW_MAX_OPACITY * s).toFixed(3);
	return `0 ${offset}cqmin ${blur}cqmin rgba(0,0,0,${opacity})`;
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
 * so the bubble stays fully inside the video. The bubble is square in *pixels*,
 * so its UV height is `width * (videoW/videoH)` — derived here rather than read
 * from `placement.height`, which keeps the bottom clamp right on a wide video.
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
	if (rectW <= 0 || rectH <= 0 || geom.videoH <= 0) return null;
	const videoCssW = rectW * (geom.videoW / geom.canvasW);
	const videoCssH = rectH * (geom.videoH / geom.canvasH);
	if (videoCssW <= 0 || videoCssH <= 0) return null;
	const dxUv = dClientX / videoCssW;
	const dyUv = dClientY / videoCssH;
	const heightUv = Math.min(1, placement.width * (geom.videoW / geom.videoH));
	return {
		x: Math.max(0, Math.min(1 - placement.width, dragStartUv.x + dxUv)),
		y: Math.max(0, Math.min(1 - heightUv, dragStartUv.y + dyUv)),
	};
}

// --- Resize -----------------------------------------------------------------

export type CameraResizeCorner = "tl" | "tr" | "bl" | "br";

/** Smallest / largest the bubble may be resized to (video-UV fraction). */
export const MIN_CAMERA_SIZE = 0.06;
export const MAX_CAMERA_SIZE = 0.6;

/**
 * New placement from dragging a corner handle to video-UV point (ux,uy),
 * keeping the diagonally-opposite corner fixed. The bubble is square in
 * *pixels*, so UV width `w` maps to UV height `w * aspect` (aspect = videoW/
 * videoH); `width` is clamped to [MIN,MAX_CAMERA_SIZE] and to the room before
 * the frame edge (both axes, the vertical room converted back to a width), so
 * the bubble never leaves the video and never distorts.
 */
export function resizeCameraSquare(
	base: CameraPlacement,
	corner: CameraResizeCorner,
	ux: number,
	uy: number,
	aspect: number,
): CameraPlacement {
	const baseH = Math.min(1, base.width * aspect); // true UV height (square px)
	const anchorRight = corner === "tl" || corner === "bl"; // drag left → right edge fixed
	const anchorBottom = corner === "tl" || corner === "tr"; // drag up → bottom edge fixed
	const anchorX = anchorRight ? base.x + base.width : base.x;
	const anchorY = anchorBottom ? base.y + baseH : base.y;
	const roomX = anchorRight ? anchorX : 1 - anchorX;
	const roomYAsWidth = (anchorBottom ? anchorY : 1 - anchorY) / aspect;
	const cap = Math.max(MIN_CAMERA_SIZE, Math.min(MAX_CAMERA_SIZE, roomX, roomYAsWidth));
	// Drive size off the larger drag axis, the vertical one converted to a width.
	let width = Math.max(Math.abs(ux - anchorX), Math.abs(uy - anchorY) / aspect);
	width = Math.max(MIN_CAMERA_SIZE, Math.min(cap, width));
	const height = width * aspect;
	const x = anchorRight ? anchorX - width : anchorX;
	const y = anchorBottom ? anchorY - height : anchorY;
	return { x, y, width, height };
}

// --- Per-cut keyframes ------------------------------------------------------

/** Smoothstep 0..1 — the ease that makes position changes GLIDE rather than move
 *  at constant speed across a cut. */
function smoothstep(p: number): number {
	const c = Math.max(0, Math.min(1, p));
	return c * c * (3 - 2 * c);
}

function lerpPlacement(a: CameraPlacement, b: CameraPlacement, e: number): CameraPlacement {
	return {
		x: a.x + (b.x - a.x) * e,
		y: a.y + (b.y - a.y) * e,
		width: a.width + (b.width - a.width) * e,
		height: a.height + (b.height - a.height) * e,
	};
}

/**
 * Effective BASE camera placement at original time `t`, gliding between
 * per-cut keyframes. No keyframes → the static `base`. Holds at the first/last
 * keyframe outside their range. `keyframes` MUST be sorted by `atSec`. Mirrored
 * by Rust `camera_placement_at` so preview == export.
 */
export function cameraPlacementAt(
	base: CameraPlacement,
	keyframes: CameraKeyframe[],
	t: number,
	easing?: Easing,
): CameraPlacement {
	if (keyframes.length === 0) return base;
	if (keyframes.length === 1 || t <= keyframes[0].atSec) return keyframes[0].placement;
	const last = keyframes[keyframes.length - 1];
	if (t >= last.atSec) return last.placement;
	const ease = (p: number) => (easing ? bezierY(easing, p) : smoothstep(p));
	for (let i = 0; i < keyframes.length - 1; i++) {
		const a = keyframes[i];
		const b = keyframes[i + 1];
		if (t >= a.atSec && t < b.atSec) {
			const span = Math.max(1e-6, b.atSec - a.atSec);
			return lerpPlacement(a.placement, b.placement, ease((t - a.atSec) / span));
		}
	}
	return last.placement;
}

/** Insert or replace a keyframe at `atSec` (within `epsilon`), returning a new
 *  sorted array. Used by the panel/overlay when editing in per-cut mode. */
export function upsertCameraKeyframe(
	keyframes: CameraKeyframe[],
	atSec: number,
	placement: CameraPlacement,
	epsilon = 0.05,
): CameraKeyframe[] {
	const next = keyframes.filter((k) => Math.abs(k.atSec - atSec) > epsilon);
	next.push({ atSec, placement });
	next.sort((a, b) => a.atSec - b.atSec);
	return next;
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
 * disabled, at rest (scale≈1), or zero strength. The bubble is square in
 * *pixels*, so its UV height is `width * aspect` (aspect = videoW/videoH) —
 * derived here, NOT read from `base.height`, so the drift centre and clamps are
 * right on a wide video. SHARED with the export path (Rust mirror) so
 * preview == export; `aspect` must match Rust's `videoW/videoH`.
 */
export function applyZoomFollow(
	base: CameraPlacement,
	zoom: { scale: number; cx: number; cy: number },
	opts: ZoomFollowOpts,
	aspect: number = 1,
): CameraPlacement {
	const k = Math.max(0, Math.min(1, opts.strength));
	if (!opts.enabled || k <= 0 || zoom.scale <= 1.0001) return base;
	const baseH = Math.min(1, base.width * aspect);
	const amount = (zoom.scale - 1) * k; // ramps with the zoom
	const width = Math.min(1, base.width * (1 + amount));
	const height = Math.min(1, width * aspect);
	const bcx = base.x + base.width / 2;
	const bcy = base.y + baseH / 2;
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
