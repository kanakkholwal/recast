import { describe, expect, it } from "vitest";
import {
	BOUNCE,
	bezierY,
	clampEasingCoord,
	EASE_IN_OUT,
	EASING_OVERSHOOT,
	EASING_PRESETS,
	LINEAR,
} from "./cubic-bezier";

describe("clampEasingCoord", () => {
	it("holds x inside the unit interval", () => {
		expect(clampEasingCoord("x1", -0.4)).toBe(0);
		expect(clampEasingCoord("x2", 1.8)).toBe(1);
		expect(clampEasingCoord("x1", 0.42)).toBe(0.42);
	});

	// Overshoot is the point of a bounce curve, so y is deliberately allowed
	// outside [0,1] — just not so far that the handle leaves the editor's viewBox
	// and becomes ungrabbable. Typing used to skip this clamp entirely.
	it("allows y to overshoot but not escape the graph", () => {
		expect(clampEasingCoord("y1", -0.55)).toBe(-0.55);
		expect(clampEasingCoord("y2", 1.55)).toBe(1.55);
		expect(clampEasingCoord("y1", -50)).toBe(-EASING_OVERSHOOT);
		expect(clampEasingCoord("y2", 50)).toBe(1 + EASING_OVERSHOOT);
	});

	it("rejects non-finite input rather than poisoning the curve", () => {
		expect(clampEasingCoord("x1", Number.NaN)).toBe(0);
		expect(clampEasingCoord("y1", Number.POSITIVE_INFINITY)).toBe(1 + EASING_OVERSHOOT);
	});

	// Tightening the overshoot band would silently flatten Bounce, which is the
	// one shipped preset that depends on it.
	it("leaves every shipped preset untouched", () => {
		for (const preset of EASING_PRESETS) {
			const v = preset.value;
			expect(clampEasingCoord("x1", v.x1), preset.id).toBe(v.x1);
			expect(clampEasingCoord("y1", v.y1), preset.id).toBe(v.y1);
			expect(clampEasingCoord("x2", v.x2), preset.id).toBe(v.x2);
			expect(clampEasingCoord("y2", v.y2), preset.id).toBe(v.y2);
		}
	});
});

describe("bezierY", () => {
	it("pins the endpoints", () => {
		expect(bezierY(EASE_IN_OUT, 0)).toBe(0);
		expect(bezierY(EASE_IN_OUT, 1)).toBe(1);
		expect(bezierY(EASE_IN_OUT, -0.5)).toBe(0);
		expect(bezierY(EASE_IN_OUT, 1.5)).toBe(1);
	});

	it("is the identity for linear", () => {
		for (const x of [0.1, 0.25, 0.5, 0.75, 0.9]) {
			expect(bezierY(LINEAR, x)).toBeCloseTo(x, 6);
		}
	});

	it("is symmetric about the midpoint for in-out", () => {
		expect(bezierY(EASE_IN_OUT, 0.5)).toBeCloseTo(0.5, 4);
		expect(bezierY(EASE_IN_OUT, 0.25) + bezierY(EASE_IN_OUT, 0.75)).toBeCloseTo(1, 4);
	});

	it("overshoots past 1 for bounce, which the clamp band must allow", () => {
		const peak = Math.max(...Array.from({ length: 101 }, (_, i) => bezierY(BOUNCE, i / 100)));
		expect(peak).toBeGreaterThan(1);
		expect(peak).toBeLessThanOrEqual(1 + EASING_OVERSHOOT);
	});
});
