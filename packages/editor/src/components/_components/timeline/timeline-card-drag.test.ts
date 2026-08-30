import { describe, expect, it } from "vitest";
import {
	type CardDragGeometry,
	computeCardMove,
	computeCardResize,
	DRAG_THRESHOLD_PX,
	dragEngaged,
	PRECISION_SCALE,
} from "./timeline-card-drag.logic";
import type { SnapTarget } from "./timeline-snap";

const FPS = 60;
const PPS = 100;

// An uncollapsed axis: original time maps straight to output pixels, so the tests read in seconds.
function geometry(over: Partial<CardDragGeometry> = {}): CardDragGeometry {
	return {
		origin: { start: 2, end: 4 },
		clientX: 0,
		startClientX: 0,
		xOf: (t) => t * PPS,
		tOf: (x) => x / PPS,
		snapTargets: [],
		tolerance: 6 / PPS,
		fps: FPS,
		duration: 20,
		...over,
	};
}

describe("dragEngaged", () => {
	// The bug: the first pointermove of a click-to-select wrote a new start and pushed undo, so selecting nudged the card.
	it("ignores travel below the threshold", () => {
		expect(dragEngaged(100, 100)).toBe(false);
		expect(dragEngaged(100 + DRAG_THRESHOLD_PX - 1, 100)).toBe(false);
	});

	it("engages at the threshold, in either direction", () => {
		expect(dragEngaged(100 + DRAG_THRESHOLD_PX, 100)).toBe(true);
		expect(dragEngaged(100 - DRAG_THRESHOLD_PX, 100)).toBe(true);
	});
});

describe("precision scaling", () => {
	it("moves the card one-for-one with the pointer by default", () => {
		const result = computeCardMove(geometry({ clientX: 150, startClientX: 0 }));
		expect(result.start).toBeCloseTo(3.5);
		expect(result.end).toBeCloseTo(5.5);
	});

	it("damps the same travel when precision is on", () => {
		const result = computeCardMove(
			geometry({ clientX: 150, startClientX: 0, scale: PRECISION_SCALE }),
		);
		// Quantised to the frame grid, like every write the timeline makes.
		const onGrid = Math.round((2 + 1.5 * PRECISION_SCALE) * FPS) / FPS;
		expect(result.start).toBeCloseTo(onGrid);
		expect(result.end - result.start).toBeCloseTo(2);
	});

	it("damps a resize too, holding the opposite edge", () => {
		const result = computeCardResize({
			...geometry({ clientX: 100, startClientX: 0, scale: PRECISION_SCALE }),
			edge: "end",
			minDuration: 0.1,
		});
		expect(result.start).toBeCloseTo(2);
		expect(result.end).toBeCloseTo(4 + 1 * PRECISION_SCALE);
	});

	// Precision applies to pointer travel, so re-seeding the anchor on a mid-drag flip leaves the card where it was.
	it("is a no-op at zero travel, whatever the scale", () => {
		for (const scale of [1, PRECISION_SCALE]) {
			const result = computeCardMove(geometry({ clientX: 40, startClientX: 40, scale }));
			expect(result.start, `scale ${scale}`).toBeCloseTo(2);
			expect(result.end, `scale ${scale}`).toBeCloseTo(4);
		}
	});
});

describe("snap bypass", () => {
	const playhead: SnapTarget[] = [{ time: 3.52, kind: "playhead" }];

	it("locks to a target within tolerance", () => {
		const result = computeCardMove(
			geometry({ clientX: 150, startClientX: 0, snapTargets: playhead }),
		);
		expect(result.start).toBeCloseTo(3.52);
		expect(result.guide?.kind).toBe("playhead");
	});

	// Dropping the targets is how components bypass magnetism; the result must still land on the frame grid.
	it("falls through to the frame grid with no targets", () => {
		const result = computeCardMove(geometry({ clientX: 150, startClientX: 0, snapTargets: [] }));
		expect(result.guide).toBeNull();
		expect(result.start * FPS).toBeCloseTo(Math.round(result.start * FPS));
	});
});
