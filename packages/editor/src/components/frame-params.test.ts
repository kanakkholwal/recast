import { describe, expect, it } from "vitest";
import { EASE_IN_OUT } from "../lib/easing/cubic-bezier";
import { computeFrameParams, type FrameInput } from "./frame-params";
import type { CursorSampleJS } from "./video-preview.logic";
import type { CursorSettings, ShadowSettings, ZoomRegion } from "../stores/editor-store.svelte";

const GEOM = { canvasW: 1920, canvasH: 1080, videoX: 100, videoY: 50, videoW: 1720, videoH: 980 };

const CURSOR_OFF = {
	enabled: false,
	style: "dot",
	size: 1,
	hideWhenIdle: false,
	idleTimeout: 2,
	highlightClicks: false,
	highlightOpacity: 50,
	highlightColor: "#3b82f6",
} as unknown as CursorSettings;
const SHADOW_OFF = {
	enabled: false,
	opacity: 0,
	blur: 0,
	spread: 0,
	offsetY: 0,
	color: "#000000",
} as unknown as ShadowSettings;

function baseInput(over: Partial<FrameInput> = {}): FrameInput {
	return {
		meta: { width: 1920, height: 1080 },
		geom: GEOM,
		canvasPxW: 1920,
		canvasPxH: 1080,
		playbackTime: 0,
		segments: [],
		segmentAnims: [],
		backgroundType: "color",
		backgroundValue: "#111111",
		backgroundBlur: 0,
		backgroundImageReady: false,
		borderRadius: 0,
		focusEnabled: false,
		zoomRegions: [],
		shadow: SHADOW_OFF,
		cursor: CURSOR_OFF,
		cursorMotionEasing: null,
		cursorSamples: [],
		idlePeriods: [],
		pressEvents: [],
		...over,
	};
}

describe("computeFrameParams — identity scene", () => {
	const { uniforms, svgCursor, bindBackgroundImage } = computeFrameParams(baseInput());

	it("maps geometry 1:1 when the render buffer matches the canvas geometry", () => {
		expect(uniforms.canvasSize).toEqual([1920, 1080]);
		expect(uniforms.videoOrigin).toEqual([100, 50]);
		expect(uniforms.videoSize).toEqual([1720, 980]);
		expect(uniforms.videoOpacity).toBe(1);
		expect(uniforms.videoRotation).toBe(0);
	});

	it("packs a solid colour background", () => {
		expect(uniforms.bgType).toBe(0);
		expect(uniforms.bgColor[0]).toBeCloseTo(17 / 255, 5);
		expect(uniforms.bgColor[3]).toBe(1);
		expect(uniforms.bgBlurPx).toBe(0);
		expect(bindBackgroundImage).toBe(false);
	});

	it("leaves zoom, cursor, highlight and shadow inert", () => {
		expect(uniforms.zoomScale).toBe(1);
		expect(uniforms.zoomCenter).toEqual([0.5, 0.5]);
		expect(uniforms.motionBlurPx).toBe(0);
		expect(uniforms.cursorVisible).toBe(0);
		expect(uniforms.highlightAlpha).toBe(0);
		expect(uniforms.shadowEnabled).toBe(0);
		expect(uniforms.shadowColor).toEqual([0, 0, 0, 0]);
		expect(svgCursor).toBeNull();
	});
});

describe("computeFrameParams — render-buffer scale", () => {
	it("scales source geometry by the smaller render buffer", () => {
		const { uniforms } = computeFrameParams(baseInput({ canvasPxW: 960, canvasPxH: 540 }));
		expect(uniforms.canvasSize).toEqual([960, 540]);
		expect(uniforms.videoOrigin).toEqual([50, 25]);
		expect(uniforms.videoSize).toEqual([860, 490]);
	});

	it("converts border radius (percent of shorter edge) to canvas pixels via sx", () => {
		const full = computeFrameParams(baseInput({ borderRadius: 10 }));
		expect(full.uniforms.borderRadiusPx).toBeCloseTo(108, 5); // 0.10 * 1080 * 1
		const half = computeFrameParams(
			baseInput({ borderRadius: 10, canvasPxW: 960, canvasPxH: 540 }),
		);
		expect(half.uniforms.borderRadiusPx).toBeCloseTo(54, 5);
	});
});

describe("computeFrameParams — background modes", () => {
	it("passes packed gradient uniforms through", () => {
		const gradient = { colors: [1, 0, 0, 1], positions: [0, 1], count: 3, angleRad: 1.5 };
		const { uniforms } = computeFrameParams(baseInput({ backgroundType: "gradient", gradient }));
		expect(uniforms.bgType).toBe(1);
		expect(uniforms.gradColors).toBe(gradient.colors);
		expect(uniforms.gradStops).toBe(gradient.positions);
		expect(uniforms.gradCount).toBe(3);
		expect(uniforms.gradAngle).toBe(1.5);
	});

	it("binds and blurs an image background only once ready", () => {
		const ready = computeFrameParams(
			baseInput({ backgroundType: "image", backgroundImageReady: true, backgroundBlur: 50 }),
		);
		expect(ready.uniforms.bgType).toBe(2);
		expect(ready.uniforms.bgBlurPx).toBeCloseTo(12, 5); // 50 * 0.24
		expect(ready.bindBackgroundImage).toBe(true);

		const pending = computeFrameParams(
			baseInput({ backgroundType: "image", backgroundImageReady: false }),
		);
		expect(pending.uniforms.bgType).toBe(0);
		expect(pending.uniforms.bgColor).toEqual([0.067, 0.067, 0.067, 1]);
		expect(pending.bindBackgroundImage).toBe(false);
	});
});

describe("computeFrameParams — zoom", () => {
	const region = {
		start: 0,
		end: 10,
		scale: 2,
		centerX: 0.3,
		centerY: 0.7,
		rampIn: 0,
		rampOut: 0,
		easeIn: EASE_IN_OUT,
		easeOut: EASE_IN_OUT,
		motionBlur: 0,
		hidden: false,
	} as unknown as ZoomRegion;

	it("applies eased scale and constant focus centre in the hold", () => {
		const { uniforms } = computeFrameParams(
			baseInput({ focusEnabled: true, zoomRegions: [region], playbackTime: 5 }),
		);
		expect(uniforms.zoomScale).toBeCloseTo(2, 5);
		expect(uniforms.zoomCenter[0]).toBeCloseTo(0.3, 5);
		expect(uniforms.zoomCenter[1]).toBeCloseTo(0.7, 5);
	});

	it("stays at scale 1 when focus is disabled", () => {
		const { uniforms } = computeFrameParams(
			baseInput({ focusEnabled: false, zoomRegions: [region], playbackTime: 5 }),
		);
		expect(uniforms.zoomScale).toBe(1);
	});
});

describe("computeFrameParams — zoom motion blur", () => {
	// A ramped region so the scale is actually moving (motion blur is velocity-driven).
	const ramped = (motionBlur: number) =>
		({
			start: 0,
			end: 10,
			scale: 2,
			centerX: 0.5,
			centerY: 0.5,
			rampIn: 0.35,
			rampOut: 0.35,
			easeIn: EASE_IN_OUT,
			easeOut: EASE_IN_OUT,
			motionBlur,
			hidden: false,
		}) as unknown as ZoomRegion;

	it("smears strongly mid-ramp — past the old 20px clamp that made it invisible", () => {
		const { uniforms } = computeFrameParams(
			baseInput({ focusEnabled: true, zoomRegions: [ramped(0.5)], playbackTime: 0.175 }),
		);
		expect(uniforms.motionBlurPx).toBeGreaterThan(20);
	});

	it("is inert during the hold (no scale motion → no blur)", () => {
		const { uniforms } = computeFrameParams(
			baseInput({ focusEnabled: true, zoomRegions: [ramped(0.5)], playbackTime: 5 }),
		);
		expect(uniforms.motionBlurPx).toBeCloseTo(0, 5);
	});

	it("is off when the region's motion blur strength is 0", () => {
		const { uniforms } = computeFrameParams(
			baseInput({ focusEnabled: true, zoomRegions: [ramped(0)], playbackTime: 0.175 }),
		);
		expect(uniforms.motionBlurPx).toBe(0);
	});
});

describe("computeFrameParams — cursor", () => {
	const samples: CursorSampleJS[] = [
		{ timestampUs: 0, x: 960, y: 540, visible: true, leftDown: false, rightDown: false },
	];

	it("normalises the dot cursor position and drives the shader path", () => {
		const cursor = {
			...CURSOR_OFF,
			enabled: true,
			style: "dot",
			size: 1,
		} as unknown as CursorSettings;
		const { uniforms, svgCursor } = computeFrameParams(
			baseInput({ cursor, cursorSamples: samples, playbackTime: 1 }),
		);
		expect(uniforms.cursorPos[0]).toBeCloseTo(0.5, 5);
		expect(uniforms.cursorPos[1]).toBeCloseTo(0.5, 5);
		expect(uniforms.cursorVisible).toBe(1);
		expect(svgCursor).toBeNull();
	});

	it("routes a non-dot style to the SVG overlay and suppresses the shader dot", () => {
		const cursor = {
			...CURSOR_OFF,
			enabled: true,
			style: "arrow",
			size: 2,
		} as unknown as CursorSettings;
		const { uniforms, svgCursor } = computeFrameParams(
			baseInput({ cursor, cursorSamples: samples, playbackTime: 1 }),
		);
		expect(uniforms.cursorVisible).toBe(0);
		expect(svgCursor).not.toBeNull();
		expect(svgCursor?.styleId).toBe("arrow");
		expect(svgCursor?.visible).toBe(true);
		expect(svgCursor?.canvasX).toBeCloseTo(960, 5); // 100 + 0.5 * 1720
		expect(svgCursor?.canvasY).toBeCloseTo(540, 5); // 50 + 0.5 * 980
		expect(svgCursor?.spritePx).toBe(32); // size 2 * 16
	});
});

describe("computeFrameParams — shadow", () => {
	it("scales shadow geometry by sx and folds opacity into the colour alpha", () => {
		const shadow = {
			enabled: true,
			opacity: 50,
			blur: 20,
			spread: 4,
			offsetY: 8,
			color: "#000000",
		} as unknown as ShadowSettings;
		const { uniforms } = computeFrameParams(baseInput({ shadow }));
		expect(uniforms.shadowEnabled).toBe(1);
		expect(uniforms.shadowBlurPx).toBeCloseTo(20, 5);
		expect(uniforms.shadowSpreadPx).toBeCloseTo(4, 5);
		expect(uniforms.shadowOffsetPx).toEqual([0, 8]);
		expect(uniforms.shadowColor).toEqual([0, 0, 0, 0.5]);
	});
});
