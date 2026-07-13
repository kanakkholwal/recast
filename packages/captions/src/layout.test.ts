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

	it("offset pushes further into the padding for bottom", () => {
		const a = captionTopFrac("bottom", 0, cap, { top: 0.1, bottom: 0.7 })!;
		const b = captionTopFrac("bottom", 5, cap, { top: 0.1, bottom: 0.7 })!;
		expect(b).toBeGreaterThan(a);
	});

	it("negative offset pulls a bottom caption up onto the video", () => {
		const v = { top: 0.15, bottom: 0.85 };
		const base = captionTopFrac("bottom", 0, cap, v)!;
		const pulled = captionTopFrac("bottom", -10, cap, v)!;
		expect(pulled).toBeLessThan(base);
	});

	it("clamps to the frame when there is no padding (falls back over video)", () => {
		// Full-bleed video: bottom edge == frame bottom.
		const full = { top: 0, bottom: 1 };
		const top = captionTopFrac("bottom", 8, cap, full)!;
		// Can't go below the frame → clamped so the block stays visible.
		expect(top).toBeCloseTo(1 - cap);
		expect(top + cap).toBeLessThanOrEqual(1 + 1e-9);
	});

	it("top clamps to 0 when there is no top padding", () => {
		const full = { top: 0, bottom: 1 };
		expect(captionTopFrac("top", 8, cap, full)).toBe(0);
	});
});
