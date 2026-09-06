import { describe, expect, it } from "vitest";
import {
	clampResolution,
	clampScroll,
	DEFAULT_RESOLUTION,
	frameToX,
	maxScrollFrames,
	RESOLUTION_MAX,
	RESOLUTION_MIN,
	type TimelineView,
	xToFrame,
	zoomAt,
} from "./canvas-view";

const view = (scrollFrames: number, resolution: number): TimelineView => ({
	scrollFrames,
	resolution,
});

describe("canvas-view geometry", () => {
	it("round-trips a frame through pixels and back", () => {
		const v = view(120, 2.5);
		for (const frame of [0, 60, 120, 1000]) {
			expect(xToFrame(frameToX(frame, v), v)).toBeCloseTo(frame, 6);
		}
	});

	it("places the scroll origin at x=0", () => {
		expect(frameToX(300, view(300, 4))).toBe(0);
	});

	it("clamps resolution into range and defaults a bad value", () => {
		expect(clampResolution(1000)).toBe(RESOLUTION_MAX);
		expect(clampResolution(0.0001)).toBe(RESOLUTION_MIN);
		expect(clampResolution(Number.NaN)).toBe(DEFAULT_RESOLUTION);
	});

	it("zooms while pinning the frame under the cursor to the same pixel", () => {
		const v = view(100, 2);
		const cursorX = 240;
		const pinnedFrame = xToFrame(cursorX, v);
		const zoomed = zoomAt(v, cursorX, 1.7);
		expect(zoomed.resolution).toBeCloseTo(3.4, 6);
		expect(frameToX(pinnedFrame, zoomed)).toBeCloseTo(cursorX, 4);
	});

	it("does not move when a zoom hits the resolution ceiling", () => {
		const v = view(0, RESOLUTION_MAX);
		expect(zoomAt(v, 100, 2)).toBe(v);
	});

	it("bounds scroll to the content extent", () => {
		// 600 frames of content, 300px viewport at 1px/frame -> 300 visible frames.
		expect(maxScrollFrames(600, 300, 1)).toBe(300);
		const clamped = clampScroll(view(9999, 1), 600, 300);
		expect(clamped.scrollFrames).toBe(300);
		expect(clampScroll(view(-50, 1), 600, 300).scrollFrames).toBe(0);
	});

	it("clamps to zero when content under-fills the viewport", () => {
		expect(maxScrollFrames(100, 300, 1)).toBe(0);
	});
});
