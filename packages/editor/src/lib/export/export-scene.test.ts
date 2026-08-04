import { describe, expect, it, vi } from "vitest";

// computeCanvasGeometry transitively imports the editor store (a value import of
// `aspectRatio`), which pulls a $constants chain vitest can't resolve. Mock it —
// this test covers buildExportBase's OWN logic (rounding + field passthrough),
// not the geometry math (which is pure and covered separately).
vi.mock("../canvas-geometry", () => ({
	computeCanvasGeometry: () => ({
		canvasW: 1920.4,
		canvasH: 1080.6,
		videoX: 0,
		videoY: 0,
		videoW: 1920,
		videoH: 1080,
		paddingPx: 0,
		compX: 0,
		compY: 0,
		compW: 1920,
		compH: 1080,
	}),
}));

import { buildExportBase, type ExportSceneInputs } from "./export-scene";

const inputs = {
	meta: { width: 1920, height: 1080 },
	padding: 0,
	outputAspect: "source",
	segments: [],
	segmentAnims: [],
	backgroundType: "color",
	backgroundValue: "#111111",
	backgroundBlur: 0,
	backgroundImageReady: false,
	gradient: undefined,
	borderRadius: 0,
	focusEnabled: false,
	zoomRegions: [],
	shadow: {} as never,
	cursor: {} as never,
	cursorMotionEasing: null,
	cursorSamples: [],
	idlePeriods: [],
	pressEvents: [],
} as unknown as ExportSceneInputs;

describe("buildExportBase", () => {
	it("sets the render buffer to the rounded composition native size", () => {
		const base = buildExportBase(inputs);
		expect(base.geom.canvasW).toBe(1920.4);
		expect(base.canvasPxW).toBe(1920); // round(1920.4)
		expect(base.canvasPxH).toBe(1081); // round(1080.6)
	});

	it("carries static scene fields through and drops the derived-only keys", () => {
		const base = buildExportBase(inputs) as Record<string, unknown>;
		expect(base.backgroundType).toBe("color");
		expect(base.backgroundImageReady).toBe(false);
		expect("padding" in base).toBe(false);
		expect("outputAspect" in base).toBe(false);
		expect("playbackTime" in base).toBe(false);
	});
});
