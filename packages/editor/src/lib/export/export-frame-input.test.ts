import { describe, expect, it } from "vitest";
import { buildTimeMap } from "$lib/timeline/time-map";
import type { FrameInput } from "../../components/editor/frame-params";
import { makeExportFrameAt, makeIndexedExportFrameAt } from "./export-frame-input";

const base = {
	meta: { width: 1920, height: 1080 },
	canvasPxW: 1920,
	canvasPxH: 1080,
	backgroundType: "color",
} as unknown as Omit<FrameInput, "playbackTime">;

describe("makeExportFrameAt", () => {
	it("maps output time to original time (identity map = no cuts, 1x)", () => {
		const map = buildTimeMap([{ origStart: 0, origEnd: 10, speed: 1 }]);
		const at = makeExportFrameAt(base, map);
		expect(at(0, 0).originalSec).toBe(0);
		expect(at(120, 4).originalSec).toBeCloseTo(4);
		expect(at(120, 4).input.playbackTime).toBeCloseTo(4);
	});

	it("applies per-segment speed: a 2x span warps output time back to original", () => {
		// orig 0..10 at 2x → output 0..5; output 2s → original 4s.
		const map = buildTimeMap([{ origStart: 0, origEnd: 10, speed: 2 }]);
		const at = makeExportFrameAt(base, map);
		expect(at(0, 2).originalSec).toBeCloseTo(4);
		expect(at(0, 2).input.playbackTime).toBeCloseTo(4);
	});

	it("skips a removed span (cut): output past the seam lands in the next kept span", () => {
		// keep 0..2 and 8..10 (2..8 cut) → output 0..4; output 3s → original 9s.
		const map = buildTimeMap([
			{ origStart: 0, origEnd: 2, speed: 1 },
			{ origStart: 8, origEnd: 10, speed: 1 },
		]);
		const at = makeExportFrameAt(base, map);
		expect(at(0, 1).originalSec).toBeCloseTo(1);
		expect(at(0, 3).originalSec).toBeCloseTo(9);
	});

	it("carries every static field through unchanged", () => {
		const map = buildTimeMap([{ origStart: 0, origEnd: 10, speed: 1 }]);
		const { input } = makeExportFrameAt(base, map)(0, 5);
		expect(input.meta).toEqual({ width: 1920, height: 1080 });
		expect(input.canvasPxW).toBe(1920);
		expect(input.backgroundType).toBe("color");
	});
});

describe("makeIndexedExportFrameAt", () => {
	it("derives outputSec from index/fps so it can't drift from the encoder clock", () => {
		const map = buildTimeMap([{ origStart: 0, origEnd: 10, speed: 1 }]);
		const at = makeIndexedExportFrameAt(base, map, 30);
		expect(at(0).originalSec).toBe(0);
		expect(at(30).originalSec).toBeCloseTo(1);
		expect(at(45).input.playbackTime).toBeCloseTo(1.5);
	});
});
