import { describe, expect, it } from "vitest";
import {
	buildPressEvents,
	clickAnchorAt,
	clickHighlightAt,
	pressStateAt,
	smoothStep01,
	type PressSample,
} from "./cursor-animation.logic";

function sample(over: Partial<PressSample> & { timestampUs: number }): PressSample {
	return { x: 0, y: 0, leftDown: false, rightDown: false, ...over };
}

describe("smoothStep01", () => {
	it("clamps and is symmetric at the midpoint", () => {
		expect(smoothStep01(-1)).toBe(0);
		expect(smoothStep01(0)).toBe(0);
		expect(smoothStep01(1)).toBe(1);
		expect(smoothStep01(2)).toBe(1);
		expect(smoothStep01(0.5)).toBe(0.5);
	});
});

describe("buildPressEvents", () => {
	it("collapses a held button into one event with position", () => {
		const events = buildPressEvents([
			sample({ timestampUs: 0 }),
			sample({ timestampUs: 1000, x: 10, y: 20, leftDown: true }),
			sample({ timestampUs: 2000, x: 10, y: 20, leftDown: true }),
			sample({ timestampUs: 3000 }),
		]);
		expect(events).toHaveLength(1);
		expect(events[0]).toMatchObject({
			downUs: 1000,
			upUs: 3000,
			downX: 10,
			downY: 20,
			right: false,
			dragged: false,
		});
	});
	it("flags a drag past the threshold and right-clicks", () => {
		const events = buildPressEvents([
			sample({ timestampUs: 0, x: 0, y: 0, rightDown: true }),
			sample({ timestampUs: 1000, x: 50, y: 0, rightDown: true }),
			sample({ timestampUs: 2000 }),
		]);
		expect(events[0].right).toBe(true);
		expect(events[0].dragged).toBe(true);
	});
	it("closes an open press at the last sample", () => {
		const events = buildPressEvents([
			sample({ timestampUs: 1000, leftDown: true }),
			sample({ timestampUs: 2000, leftDown: true }),
		]);
		expect(events).toHaveLength(1);
		expect(events[0].upUs).toBe(2000);
	});
});

describe("clickAnchorAt / clickHighlightAt", () => {
	const events = buildPressEvents([
		sample({ timestampUs: 1_000_000, x: 100, y: 200, leftDown: true }),
		sample({ timestampUs: 1_010_000 }),
	]);
	it("snaps to the anchor at the click frame with full weight", () => {
		const a = clickAnchorAt(events, 1_000_000);
		expect(a).not.toBeNull();
		expect(a!.x).toBe(100);
		expect(a!.weight).toBeCloseTo(1, 5);
	});
	it("returns null far from any click", () => {
		expect(clickAnchorAt(events, 5_000_000)).toBeNull();
	});
	it("highlights at full alpha during the hold", () => {
		const hl = clickHighlightAt(events, 1_050_000);
		expect(hl).not.toBeNull();
		expect(hl!.alpha).toBe(1);
	});
});

describe("pressStateAt", () => {
	const events = buildPressEvents([
		sample({ timestampUs: 1_000_000, x: 0, y: 0, leftDown: true }),
		sample({ timestampUs: 1_010_000 }),
	]);
	it("is idle far from any press", () => {
		expect(pressStateAt(events, 5_000_000)).toMatchObject({
			pressedSprite: false,
			visibleAlpha: 0,
			scale: 1,
		});
	});
	it("shows the pressed sprite during the hold", () => {
		expect(pressStateAt(events, 1_000_000).pressedSprite).toBe(true);
	});
	it("snaps scale below 1 right at the click frame (punch)", () => {
		expect(pressStateAt(events, 1_000_000).scale).toBeLessThan(1);
	});
});
