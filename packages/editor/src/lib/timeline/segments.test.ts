import { describe, expect, it } from "vitest";
import type { TimelineCut } from "./cuts";
import {
	deriveSeams,
	deriveSegments,
	planDeleteSegment,
	planSplit,
	segmentAt,
	type ClipShape,
} from "./segments";

function cut(start: number, end: number, id = `${start}-${end}`): TimelineCut {
	return { id, start, end, source: "manual" };
}

function shape(partial: Partial<ClipShape> = {}): ClipShape {
	return {
		trimStart: 0,
		trimEnd: 10,
		cuts: [],
		splitPoints: [],
		...partial,
	};
}

describe("deriveSegments", () => {
	it("returns a single segment spanning the trim when there are no cuts or splits", () => {
		expect(deriveSegments(shape())).toEqual([{ start: 0, end: 10, index: 0 }]);
	});

	it("respects the trim bounds", () => {
		expect(deriveSegments(shape({ trimStart: 2, trimEnd: 8 }))).toEqual([
			{ start: 2, end: 8, index: 0 },
		]);
	});

	it("returns nothing for an empty (or inverted) trim", () => {
		expect(deriveSegments(shape({ trimStart: 5, trimEnd: 5 }))).toEqual([]);
		expect(deriveSegments(shape({ trimStart: 6, trimEnd: 5 }))).toEqual([]);
	});

	it("slices into two segments at a single interior split", () => {
		expect(deriveSegments(shape({ splitPoints: [4] }))).toEqual([
			{ start: 0, end: 4, index: 0 },
			{ start: 4, end: 10, index: 1 },
		]);
	});

	it("orders multiple splits regardless of input order", () => {
		expect(deriveSegments(shape({ splitPoints: [7, 2, 5] }))).toEqual([
			{ start: 0, end: 2, index: 0 },
			{ start: 2, end: 5, index: 1 },
			{ start: 5, end: 7, index: 2 },
			{ start: 7, end: 10, index: 3 },
		]);
	});

	it("removes a cut, producing two kept segments around the gap", () => {
		expect(deriveSegments(shape({ cuts: [cut(4, 6)] }))).toEqual([
			{ start: 0, end: 4, index: 0 },
			{ start: 6, end: 10, index: 1 },
		]);
	});

	it("drops a leading cut that touches trimStart", () => {
		expect(deriveSegments(shape({ trimStart: 0, cuts: [cut(0, 3)] }))).toEqual([
			{ start: 3, end: 10, index: 0 },
		]);
	});

	it("ignores split points that land inside a cut", () => {
		// 5 is inside the cut [4,6] → it cannot subdivide anything.
		expect(deriveSegments(shape({ cuts: [cut(4, 6)], splitPoints: [5] }))).toEqual([
			{ start: 0, end: 4, index: 0 },
			{ start: 6, end: 10, index: 1 },
		]);
	});

	it("ignores split points outside the trim", () => {
		expect(deriveSegments(shape({ trimStart: 2, trimEnd: 8, splitPoints: [1, 9, 5] }))).toEqual([
			{ start: 2, end: 5, index: 0 },
			{ start: 5, end: 8, index: 1 },
		]);
	});

	it("combines cuts and splits into a correctly indexed list", () => {
		// kept = [0,4] and [6,10]; split at 2 (in first) and 8 (in second).
		expect(deriveSegments(shape({ cuts: [cut(4, 6)], splitPoints: [2, 8] }))).toEqual([
			{ start: 0, end: 2, index: 0 },
			{ start: 2, end: 4, index: 1 },
			{ start: 6, end: 8, index: 2 },
			{ start: 8, end: 10, index: 3 },
		]);
	});

	it("merges overlapping cuts before deriving", () => {
		expect(deriveSegments(shape({ cuts: [cut(3, 5), cut(4, 7)] }))).toEqual([
			{ start: 0, end: 3, index: 0 },
			{ start: 7, end: 10, index: 1 },
		]);
	});
});

describe("segmentAt", () => {
	const segments = deriveSegments(shape({ splitPoints: [4] })); // [0,4],[4,10]

	it("finds the segment containing an interior time", () => {
		expect(segmentAt(segments, 2)?.index).toBe(0);
		expect(segmentAt(segments, 7)?.index).toBe(1);
	});

	it("assigns an exact internal boundary to the right-hand segment", () => {
		expect(segmentAt(segments, 4)?.index).toBe(1);
	});

	it("treats the clip start as belonging to the first segment", () => {
		expect(segmentAt(segments, 0)?.index).toBe(0);
	});

	it("treats the very end as the last segment", () => {
		expect(segmentAt(segments, 10)?.index).toBe(1);
	});

	it("returns null in a trimmed-off region", () => {
		const trimmed = deriveSegments(shape({ trimStart: 2, trimEnd: 8 }));
		expect(segmentAt(trimmed, 1)).toBeNull();
		expect(segmentAt(trimmed, 9)).toBeNull();
	});

	it("returns null inside a cut", () => {
		const withCut = deriveSegments(shape({ cuts: [cut(4, 6)] }));
		expect(segmentAt(withCut, 5)).toBeNull();
	});
});

describe("planSplit", () => {
	it("adds a split at a valid interior time, kept sorted", () => {
		expect(planSplit(6, shape({ splitPoints: [3] }))).toEqual([3, 6]);
	});

	it("returns null at or beyond the clip edges", () => {
		expect(planSplit(0, shape())).toBeNull();
		expect(planSplit(10, shape())).toBeNull();
		expect(planSplit(2, shape({ trimStart: 2 }))).toBeNull();
	});

	it("returns null when a split already exists there", () => {
		expect(planSplit(4, shape({ splitPoints: [4] }))).toBeNull();
	});

	it("returns null inside or on the edge of a cut", () => {
		expect(planSplit(5, shape({ cuts: [cut(4, 6)] }))).toBeNull();
		expect(planSplit(4, shape({ cuts: [cut(4, 6)] }))).toBeNull();
	});
});

describe("planDeleteSegment", () => {
	it("turns the segment range into a cut", () => {
		const seg = { start: 4, end: 6, index: 1 };
		expect(planDeleteSegment(seg, []).cut).toEqual({ start: 4, end: 6 });
	});

	it("prunes split points inside or on the edges of the deleted range", () => {
		const seg = { start: 4, end: 6, index: 1 };
		const { splitPoints } = planDeleteSegment(seg, [2, 4, 5, 6, 8]);
		expect(splitPoints).toEqual([2, 8]);
	});

	it("keeps split points outside the deleted range untouched", () => {
		const seg = { start: 4, end: 6, index: 1 };
		expect(planDeleteSegment(seg, [1, 9]).splitPoints).toEqual([1, 9]);
	});
});

describe("deriveSeams", () => {
	it("returns no seams for a single segment", () => {
		const segs = deriveSegments(shape());
		expect(deriveSeams(segs)).toEqual([]);
	});

	it("emits one seam per ripple-removed gap, with the removed amount", () => {
		// Trim [0,10] with [3,5] removed → segments [0,3] and [5,10], one seam.
		const segs = deriveSegments(shape({ cuts: [cut(3, 5)] }));
		expect(deriveSeams(segs)).toEqual([{ gapStart: 3, gapEnd: 5, removed: 2 }]);
	});

	it("emits a seam per cut when several are removed", () => {
		const segs = deriveSegments(shape({ trimEnd: 12, cuts: [cut(2, 4), cut(7, 8)] }));
		expect(deriveSeams(segs)).toEqual([
			{ gapStart: 2, gapEnd: 4, removed: 2 },
			{ gapStart: 7, gapEnd: 8, removed: 1 },
		]);
	});

	it("does NOT emit a seam for a split (segments that merely touch)", () => {
		// A split divides the clip into two adjacent segments with no gap.
		const segs = deriveSegments(shape({ splitPoints: [5] }));
		expect(segs).toHaveLength(2);
		expect(deriveSeams(segs)).toEqual([]);
	});

	it("seam removed-amount equals the gap between adjacent segments", () => {
		const segs = deriveSegments(shape({ trimEnd: 20, cuts: [cut(6, 9)] }));
		const seams = deriveSeams(segs);
		expect(seams).toHaveLength(1);
		expect(seams[0].removed).toBeCloseTo(seams[0].gapEnd - seams[0].gapStart);
	});
});
