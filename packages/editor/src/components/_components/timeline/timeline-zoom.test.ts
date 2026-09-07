import { describe, expect, it } from "vitest";
import {
	clampTimelineZoom,
	MAX_PIXELS_PER_SECOND,
	MIN_TIMELINE_ZOOM,
	maxTimelineZoom,
	steppedZoom,
} from "./timeline-helpers";

const VIEWPORT = 900;

/** Zoom is a multiple of fit, so px/sec is what actually matters to the user. */
function pixelsPerSecond(zoom: number, duration: number): number {
	return (VIEWPORT * zoom) / duration;
}

describe("maxTimelineZoom", () => {
	// The bug: a flat 5x ceiling made magnification a function of clip length, so a 30-minute recording couldn't be trimmed to a frame.
	it("lets a long recording reach the same px/sec as a short one", () => {
		for (const duration of [30, 300, 1800]) {
			const zoom = maxTimelineZoom(duration, VIEWPORT);
			expect(pixelsPerSecond(zoom, duration)).toBeCloseTo(MAX_PIXELS_PER_SECOND, 6);
		}
	});

	it("gives a 30-minute clip enough room to aim at a frame", () => {
		const duration = 30 * 60;
		const pps = pixelsPerSecond(maxTimelineZoom(duration, VIEWPORT), duration);
		// Old ceiling was 2.5 px/sec, i.e. 0.04px per frame at 60fps.
		expect(pps / 60).toBeGreaterThan(5);
	});

	it("never drops below fit, even for a clip shorter than the viewport", () => {
		expect(maxTimelineZoom(0.5, VIEWPORT)).toBe(MIN_TIMELINE_ZOOM);
	});

	it("is safe before metadata loads", () => {
		expect(maxTimelineZoom(0, VIEWPORT)).toBe(MIN_TIMELINE_ZOOM);
		expect(maxTimelineZoom(60, 0)).toBe(MIN_TIMELINE_ZOOM);
	});
});

describe("clampTimelineZoom", () => {
	it("floors at fit, so the viewport never grows dead space", () => {
		expect(clampTimelineZoom(0.5, 60, VIEWPORT)).toBe(MIN_TIMELINE_ZOOM);
	});

	it("ceilings at the px/sec limit", () => {
		const max = maxTimelineZoom(60, VIEWPORT);
		expect(clampTimelineZoom(max * 10, 60, VIEWPORT)).toBeCloseTo(max, 6);
	});

	// A persisted zoom from another window size must be pulled back into range.
	it("pulls an out-of-range persisted zoom back", () => {
		const zoom = clampTimelineZoom(999, 10, VIEWPORT);
		expect(zoom).toBeLessThanOrEqual(maxTimelineZoom(10, VIEWPORT));
		expect(zoom).toBeGreaterThanOrEqual(MIN_TIMELINE_ZOOM);
	});
});

describe("steppedZoom", () => {
	it("is multiplicative, so one step covers the same proportion at any length", () => {
		const inOnce = steppedZoom(1, 1, 600, VIEWPORT);
		expect(inOnce).toBeGreaterThan(1);
		// Stepping back out returns to where it started.
		expect(steppedZoom(inOnce, -1, 600, VIEWPORT)).toBeCloseTo(1, 6);
	});

	it("cannot step below fit", () => {
		expect(steppedZoom(1, -1, 600, VIEWPORT)).toBe(MIN_TIMELINE_ZOOM);
	});

	it("cannot step past the ceiling", () => {
		const max = maxTimelineZoom(600, VIEWPORT);
		expect(steppedZoom(max, 1, 600, VIEWPORT)).toBeCloseTo(max, 6);
	});
});
