import { describe, expect, it } from "vitest";
import { captionHeightFrac, captionTopFrac } from "./layout";

describe("captionHeightFrac", () => {
	it("scales with size and lines, capped at 0.7", () => {
		expect(captionHeightFrac(5, 1)).toBeCloseTo(0.0675);
		expect(captionHeightFrac(5, 2)).toBeCloseTo(0.135);
		expect(captionHeightFrac(60, 6)).toBe(0.7); // clamped
	});
});

describe("captionTopFrac", () => {
	// Video occupies the middle 70% vertically (15% padding top and bottom).
	const padded = { top: 0.15, bottom: 0.85 };
	const cap = 0.12;

	it("center returns null", () => {
		expect(captionTopFrac("center", 8, cap, padded)).toBeNull();
	});

	it("bottom sits just below the video, in the padding", () => {
		const top = captionTopFrac("bottom", 0, cap, padded)!;
		// Block top at/after the video bottom → does not cover the video.
		expect(top).toBeGreaterThanOrEqual(padded.bottom - 1e-9);
		// And fits on-frame.
		expect(top + cap).toBeLessThanOrEqual(1 + 1e-9);
	});

	it("top sits just above the video, in the padding", () => {
		const top = captionTopFrac("top", 0, cap, padded)!;
		// Block bottom (top + cap) is at/above the video top → no overlap.
		expect(top + cap).toBeLessThanOrEqual(padded.top + 1e-9);
		expect(top).toBeGreaterThanOrEqual(0);
	});

	it("positive offset lifts a bottom caption INWARD over the video", () => {
		const a = captionTopFrac("bottom", 0, cap, { top: 0.1, bottom: 0.7 })!;
		const b = captionTopFrac("bottom", 5, cap, { top: 0.1, bottom: 0.7 })!;
		expect(b).toBeLessThan(a); // top edge moves up = inward
	});

	it("negative offset tucks a bottom caption outward into the padding", () => {
		const v = { top: 0.15, bottom: 0.85 };
		const base = captionTopFrac("bottom", 0, cap, v)!;
		const tucked = captionTopFrac("bottom", -10, cap, v)!;
		expect(tucked).toBeGreaterThan(base);
	});

	it("keeps the whole Offset range live on a full-bleed video (no dead clamp)", () => {
		// Full-bleed video: bottom edge == frame bottom. Baseline anchors at the
		// frame edge, so positive Offset still lifts the caption up visibly.
		const full = { top: 0, bottom: 1 };
		const base = captionTopFrac("bottom", 0, cap, full)!;
		const lifted = captionTopFrac("bottom", 8, cap, full)!;
		expect(base).toBeCloseTo(1 - cap); // sits at the frame bottom
		expect(lifted).toBeCloseTo(1 - cap - 0.08); // and 8% lifts it up, not dead
		expect(lifted).toBeLessThan(base);
	});

	it("top: positive offset pushes inward (down) even with no top padding", () => {
		const full = { top: 0, bottom: 1 };
		expect(captionTopFrac("top", 0, cap, full)).toBe(0); // baseline at frame top
		expect(captionTopFrac("top", 8, cap, full)).toBeCloseTo(0.08); // pushed inward
	});
});
