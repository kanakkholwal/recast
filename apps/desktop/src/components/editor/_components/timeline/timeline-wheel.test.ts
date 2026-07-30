import { describe, expect, it } from "vitest";
import { type WheelGesture, wheelIntent } from "./timeline-wheel.logic";

function gesture(partial: Partial<WheelGesture>): WheelGesture {
	return { deltaX: 0, deltaY: 0, shiftKey: false, ctrlKey: false, metaKey: false, ...partial };
}

describe("wheelIntent", () => {
	it("scrolls the lanes when they overflow", () => {
		expect(wheelIntent(gesture({ deltaY: 120 }), true)).toEqual({ kind: "vertical", delta: 120 });
		expect(wheelIntent(gesture({ deltaY: -80 }), true)).toEqual({ kind: "vertical", delta: -80 });
	});

	it("falls back to panning when there is nothing below the fold", () => {
		expect(wheelIntent(gesture({ deltaY: 120 }), false)).toEqual({
			kind: "horizontal",
			delta: 120,
		});
	});

	it("pans on shift even while the lanes overflow", () => {
		expect(wheelIntent(gesture({ deltaY: 90, shiftKey: true }), true)).toEqual({
			kind: "horizontal",
			delta: 90,
		});
		// Platforms that report shift+wheel as deltaX still pan.
		expect(wheelIntent(gesture({ deltaX: 90, shiftKey: true }), true)).toEqual({
			kind: "horizontal",
			delta: 90,
		});
	});

	it("pans on a horizontal trackpad swipe", () => {
		expect(wheelIntent(gesture({ deltaX: 60, deltaY: 4 }), true)).toEqual({
			kind: "horizontal",
			delta: 60,
		});
	});

	it("zooms on ctrl or meta regardless of overflow", () => {
		expect(wheelIntent(gesture({ deltaY: -10, ctrlKey: true }), true)).toEqual({
			kind: "zoom",
			direction: 1,
		});
		expect(wheelIntent(gesture({ deltaY: 10, metaKey: true }), false)).toEqual({
			kind: "zoom",
			direction: -1,
		});
	});

	it("ignores gestures that moved nothing", () => {
		expect(wheelIntent(gesture({}), true)).toEqual({ kind: "none" });
		expect(wheelIntent(gesture({ ctrlKey: true }), true)).toEqual({ kind: "none" });
	});
});
