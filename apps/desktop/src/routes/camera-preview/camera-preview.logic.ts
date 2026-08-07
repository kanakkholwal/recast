/** Geometry + preview-state kernels for the camera-preview window. */

import type { CameraPreviewState } from "@recast/editor/lib/wire-types";

/** Preferred → fallback MediaRecorder container/codec for the camera track.
 *  H.264/MP4 first (WebView2/Chromium supports it and the editor stream-copies
 *  it, no transcode); VP9/VP8 WebM otherwise (Rust transcodes to MP4). */
const CAMERA_MIME_CANDIDATES = [
	"video/mp4;codecs=avc1.42E01E",
	"video/mp4",
	"video/webm;codecs=vp9",
	"video/webm;codecs=vp8",
	"video/webm",
] as const;

/** First candidate the runtime supports. `mimeType: ""` lets MediaRecorder pick
 *  its own default. `supported` is injectable for tests. */
export function pickCameraMimeType(
	supported: (t: string) => boolean = (t) =>
		typeof MediaRecorder !== "undefined" && MediaRecorder.isTypeSupported(t),
): string {
	for (const candidate of CAMERA_MIME_CANDIDATES) {
		if (supported(candidate)) return candidate;
	}
	return "";
}

export type AspectKey = "1:1" | "4:3" | "16:9";
export type ShapeKey = "square" | "rounded" | "circle";
export type CameraStatus = "loading" | "live" | "warning" | "failed";

export const ASPECTS: AspectKey[] = ["1:1", "4:3", "16:9"];
export const ASPECT_RATIO: Record<AspectKey, number> = {
	"1:1": 1,
	"4:3": 4 / 3,
	"16:9": 16 / 9,
};

/** CSS px radius for the "rounded" shape, matching the rounded-3xl token. */
export const WINDOW_RADIUS = 20;

/** Max preview size as a fraction of the screen, so it never covers recorded
 *  content or balloons the composited bubble. */
export const MAX_SCREEN_FRACTION = 0.25;

/** Min video width. Window width == video width, so this floors at the width of
 *  the controls pill (widest at the "16:9" label) to avoid clipping it. */
export const CONTROL_BAR_MIN_WIDTH = 168;
export const MIN_LOGICAL_SIZE = CONTROL_BAR_MIN_WIDTH;

/** Bottom strip for the control bar, outside the rounded/clipped video bubble.
 *  The aspect lock governs only `windowHeight − CONTROL_BAR_HEIGHT`. Keep in
 *  sync with the strip height in markup and `openCameraPreviewWindow` (ipc.ts). */
export const CONTROL_BAR_HEIGHT = 40;

/**
 * Circle is 1:1-only. On a non-square aspect it'd be an ellipse, which the
 * composited bubble in the editor doesn't render.
 */
export function allowedShapesFor(a: AspectKey): ShapeKey[] {
	return a === "1:1" ? ["square", "rounded", "circle"] : ["square", "rounded"];
}

/** Largest box of the given ratio that fits inside (maxW, maxH). */
export function fitInsideMax(
	w: number,
	h: number,
	ratio: number,
	maxW: number,
	maxH: number,
): [number, number] {
	let outW = w;
	let outH = h;
	if (outW > maxW) {
		outW = maxW;
		outH = outW / ratio;
	}
	if (outH > maxH) {
		outH = maxH;
		outW = outH * ratio;
	}
	return [Math.round(outW), Math.round(outH)];
}

export type SizeConstraints = {
	maxLogicalW: number;
	maxLogicalH: number;
	minLogicalW: number;
	minWinH: number;
};

/**
 * OS min/max size constraints keyed off screen width. Every aspect is
 * landscape-or-square (ratio ≥ 1), so a square max box bounds the window by
 * width without clipping the proportional height. The min height uses the
 * widest aspect (shortest video) so the OS floor never out-clamps it.
 */
export function computeSizeConstraints(screenW: number): SizeConstraints {
	const w = Math.max(screenW, 320);
	const maxW = Math.floor(w * MAX_SCREEN_FRACTION);
	const widestRatio = Math.max(...Object.values(ASPECT_RATIO));
	const minWinH = Math.round(MIN_LOGICAL_SIZE / widestRatio) + CONTROL_BAR_HEIGHT;
	return {
		maxLogicalW: maxW,
		maxLogicalH: maxW,
		minLogicalW: MIN_LOGICAL_SIZE,
		minWinH,
	};
}

/**
 * Clamped [videoWidth, videoHeight] for a window of logical `width` at `ratio`,
 * bounded by the max box. Window width == video width (no horizontal chrome).
 */
export function targetWindowSize(
	width: number,
	ratio: number,
	maxW: number,
	maxH: number,
): [number, number] {
	return fitInsideMax(width, width / ratio, ratio, maxW, maxH);
}

function clamp01(v: number): number {
	return Math.max(0, Math.min(1, v));
}

/**
 * Normalised preview state for the compositor: corner radius as a fraction of
 * the shorter side, and window rect as screen fractions. `size`/`position` are
 * physical pixels; the bottom control strip is subtracted so the reported
 * bubble is just the video region.
 */
export function buildPreviewState(
	position: { x: number; y: number },
	size: { width: number; height: number },
	screen: { width: number; height: number },
	dpr: number,
	shape: ShapeKey,
	mirror: boolean,
	status: CameraStatus,
): CameraPreviewState {
	const factor = dpr;
	const videoHeightPhys = Math.max(1, size.height - CONTROL_BAR_HEIGHT * factor);
	const widthLogical = size.width / factor;
	const shortLogical = Math.min(widthLogical, videoHeightPhys / factor);
	const cornerRadius =
		shape === "square"
			? 0
			: shape === "circle"
				? 0.5
				: Math.min(0.5, WINDOW_RADIUS / Math.max(shortLogical, 1));

	return {
		mirror,
		shape,
		cornerRadius,
		animationPreset: status === "warning" ? "lively" : "soft",
		// Window top == video top (strip is at the bottom), so X/Y are unchanged.
		windowX: clamp01(position.x / screen.width),
		windowY: clamp01(position.y / screen.height),
		windowWidth: Math.max(0.05, Math.min(1, size.width / screen.width)),
		windowHeight: Math.max(0.05, Math.min(1, videoHeightPhys / screen.height)),
	};
}
