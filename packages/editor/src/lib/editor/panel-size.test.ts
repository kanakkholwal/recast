import { describe, expect, it } from "vitest";
import {
	clampSidebarWidth,
	clampTimelineHeight,
	SIDEBAR_DEFAULT_WIDTH_PX,
	SIDEBAR_MAX_WIDTH_PX,
	SIDEBAR_MIN_WIDTH_PX,
	TIMELINE_DEFAULT_HEIGHT_PX,
	TIMELINE_MAX_HEIGHT_PX,
	TIMELINE_MAX_SHARE,
	TIMELINE_MIN_HEIGHT_PX,
	timelineMaxHeight,
} from "./panel-size";

describe("timelineMaxHeight", () => {
	// The reason the panel is bounded at all: every lane visible left the video
	// a strip, so the timeline must never be able to take the column.
	it("never exceeds its share of the column", () => {
		for (const column of [600, 800, 1080, 1440]) {
			expect(timelineMaxHeight(column), `column ${column}`).toBeLessThanOrEqual(
				Math.round(column * TIMELINE_MAX_SHARE),
			);
		}
	});

	it("leaves the preview the larger share", () => {
		for (const column of [600, 800, 1080, 1440]) {
			const max = timelineMaxHeight(column);
			expect(column - max, `column ${column}`).toBeGreaterThan(max);
		}
	});

	it("caps at the absolute ceiling on a tall display", () => {
		expect(timelineMaxHeight(4000)).toBe(TIMELINE_MAX_HEIGHT_PX);
	});

	// Falling back to the share of zero would collapse the panel to its floor for
	// a frame, then jump once layout settled.
	it("falls back to the ceiling before the column is measured", () => {
		expect(timelineMaxHeight(0)).toBe(TIMELINE_MAX_HEIGHT_PX);
		expect(timelineMaxHeight(-1)).toBe(TIMELINE_MAX_HEIGHT_PX);
		expect(timelineMaxHeight(Number.NaN)).toBe(TIMELINE_MAX_HEIGHT_PX);
	});

	it("keeps the floor usable on a very short window", () => {
		expect(timelineMaxHeight(200)).toBe(TIMELINE_MIN_HEIGHT_PX);
	});
});

describe("clampTimelineHeight", () => {
	const COLUMN = 900;

	it("passes a height inside the bounds straight through", () => {
		expect(clampTimelineHeight(300, COLUMN)).toBe(300);
	});

	it("holds both ends", () => {
		expect(clampTimelineHeight(0, COLUMN)).toBe(TIMELINE_MIN_HEIGHT_PX);
		expect(clampTimelineHeight(-500, COLUMN)).toBe(TIMELINE_MIN_HEIGHT_PX);
		expect(clampTimelineHeight(9000, COLUMN)).toBe(timelineMaxHeight(COLUMN));
	});

	// A height saved on a big display must not swallow the preview on a laptop.
	it("re-clamps a height stored against a taller column", () => {
		const onBigDisplay = clampTimelineHeight(540, 1440);
		expect(clampTimelineHeight(onBigDisplay, 700)).toBe(timelineMaxHeight(700));
	});

	it("lands on whole pixels", () => {
		expect(clampTimelineHeight(300.4, COLUMN)).toBe(300);
		expect(clampTimelineHeight(300.6, COLUMN)).toBe(301);
	});

	it("recovers from a corrupt stored value", () => {
		expect(clampTimelineHeight(Number.NaN, COLUMN)).toBe(TIMELINE_DEFAULT_HEIGHT_PX);
	});
});

describe("clampSidebarWidth", () => {
	it("holds the panel between its bounds", () => {
		expect(clampSidebarWidth(10)).toBe(SIDEBAR_MIN_WIDTH_PX);
		expect(clampSidebarWidth(99999)).toBe(SIDEBAR_MAX_WIDTH_PX);
		expect(clampSidebarWidth(400.6)).toBe(401);
	});

	// localStorage returns "" for a missing key, and Number("") is 0 — which must
	// not collapse the panel to its minimum on first open.
	it("uses the default for a non-numeric stored value", () => {
		expect(clampSidebarWidth(Number.NaN)).toBe(SIDEBAR_DEFAULT_WIDTH_PX);
	});
});
