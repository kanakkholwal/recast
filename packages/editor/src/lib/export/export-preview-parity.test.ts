/**
 * Preview↔export golden-frame parity, GL-free. Both drivers feed the SAME
 * `computeFrameParams` → `RenderCore` → `WebGL2Backend`; they differ ONLY in
 * render-buffer resolution (preview is DPR/viewport-capped, export is native
 * full-res). So parity = "the export driver composites the identical picture the
 * preview would, at any resolution": resolution-invariant uniforms are equal, and
 * pixel-scaled uniforms scale by exactly the resolution ratio. A real pixel diff
 * needs a headless-GL/browser harness (not the vitest jsdom runner) — but since
 * the compositor is one shared implementation, the only place the two paths can
 * diverge is this driver-level frame mapping, which is what this locks down.
 */

import { describe, expect, it, vi } from "vitest";
import { EASE_IN_OUT } from "../easing/cubic-bezier";
import { buildTimeMap, outputToOriginal } from "../timeline/time-map";
import type { CursorSettings, ShadowSettings, ZoomRegion } from "../../stores/editor-store.svelte";
import { computeFrameParams, type FrameUniforms } from "../../components/frame-params";
import type { CursorSampleJS } from "../../components/video-preview.logic";

// computeCanvasGeometry does a value import of the runes store (`aspectRatio`),
// pulling a $constants chain the Node runner can't resolve. Mock it to a padded
// layout — parity/covariance holds for any geometry; the math is covered by the
// canvas-geometry + export-scene suites.
vi.mock("../canvas-geometry", () => ({
	computeCanvasGeometry: () => ({
		canvasW: 2100,
		canvasH: 1180,
		videoX: 90,
		videoY: 50,
		videoW: 1920,
		videoH: 1080,
		paddingPx: 90,
		compX: 0,
		compY: 0,
		compW: 2100,
		compH: 1180,
	}),
}));

import { buildExportBase, type ExportSceneInputs } from "./export-scene";
import { makeExportFrameAt } from "./export-frame-input";

// A cut (4–6s removed) followed by a 1.5× speed span — so the output clock and the
// original clock differ, exercising the export driver's time mapping.
const TIME_MAP = buildTimeMap([
	{ origStart: 0, origEnd: 4, speed: 1 },
	{ origStart: 6, origEnd: 12, speed: 1.5 },
]);

const ZOOM: ZoomRegion = {
	start: 1,
	end: 9,
	scale: 2,
	centerX: 0.4,
	centerY: 0.6,
	rampIn: 0.5,
	rampOut: 0.5,
	easeIn: EASE_IN_OUT,
	easeOut: EASE_IN_OUT,
	motionBlur: 0.4,
	hidden: false,
} as unknown as ZoomRegion;

const CURSOR: CursorSettings = {
	enabled: true,
	style: "dot",
	size: 2,
	hideWhenIdle: false,
	idleTimeout: 2,
	highlightClicks: false,
	highlightOpacity: 50,
	highlightColor: "#3b82f6",
} as unknown as CursorSettings;

const SHADOW: ShadowSettings = {
	enabled: true,
	opacity: 40,
	blur: 24,
	spread: 6,
	offsetY: 10,
	color: "#000000",
} as unknown as ShadowSettings;

const CURSOR_SAMPLES: CursorSampleJS[] = [
	{ timestampUs: 0, x: 200, y: 200, visible: true, leftDown: false, rightDown: false },
	{ timestampUs: 12_000_000, x: 1700, y: 900, visible: true, leftDown: false, rightDown: false },
];

function scene(): ExportSceneInputs {
	return {
		meta: { width: 1920, height: 1080 },
		padding: 8,
		outputAspect: "16:9",
		segments: [],
		segmentAnims: [],
		backgroundType: "gradient",
		backgroundValue: "linear-gradient(90deg,#f00,#00f)",
		backgroundBlur: 0,
		backgroundImageReady: false,
		gradient: { colors: [1, 0, 0, 1, 0, 0, 1, 1], positions: [0, 1], count: 2, angleRad: 1.57 },
		borderRadius: 6,
		focusEnabled: true,
		zoomRegions: [ZOOM],
		shadow: SHADOW,
		cursor: CURSOR,
		cursorMotionEasing: null,
		cursorSamples: CURSOR_SAMPLES,
		idlePeriods: [],
		pressEvents: [],
	};
}

// Uniforms specified in absolute pixels but INDEPENDENT of render-buffer size —
// they must be identical at every resolution (a real preview↔export caveat: bg
// blur + motion blur cover a different FRACTION of the frame at different res).
const RESOLUTION_INDEPENDENT_PX = ["bgBlurPx", "motionBlurPx"] as const;

// Scalars that scale with the render buffer's X axis (sx = canvasPxW/geom.canvasW).
const COVARIANT_X = ["borderRadiusPx", "cursorRadius", "shadowBlurPx", "shadowSpreadPx"] as const;

function expectClose(a: number, b: number, label: string) {
	expect(a, label).toBeCloseTo(b, 3);
}

/** Assert the preview-resolution frame is the export frame at a different size:
 *  invariants equal, X/Y-covariant uniforms scaled by the render-buffer ratio. */
function assertParity(exp: FrameUniforms, prev: FrameUniforms, rx: number, ry: number) {
	// Invariant — exactly equal regardless of resolution.
	expectClose(prev.videoOpacity, exp.videoOpacity, "videoOpacity");
	expectClose(prev.videoRotation, exp.videoRotation, "videoRotation");
	expect(prev.bgType).toBe(exp.bgType);
	expect(prev.bgColor).toEqual(exp.bgColor);
	expect(prev.gradCount).toBe(exp.gradCount);
	expectClose(prev.gradAngle, exp.gradAngle, "gradAngle");
	expectClose(prev.zoomScale, exp.zoomScale, "zoomScale");
	expectClose(prev.zoomCenter[0], exp.zoomCenter[0], "zoomCenter.x");
	expectClose(prev.zoomCenter[1], exp.zoomCenter[1], "zoomCenter.y");
	expectClose(prev.cursorPos[0], exp.cursorPos[0], "cursorPos.x");
	expectClose(prev.cursorPos[1], exp.cursorPos[1], "cursorPos.y");
	expectClose(prev.cursorVisible, exp.cursorVisible, "cursorVisible");
	expectClose(prev.highlightAlpha, exp.highlightAlpha, "highlightAlpha");
	expect(prev.shadowEnabled).toBe(exp.shadowEnabled);
	expect(prev.shadowColor).toEqual(exp.shadowColor);
	for (const k of RESOLUTION_INDEPENDENT_PX) expectClose(prev[k], exp[k], k);

	// Covariant — scale by the render-buffer ratio.
	expectClose(prev.canvasSize[0], exp.canvasSize[0] * rx, "canvasSize.w");
	expectClose(prev.canvasSize[1], exp.canvasSize[1] * ry, "canvasSize.h");
	expectClose(prev.videoOrigin[0], exp.videoOrigin[0] * rx, "videoOrigin.x");
	expectClose(prev.videoOrigin[1], exp.videoOrigin[1] * ry, "videoOrigin.y");
	expectClose(prev.videoSize[0], exp.videoSize[0] * rx, "videoSize.w");
	expectClose(prev.videoSize[1], exp.videoSize[1] * ry, "videoSize.h");
	expectClose(prev.shadowOffsetPx[1], exp.shadowOffsetPx[1] * rx, "shadowOffsetPx.y");
	for (const k of COVARIANT_X) expectClose(prev[k], exp[k] * rx, k);
}

describe("preview↔export golden-frame parity (driver-level)", () => {
	const base = buildExportBase(scene());
	const frameAt = makeExportFrameAt(base, TIME_MAP);
	// Preview renders the same composition at a smaller (DPR-capped) buffer.
	const prevPxW = Math.round(base.canvasPxW * 0.5);
	const prevPxH = Math.round(base.canvasPxH * 0.5);
	const rx = prevPxW / base.canvasPxW;
	const ry = prevPxH / base.canvasPxH;

	// Sample across the output timeline incl. inside the zoom ramp tail — the exact
	// region whose FP drift caused the export crop-OOB segfault (see graph.rs).
	const outputTimes = [0, 0.5, 1.5, 3.2, TIME_MAP.outputDuration - 0.2];

	for (const tOut of outputTimes) {
		it(`same picture at export vs preview resolution @ output t=${tOut.toFixed(2)}s`, () => {
			const { input, originalSec } = frameAt(0, tOut);

			// The export driver samples the frame at the cut/speed-mapped original time.
			expect(input.playbackTime).toBeCloseTo(outputToOriginal(TIME_MAP, tOut), 6);
			expect(input.playbackTime).toBeCloseTo(originalSec, 6);

			const exp = computeFrameParams(input).uniforms;
			const prev = computeFrameParams({
				...input,
				canvasPxW: prevPxW,
				canvasPxH: prevPxH,
			}).uniforms;

			assertParity(exp, prev, rx, ry);
		});
	}

	it("export renders at the composition's native full resolution (no DPR cap)", () => {
		expect(base.canvasPxW).toBe(Math.round(base.geom.canvasW));
		expect(base.canvasPxH).toBe(Math.round(base.geom.canvasH));
	});
});
