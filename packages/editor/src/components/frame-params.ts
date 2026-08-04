/**
 * Pure compositor frame params: the single `(scene, geom, time) → uniforms`
 * evaluator behind the preview shader. `draw()` interleaves this math with
 * `gl.uniform*` calls today; extracting it lets ONE definition drive preview,
 * the offline export renderer, and the future WebGPU uniform-buffer path.
 *
 * Every value here mirrors the WebGL fragment shader (video-preview.shaders.ts)
 * and its Rust export twin 1:1 — this is a transcription of `draw()`, not new
 * behaviour. All inputs are plain data or already-pure evaluators; no GL, no DOM,
 * no store access, so it unit-tests in a plain Node run.
 */

import { evalSceneAt } from "$lib/scenes/eval";
import { hexToRgba } from "./color.logic";
import {
	evaluateZoomAt,
	idleAlphaAt,
	interpolateCursor,
	type CursorSampleJS,
	type IdlePeriodJS,
	type ZoomState,
} from "./video-preview.logic";
import {
	clickAnchorAt,
	clickHighlightAt,
	pressStateAt,
	type PressEvent,
} from "./cursor-animation.logic";
import type { Segment } from "$lib/timeline/segments";
import type { SegmentAnim } from "$lib/scenes/segment-anim";
import type { Easing } from "$lib/easing/cubic-bezier";
import type {
	CursorSettings,
	ShadowSettings,
	StoredCursorId,
	ZoomRegion,
} from "$lib/stores/editor-store.svelte";

/** Source-video rectangle inside the canvas, in canvas-geometry pixels (the
 *  `computeCanvasGeometry` output, before the render-buffer scale). */
export interface FrameGeometry {
	canvasW: number;
	canvasH: number;
	videoX: number;
	videoY: number;
	videoW: number;
	videoH: number;
}

/** Pre-packed gradient uniform arrays (from `buildGradientUniforms`). Required
 *  only when `backgroundType === "gradient"`; the caller memoises it. */
export interface GradientUniformInput {
	colors: Float32Array | number[];
	positions: Float32Array | number[];
	count: number;
	angleRad: number;
}

/** The visual scene at a moment in time — the seed of the shared `SceneState`
 *  contract. Everything the compositor needs to produce one frame's uniforms. */
export interface FrameInput {
	meta: { width: number; height: number };
	geom: FrameGeometry;
	/** Render-buffer size in device pixels (`canvas.width`/`height`), which the
	 *  DPR/max-dim cap can shrink below `geom.canvasW`/`canvasH`. */
	canvasPxW: number;
	canvasPxH: number;
	playbackTime: number;

	segments: ReadonlyArray<Segment>;
	segmentAnims: ReadonlyArray<SegmentAnim>;

	backgroundType: string;
	backgroundValue: string;
	backgroundBlur: number;
	/** True when the background image texture is uploaded and ready to sample. */
	backgroundImageReady: boolean;
	gradient?: GradientUniformInput;

	borderRadius: number;

	focusEnabled: boolean;
	zoomRegions: ZoomRegion[];

	shadow: ShadowSettings;

	cursor: CursorSettings;
	cursorMotionEasing: Easing | null;
	cursorSamples: CursorSampleJS[];
	idlePeriods: IdlePeriodJS[];
	pressEvents: PressEvent[];
}

/** Flat uniform set matching video-preview.shaders.ts, ready for `gl.uniform*`
 *  (WebGL2) or a uniform buffer (WebGPU). */
export interface FrameUniforms {
	canvasSize: [number, number];
	videoOrigin: [number, number];
	videoSize: [number, number];
	videoOpacity: number;
	videoRotation: number;
	bgType: number;
	bgColor: [number, number, number, number];
	gradColors: Float32Array | number[];
	gradStops: Float32Array | number[];
	gradCount: number;
	gradAngle: number;
	bgBlurPx: number;
	zoomCenter: [number, number];
	zoomScale: number;
	motionBlurPx: number;
	borderRadiusPx: number;
	cursorPos: [number, number];
	cursorVisible: number;
	cursorRadius: number;
	cursorColor: [number, number, number, number];
	highlightColor: [number, number, number, number];
	highlightAlpha: number;
	highlightPos: [number, number];
	shadowEnabled: number;
	shadowBlurPx: number;
	shadowSpreadPx: number;
	shadowOffsetPx: [number, number];
	shadowColor: [number, number, number, number];
}

/** Placement + state for the HTML SVG-cursor overlay (non-`dot` styles). Null
 *  when the shader's dot cursor is in use. */
export interface SvgCursorParams {
	visible: boolean;
	alpha: number;
	styleId: StoredCursorId;
	pressed: boolean;
	right: boolean;
	dragging: boolean;
	scale: number;
	canvasX: number;
	canvasY: number;
	compW: number;
	compH: number;
	spritePx: number;
}

export interface FrameParams {
	uniforms: FrameUniforms;
	svgCursor: SvgCursorParams | null;
	/** Caller must bind the background-image texture before drawing (image mode). */
	bindBackgroundImage: boolean;
}

const ZERO_GRAD_COLORS = new Float32Array(32);
const ZERO_GRAD_STOPS = new Float32Array(8);

/** Compute every per-frame uniform + overlay param from the scene at `playbackTime`.
 *  Pure transcription of `VideoPreview.svelte`'s `draw()`; see that file for the
 *  rationale behind each branch. */
export function computeFrameParams(input: FrameInput): FrameParams {
	const { meta, geom, canvasPxW, canvasPxH, playbackTime } = input;
	const sx = canvasPxW / Math.max(1, geom.canvasW);
	const sy = canvasPxH / Math.max(1, geom.canvasH);

	// Scene entrance/exit transform on the video layer (background stays put).
	const scene = evalSceneAt(input.segments, input.segmentAnims, playbackTime);
	let videoX = geom.videoX * sx;
	let videoY = geom.videoY * sy;
	let videoW = geom.videoW * sx;
	let videoH = geom.videoH * sy;
	if (scene.scale !== 1) {
		const cx = videoX + videoW * 0.5;
		const cy = videoY + videoH * 0.5;
		videoW *= scene.scale;
		videoH *= scene.scale;
		videoX = cx - videoW * 0.5;
		videoY = cy - videoH * 0.5;
	}
	videoX += scene.translateX * canvasPxW;
	videoY += scene.translateY * canvasPxH;

	// Background.
	let bgType = 0;
	let bgColor: [number, number, number, number] = [0, 0, 0, 1];
	let gradColors: Float32Array | number[] = ZERO_GRAD_COLORS;
	let gradStops: Float32Array | number[] = ZERO_GRAD_STOPS;
	let gradCount = 0;
	let gradAngle = 0;
	let bgBlurPx = 0;
	let bindBackgroundImage = false;
	if (input.backgroundType === "color") {
		bgType = 0;
		bgColor = hexToRgba(input.backgroundValue || "#111111");
	} else if (input.backgroundType === "gradient") {
		bgType = 1;
		const grad = input.gradient;
		if (grad) {
			gradColors = grad.colors;
			gradStops = grad.positions;
			gradCount = grad.count;
			gradAngle = grad.angleRad;
		}
	} else if (input.backgroundImageReady) {
		bgType = 2;
		// 0..100 slider → pixel radius; 100 ≈ 24px.
		bgBlurPx = Math.max(0, input.backgroundBlur * 0.24);
		bindBackgroundImage = true;
	} else {
		// Image not yet loaded: dark fallback colour.
		bgType = 0;
		bgColor = [0.067, 0.067, 0.067, 1];
	}

	// Border radius: percent of the shorter source edge → canvas pixels via sx.
	const shorterEdge = Math.min(meta.width, meta.height);
	const radiusPx = ((input.borderRadius ?? 0) / 100) * shorterEdge * sx;
	const borderRadiusPx = Math.max(0, radiusPx);

	// Zoom + motion blur.
	const zoom: ZoomState = input.focusEnabled
		? evaluateZoomAt(input.zoomRegions, playbackTime)
		: { scale: 1.0, cx: 0.5, cy: 0.5, motionBlur: 0 };
	let motionBlurPx = 0;
	if (zoom.motionBlur > 0.001 && zoom.scale > 1.0001) {
		const dt = 1 / 60;
		const next = evaluateZoomAt(input.zoomRegions, playbackTime + dt);
		const dScaleDt = Math.abs(next.scale - zoom.scale) / dt;
		// `strength * dScaleDt * 45` overshoots the physically-correct per-frame smear
		// (≈ dScaleDt * videoW / 2fps) for a punchier, more legible dolly blur; the
		// old 20px clamp made it invisible (shown ~half-size), so allow up to 100px.
		motionBlurPx = Math.min(100, zoom.motionBlur * dScaleDt * 45);
	}

	// Cursor + click highlight.
	const cs = input.cursor;
	let cursorAlpha = 0;
	let highlightAlpha = 0;
	let highlightPosX = 0;
	let highlightPosY = 0;
	let cursorPosX = 0;
	let cursorPosY = 0;
	let cursorPressed = false;
	let cursorRight = false;
	let cursorDragging = false;
	let cursorScale = 1;
	if (cs.enabled && input.cursorSamples.length > 0) {
		const ts = Math.max(0, playbackTime) * 1_000_000;
		const idleA = cs.hideWhenIdle ? idleAlphaAt(input.idlePeriods, ts, cs.idleTimeout) : 1;
		const press = pressStateAt(input.pressEvents, ts);
		const baseAlpha = Math.max(idleA, press.visibleAlpha);
		if (baseAlpha > 0) {
			const pos = interpolateCursor(input.cursorSamples, input.cursorMotionEasing, ts);
			if (pos && pos.visible) {
				cursorAlpha = baseAlpha;
				let posX = pos.x;
				let posY = pos.y;
				const anchor = clickAnchorAt(input.pressEvents, ts);
				if (anchor) {
					posX = posX * (1 - anchor.weight) + anchor.x * anchor.weight;
					posY = posY * (1 - anchor.weight) + anchor.y * anchor.weight;
				}
				cursorPosX = posX / meta.width;
				cursorPosY = posY / meta.height;
				cursorPressed = press.pressedSprite;
				cursorRight = press.right;
				cursorDragging = press.dragging;
				cursorScale = press.scale;
			}
		}
		if (cs.highlightClicks) {
			const hl = clickHighlightAt(input.pressEvents, ts);
			if (hl) {
				highlightAlpha = (cs.highlightOpacity / 100) * hl.alpha;
				let hlUvX = hl.x / meta.width;
				let hlUvY = hl.y / meta.height;
				if (zoom.scale > 1.0001) {
					hlUvX = (hlUvX - zoom.cx) * zoom.scale + zoom.cx;
					hlUvY = (hlUvY - zoom.cy) * zoom.scale + zoom.cy;
				}
				highlightPosX = hlUvX;
				highlightPosY = hlUvY;
			}
		}
	}
	const usingSvgCursor = cs.enabled && cs.style !== "dot";
	const overlayVisible = usingSvgCursor && cursorAlpha > 0;
	// SVG overlay mirrors the shader's cursor-zoom affine so the sprite tracks
	// the dot pixel-for-pixel.
	let svgUvX = cursorPosX;
	let svgUvY = cursorPosY;
	if (zoom.scale > 1.0001) {
		svgUvX = (cursorPosX - zoom.cx) * zoom.scale + zoom.cx;
		svgUvY = (cursorPosY - zoom.cy) * zoom.scale + zoom.cy;
	}
	const svgCursor: SvgCursorParams | null = usingSvgCursor
		? {
				visible: overlayVisible,
				alpha: cursorAlpha,
				styleId: cs.style,
				pressed: cursorPressed,
				right: cursorRight,
				dragging: cursorDragging,
				scale: cursorScale,
				canvasX: geom.videoX + svgUvX * geom.videoW,
				canvasY: geom.videoY + svgUvY * geom.videoH,
				compW: geom.canvasW,
				compH: geom.canvasH,
				spritePx: cs.size * 16,
			}
		: null;
	const cursorRadius = Math.max(2, cs.size * 2 * sx * cursorScale);
	const [hr, hg, hb] = hexToRgba(cs.highlightColor || "#3b82f6");

	// Drop shadow (video-pixel units → canvas pixels via sx, matching padding).
	const shadow = input.shadow;
	let shadowEnabled = 0;
	let shadowBlurPx = 0;
	let shadowSpreadPx = 0;
	let shadowOffsetPx: [number, number] = [0, 0];
	let shadowColor: [number, number, number, number] = [0, 0, 0, 0];
	if (shadow.enabled && shadow.opacity > 0) {
		shadowEnabled = 1;
		shadowBlurPx = Math.max(0.5, shadow.blur * sx);
		shadowSpreadPx = Math.max(0, shadow.spread * sx);
		shadowOffsetPx = [0, shadow.offsetY * sx];
		const [sr, sg, sb] = hexToRgba(shadow.color || "#000000");
		shadowColor = [sr, sg, sb, shadow.opacity / 100];
	}

	return {
		uniforms: {
			canvasSize: [canvasPxW, canvasPxH],
			videoOrigin: [videoX, videoY],
			videoSize: [videoW, videoH],
			videoOpacity: scene.opacity,
			videoRotation: (scene.rotate * Math.PI) / 180,
			bgType,
			bgColor,
			gradColors,
			gradStops,
			gradCount,
			gradAngle,
			bgBlurPx,
			zoomCenter: [zoom.cx, zoom.cy],
			zoomScale: zoom.scale,
			motionBlurPx,
			borderRadiusPx,
			cursorPos: [cursorPosX, cursorPosY],
			cursorVisible: usingSvgCursor ? 0 : cursorAlpha,
			cursorRadius,
			cursorColor: [1, 1, 1, 0.9],
			highlightColor: [hr, hg, hb, 1],
			highlightAlpha,
			highlightPos: [highlightPosX, highlightPosY],
			shadowEnabled,
			shadowBlurPx,
			shadowSpreadPx,
			shadowOffsetPx,
			shadowColor,
		},
		svgCursor,
		bindBackgroundImage,
	};
}
