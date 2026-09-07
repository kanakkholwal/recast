import { describe, expect, it } from "vitest";
import {
	buildPreviewState,
	CONTROL_BAR_HEIGHT,
	computeSizeConstraints,
	fitInsideMax,
	MAX_SCREEN_FRACTION,
	MIN_LOGICAL_SIZE,
} from "./camera-preview.logic";

describe("fitInsideMax", () => {
	it("passes a box through unchanged when it fits", () => {
		expect(fitInsideMax(200, 200, 1, 400, 400)).toEqual([200, 200]);
	});

	it("clamps by width, deriving height from the ratio", () => {
		// 16:9 box wider than max → width pinned to 400, height = 400/(16/9).
		expect(fitInsideMax(800, 450, 16 / 9, 400, 400)).toEqual([400, 225]);
	});

	it("clamps by height after width, keeping the ratio", () => {
		// Tall 1:1 box: width fits but height exceeds max → both snap to 300.
		expect(fitInsideMax(300, 600, 1, 400, 300)).toEqual([300, 300]);
	});

	it("rounds to whole pixels", () => {
		const [w, h] = fitInsideMax(333, 333 / (4 / 3), 4 / 3, 200, 200);
		expect(Number.isInteger(w)).toBe(true);
		expect(Number.isInteger(h)).toBe(true);
	});
});

describe("computeSizeConstraints", () => {
	it("derives a square max box from the screen fraction", () => {
		const c = computeSizeConstraints(1920);
		const expectedMax = Math.floor(1920 * MAX_SCREEN_FRACTION);
		expect(c.maxLogicalW).toBe(expectedMax);
		expect(c.maxLogicalH).toBe(expectedMax);
	});

	it("min height = widest-aspect video height + control strip", () => {
		const c = computeSizeConstraints(1920);
		// Widest ratio is 16:9; min video height floors on that.
		const expected = Math.round(MIN_LOGICAL_SIZE / (16 / 9)) + CONTROL_BAR_HEIGHT;
		expect(c.minWinH).toBe(expected);
		expect(c.minLogicalW).toBe(MIN_LOGICAL_SIZE);
	});

	it("floors tiny screens at 320", () => {
		const c = computeSizeConstraints(100);
		expect(c.maxLogicalW).toBe(Math.floor(320 * MAX_SCREEN_FRACTION));
	});
});

describe("buildPreviewState", () => {
	const screen = { width: 1000, height: 1000 };

	it("subtracts the control strip from the reported video height", () => {
		// dpr 1, 200×240 window → video height 240 − 40 = 200.
		const s = buildPreviewState(
			{ x: 0, y: 0 },
			{ width: 200, height: 240 },
			screen,
			1,
			"rounded",
			true,
			"live",
		);
		expect(s.windowHeight).toBeCloseTo(200 / 1000);
		expect(s.windowWidth).toBeCloseTo(200 / 1000);
	});

	it("uses fixed corner radii for square and circle", () => {
		const base = [{ x: 0, y: 0 }, { width: 200, height: 240 }, screen, 1] as const;
		expect(buildPreviewState(...base, "square", false, "live").cornerRadius).toBe(0);
		expect(buildPreviewState(...base, "circle", false, "live").cornerRadius).toBe(0.5);
	});

	it("scales the rounded radius by the shorter side, capped at 0.5", () => {
		const s = buildPreviewState(
			{ x: 0, y: 0 },
			{ width: 200, height: 240 },
			screen,
			1,
			"rounded",
			false,
			"live",
		);
		// shorter side = 200 → 20/200 = 0.1.
		expect(s.cornerRadius).toBeCloseTo(0.1);
	});

	it("clamps normalized position into [0,1]", () => {
		const s = buildPreviewState(
			{ x: -50, y: 5000 },
			{ width: 200, height: 240 },
			screen,
			1,
			"rounded",
			false,
			"live",
		);
		expect(s.windowX).toBe(0);
		expect(s.windowY).toBe(1);
	});

	it("switches the animation preset to lively on a warning", () => {
		const s = buildPreviewState(
			{ x: 0, y: 0 },
			{ width: 200, height: 240 },
			screen,
			1,
			"rounded",
			false,
			"warning",
		);
		expect(s.animationPreset).toBe("lively");
	});
});
