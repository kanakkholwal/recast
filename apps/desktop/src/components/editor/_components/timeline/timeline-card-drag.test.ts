import { describe, expect, it } from "vitest";
import {
	computeCardMove,
	computeCardResize,
	dragEngaged,
	DRAG_THRESHOLD_PX,
	PRECISION_SCALE,
	type CardDragGeometry,
} from "./timeline-card-drag.logic";
import type { SnapTarget } from "./timeline-snap";

const FPS = 60;
const PPS = 100;

// An uncollapsed axis: original time maps straight to output pixels, so the
// tests read in seconds and only exercise the drag maths.
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
	// The bug this exists for: the first pointermove of a click-to-select wrote
	// a new start/end and pushed an undo entry, so selecting a card nudged it.
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

	// Precision is applied to pointer travel, so re-seeding the anchor when the
	// modifier flips mid-drag leaves the card exactly where it was.
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

	// Dropping the targets is how the components bypass magnetism mid-drag; the
	// result must still land on the frame grid, never sub-frame.
	it("falls through to the frame grid with no targets", () => {
		const result = computeCardMove(geometry({ clientX: 150, startClientX: 0, snapTargets: [] }));
		expect(result.guide).toBeNull();
		expect(result.start * FPS).toBeCloseTo(Math.round(result.start * FPS));
	});
});
