import { describe, expect, it } from "vitest";
import {
	buildPressEvents,
	clickAnchorAt,
	clickHighlightAt,
	type PressEvent,
	type PressSample,
	pressStateAt,
	smoothStep01,
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

describe("press lookups near the end of a long timeline", () => {
	function click(i: number, holdUs = 50_000): PressEvent {
		const downUs = i * 1_000_000;
		return { downUs, upUs: downUs + holdUs, downX: i, downY: i, right: false, dragged: false };
	}
	// 30 min at one click a second.
	const events = Array.from({ length: 1800 }, (_, i) => click(i));
	const LATE = 1_500_000_000;

	function counting(list: PressEvent[]) {
		let reads = 0;
		const proxy = new Proxy(list, {
			get(target, prop, recv) {
				if (typeof prop === "string" && /^\d+$/.test(prop)) reads++;
				return Reflect.get(target, prop, recv);
			},
		});
		return {
			proxy,
			reads: () => reads,
			reset: () => {
				reads = 0;
			},
		};
	}

	it("does not rescan the whole track per frame", () => {
		const { proxy, reads, reset } = counting(events);
		// First call pays the one-time index build; steady state is what matters.
		clickAnchorAt(proxy, LATE);
		clickHighlightAt(proxy, LATE);
		pressStateAt(proxy, LATE);
		reset();

		clickAnchorAt(proxy, LATE);
		clickHighlightAt(proxy, LATE);
		pressStateAt(proxy, LATE);

		expect(reads()).toBeLessThan(20);
	});

	it("still finds the click under the playhead", () => {
		expect(clickAnchorAt(events, LATE)?.x).toBe(1500);
		expect(clickHighlightAt(events, LATE)?.x).toBe(1500);
		expect(pressStateAt(events, LATE).pressedSprite).toBe(true);
	});

	it("still finds a press held since long before the playhead", () => {
		const held = [click(0, 100_000_000), ...Array.from({ length: 200 }, (_, i) => click(i + 200))];
		expect(pressStateAt(held, 100_100_000).pressedSprite).toBe(true);
		expect(clickHighlightAt(held, 100_100_000)?.x).toBe(0);
	});
});
