// Pure geometry for the camera bubble: placement, corner radius and the drag clamp. The component owns the wiring.

import type { CanvasGeometry } from "../../lib/canvas-geometry";
import { bezierY, EASE, EASE_IN_OUT, type Easing } from "../../lib/easing/cubic-bezier";
import type {
	CameraKeyframe,
	CameraMotionSegment,
	CameraOverlaySettings,
	CameraOverlayShape,
	CameraPlacement,
	ZoomRegion,
} from "../../stores/editor-store.svelte";

export type { CameraKeyframe };

/**
 * The camera overlay a loaded project should start with.
 *
 * Recorded `motionSegments` are CARRIED, never applied: dragging the preview
 * window mid-take to see your own face is a record of the session, not an edit.
 * The panel offers them as an explicit import instead.
 */
export function cameraOverlayFromState(
	loaded: Partial<CameraOverlaySettings> | undefined,
	fallbackPlacement: CameraPlacement,
): CameraOverlaySettings {
	const defaultPlacement = clampPlacement({
		x: loaded?.defaultPlacement?.x ?? fallbackPlacement.x,
		y: loaded?.defaultPlacement?.y ?? fallbackPlacement.y,
		width: loaded?.defaultPlacement?.width ?? fallbackPlacement.width,
		height: loaded?.defaultPlacement?.height ?? fallbackPlacement.height,
	});
	return {
		enabled: loaded?.enabled ?? false,
		mirror: loaded?.mirror ?? true,
		shape: loaded?.shape ?? "rounded",
		cornerRadius: loaded?.cornerRadius ?? 0.16,
		animationPreset: loaded?.animationPreset ?? "soft",
		zoomFollow: loaded?.zoomFollow ?? true,
		zoomFollowStrength: loaded?.zoomFollowStrength ?? 0.6,
		zoomFollowDuration: loaded?.zoomFollowDuration ?? 0.4,
		zoomFollowEasing: { ...(loaded?.zoomFollowEasing ?? EASE_IN_OUT) },
		keyframes: (loaded?.keyframes ?? []).map((k) => ({
			atSec: k.atSec,
			placement: { ...k.placement },
		})),
		keyframeEasing: { ...(loaded?.keyframeEasing ?? EASE_IN_OUT) },
		clipLayouts: (loaded?.clipLayouts ?? []).map((c) => ({
			start: c.start,
			layout: { ...c.layout },
		})),
		shadow: loaded?.shadow ?? 0.35,
		defaultPlacement,
		motionSegments: (loaded?.motionSegments ?? []).map((segment) => ({
			...segment,
			easeIn: segment.easeIn ?? { ...EASE },
			easeOut: segment.easeOut ?? { ...EASE },
		})),
	};
}

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

// Fractions of the bubble's size, so it is resolution-independent and camera.rs's CAMERA_SHADOW_* can mirror it exactly.
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

/**
 * Keep a placement fully inside the frame. The drag path already clamps, but a
 * recorded one does not: a live capture can write `x: 1`, which puts the whole
 * bubble past the right edge until the glide happens to pull it back.
 */
export function clampPlacement(p: CameraPlacement): CameraPlacement {
	const width = Math.max(0, Math.min(1, p.width));
	const height = Math.max(0, Math.min(1, p.height));
	return {
		width,
		height,
		x: Math.max(0, Math.min(1 - width, p.x)),
		y: Math.max(0, Math.min(1 - height, p.y)),
	};
}

/**
 * The bubble's drawn placement expressed as a delta on its LAYOUT box, as
 * `translate(%)` + `scale`. Percentages are relative to the layout box's own
 * width (and its square height in UV, which is `width * videoAspect`), because
 * that is what a CSS percentage translate resolves against.
 *
 * Keeping the layout box out of the playhead is the point: it is written by
 * Svelte reactivity and the transform is written per rAF, so a layout box that
 * moved with the clock made the two race and the bubble judder.
 */
export function cameraBubbleDelta(
	layout: CameraPlacement,
	drawn: CameraPlacement,
	videoAspect: number,
): { tx: number; ty: number; scale: number } {
	if (!(layout.width > 0)) return { tx: 0, ty: 0, scale: 1 };
	const baseH = Math.min(1, layout.width * videoAspect);
	return {
		tx: ((drawn.x - layout.x) / layout.width) * 100,
		ty: baseH > 0 ? ((drawn.y - layout.y) / baseH) * 100 : 0,
		scale: drawn.width / layout.width,
	};
}

/**
 * Camera moves made DURING a recording, as keyframes.
 *
 * The recorder writes `motionSegments`; the preview and the export both read
 * `keyframes`. Folding one into the other on load is what makes a live move
 * render at all — a second evaluator would be a parity liability, and the
 * recorded moves become editable once they are keyframes.
 */
/**
 * A recorded segment longer than this cannot be a drag. Before the recorder had
 * a movement dead zone, sub-pixel jitter in the preview geometry made every tick
 * read as a move and the coalescing rule extended one segment for the whole
 * take, so the file records a minutes-long glide that never happened.
 *
 * Kept in step with `MAX_MOTION_SEGMENT_SECS` in
 * `apps/desktop/src-tauri/src/recording/mod.rs`, which stops new recordings
 * producing one.
 */
export const MAX_RECORDED_MOVE_SECS = 10;

/**
 * Trims a segment that spans more than a drag can. Only the endpoints were ever
 * sampled, so the honest repair is to HOLD the start placement and move over
 * the last {@link MAX_RECORDED_MOVE_SECS}: the bubble ends up where the file
 * says it ended, without drifting across the whole video on the way.
 */
function repairLongMove(segment: CameraMotionSegment): CameraMotionSegment {
	if (segment.end - segment.start <= MAX_RECORDED_MOVE_SECS) return segment;
	return { ...segment, start: segment.end - MAX_RECORDED_MOVE_SECS };
}

export function keyframesFromMotionSegments(
	segments: readonly CameraMotionSegment[],
	defaultPlacement: CameraPlacement,
): CameraKeyframe[] {
	if (segments.length === 0) return [];
	const sorted = [...segments].map(repairLongMove).sort((a, b) => a.start - b.start);
	const out: CameraKeyframe[] = [];
	const push = (atSec: number, placement: CameraPlacement) => {
		const last = out[out.length - 1];
		// Two segments meeting at one instant: the later wins, matching the walk that adopts each segment's `to`.
		if (last && Math.abs(last.atSec - atSec) < 1e-6) out[out.length - 1] = { atSec, placement };
		else out.push({ atSec, placement });
	};
	for (const s of sorted) {
		push(
			s.start,
			clampPlacement({ x: s.fromX, y: s.fromY, width: s.fromWidth, height: s.fromHeight }),
		);
		push(s.end, clampPlacement({ x: s.toX, y: s.toY, width: s.toWidth, height: s.toHeight }));
	}
	const head = out[0];
	// Only pin the head when it differs from `defaultPlacement`; holding the first keyframe covers the rest.
	if (head.atSec > 0 && !samePlacement(head.placement, defaultPlacement)) {
		out.unshift({ atSec: 0, placement: clampPlacement(defaultPlacement) });
	}
	return out;
}

function samePlacement(a: CameraPlacement, b: CameraPlacement): boolean {
	return (
		Math.abs(a.x - b.x) < 1e-6 &&
		Math.abs(a.y - b.y) < 1e-6 &&
		Math.abs(a.width - b.width) < 1e-6 &&
		Math.abs(a.height - b.height) < 1e-6
	);
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
/**
 * Camera-grow "scale" at time `t`, gated to the zoom's ACTIVE window but ramped
 * on the camera's OWN `durationS` + `easing` (not the zoom region's ramp), so the
 * grow/shrink is a smooth, separately-tunable transition. Returns the zoom's
 * focus centre so the bubble still drifts away from it. `{scale:1}` when no zoom
 * is active. Mirror of the export's `camera_follow_scale_at` (keep in lockstep).
 */
export function cameraFollowScaleAt(
	regions: ZoomRegion[],
	t: number,
	durationS: number,
	easing: Easing,
): { scale: number; cx: number; cy: number } {
	for (const r of regions) {
		if (r.hidden || t <= r.start || t >= r.end) continue;
		const d = Math.max(1e-3, durationS);
		const inA = bezierY(easing, Math.max(0, Math.min(1, (t - r.start) / d)));
		const outA = bezierY(easing, Math.max(0, Math.min(1, (r.end - t) / d)));
		const a = Math.min(inA, outA);
		const peak = Math.max(1, r.scale);
		return { scale: 1 + a * (peak - 1), cx: r.centerX ?? 0.5, cy: r.centerY ?? 0.5 };
	}
	return { scale: 1, cx: 0.5, cy: 0.5 };
}

export function applyZoomFollow(
	base: CameraPlacement,
	zoom: { scale: number; cx: number; cy: number },
	opts: ZoomFollowOpts,
	aspect = 1,
): CameraPlacement {
	const k = Math.max(0, Math.min(1, opts.strength));
	if (!opts.enabled || k <= 0 || zoom.scale <= 1.0001) return base;
	const baseH = Math.min(1, base.width * aspect);
	const amount = (zoom.scale - 1) * k; // ramps with the zoom
	const width = Math.min(1, base.width * (1 + amount));
	const height = Math.min(1, width * aspect);
	const bcx = base.x + base.width / 2;
	const bcy = base.y + baseH / 2;
	// Away-from-focus is SCREEN-SPACE but bcx-cx/bcy-cy are UV (one UV-x unit=videoW px, one UV-y unit=videoH px); normalise in pixels (videoH unit) then back to UV per axis, else the angle is wrong on a wide frame. Mirror of Rust `follow_placement` (D-2); keep in lockstep.
	const drift = amount * DRIFT_MAX;
	const px = (bcx - zoom.cx) * aspect;
	const py = bcy - zoom.cy;
	const len = Math.hypot(px, py);
	const dx = len > 1e-4 ? ((px / len) * drift) / aspect : 0;
	const dy = len > 1e-4 ? (py / len) * drift : 0;
	return {
		x: Math.max(0, Math.min(1 - width, bcx + dx - width / 2)),
		y: Math.max(0, Math.min(1 - height, bcy + dy - height / 2)),
		width,
		height,
	};
}
