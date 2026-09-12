import { describe, expect, it } from "vitest";
import { evalOpacity } from "./eval";
import { clickPlacedArrow, clickPlacedBox, placedTimeRange } from "./place-defaults";

const WIDE = { w: 1920, h: 1080 };
const TALL = { w: 1080, h: 1920 };

describe("clickPlacedBox", () => {
	it("centres on the click", () => {
		const b = clickPlacedBox(0.5, 0.5, WIDE.w, WIDE.h);
		expect(b.x + b.w / 2).toBeCloseTo(0.5, 6);
		expect(b.y + b.h / 2).toBeCloseTo(0.5, 6);
	});

	it("stays inside the frame at every corner", () => {
		for (const [ux, uy] of [
			[0, 0],
			[1, 1],
			[0.01, 0.99],
		]) {
			for (const f of [WIDE, TALL]) {
				const b = clickPlacedBox(ux, uy, f.w, f.h);
				expect(b.x).toBeGreaterThanOrEqual(0);
				expect(b.y).toBeGreaterThanOrEqual(0);
				expect(b.x + b.w).toBeLessThanOrEqual(1 + 1e-9);
				expect(b.y + b.h).toBeLessThanOrEqual(1 + 1e-9);
			}
		}
	});

	// A fixed UV height reads squat on 16:9 and stretched on a portrait capture, which is why the box is sized in visual pixels.
	it("holds the same screen ratio across frame shapes", () => {
		const wide = clickPlacedBox(0.5, 0.5, WIDE.w, WIDE.h);
		const tall = clickPlacedBox(0.5, 0.5, TALL.w, TALL.h);
		const ratio = (b: { w: number; h: number }, f: { w: number; h: number }) =>
			(b.w * f.w) / (b.h * f.h);
		expect(ratio(wide, WIDE)).toBeCloseTo(ratio(tall, TALL), 6);
	});
});

describe("clickPlacedArrow", () => {
	it("runs down-right from the click at a visual 45 degrees", () => {
		const a = clickPlacedArrow(0.2, 0.2, WIDE.w, WIDE.h);
		expect(a.x2).toBeGreaterThan(a.x1);
		expect(a.y2).toBeGreaterThan(a.y1);
		expect((a.x2 - a.x1) * WIDE.w).toBeCloseTo((a.y2 - a.y1) * WIDE.h, 6);
	});

	it("flips back toward the frame when clicked near the far corner", () => {
		const a = clickPlacedArrow(0.98, 0.98, WIDE.w, WIDE.h);
		expect(a.x2).toBeLessThan(a.x1);
		expect(a.y2).toBeLessThan(a.y1);
		expect(a.x2).toBeGreaterThanOrEqual(0);
		expect(a.y2).toBeGreaterThanOrEqual(0);
	});

	it("keeps both endpoints in frame from any click", () => {
		for (const [ux, uy] of [
			[0, 0],
			[1, 0],
			[0, 1],
			[1, 1],
			[0.5, 0.5],
		]) {
			for (const f of [WIDE, TALL]) {
				const a = clickPlacedArrow(ux, uy, f.w, f.h);
				for (const v of [a.x1, a.y1, a.x2, a.y2]) {
					expect(v).toBeGreaterThanOrEqual(0);
					expect(v).toBeLessThanOrEqual(1);
				}
			}
		}
	});
});

const RAMP = 0.2;

function opacityAt(start: number, end: number, t: number): number {
	return evalOpacity(
		{
			start,
			end,
			rampIn: RAMP,
			rampOut: RAMP,
			easeIn: { x1: 0, y1: 0, x2: 1, y2: 1 },
			easeOut: { x1: 0, y1: 0, x2: 1, y2: 1 },
		} as Parameters<typeof evalOpacity>[0],
		t,
	);
}

describe("placedTimeRange", () => {
	/** The reported bug: a 200 ms fade at the playhead left nothing on screen where it was placed. */
	it("puts the fade before the playhead so the annotation is up where it was placed", () => {
		const { start, end } = placedTimeRange(10, 0, 60, 0.2);

		expect(start).toBeCloseTo(9.8, 6);
		expect(end).toBeCloseTo(11.8, 6);
	});

	it("keeps the start at the playhead when there is no fade to lead in", () => {
		expect(placedTimeRange(10, 0, 60, 0)).toEqual({ start: 10, end: 12 });
	});

	it("cannot start before the trim, where the fade has nowhere to go", () => {
		const { start } = placedTimeRange(5, 5, 60, 0.2);

		expect(start).toBe(5);
	});

	it("takes the last stretch of the clip when the playhead is at the very end", () => {
		expect(placedTimeRange(60, 0, 60, 0.2)).toEqual({ start: 58, end: 60 });
	});

	it("gives the whole clip to an annotation that cannot fit in it", () => {
		expect(placedTimeRange(0.5, 0, 1, 0.2)).toEqual({ start: 0, end: 1 });
	});

	/** The range is a means; being ON SCREEN where it was placed is the point. */
	it("is fully opaque at the playhead it was placed at", () => {
		const at = 10;
		const { start, end } = placedTimeRange(at, 0, 60, RAMP);

		expect(opacityAt(start, end, at)).toBe(1);
	});

	/**
	 * Placed on the last frame there is nowhere to put a fade-out, so it is still
	 * ramping down at the playhead. Nothing can fix that without running past the clip.
	 */
	it("cannot be opaque when placed on the very last frame", () => {
		const { start, end } = placedTimeRange(60, 0, 60, RAMP);

		expect(opacityAt(start, end, 60)).toBe(0);
		expect(opacityAt(start, end, 59)).toBe(1);
	});
});
