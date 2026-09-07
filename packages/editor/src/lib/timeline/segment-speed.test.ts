import { describe, expect, it } from "vitest";
import speedParity from "./__fixtures__/speed-parity.json";
import {
	buildSpeedOf,
	clampSpeed,
	MAX_SEGMENT_SPEED,
	MIN_SEGMENT_SPEED,
	pruneSegmentSpeeds,
	type SegmentSpeed,
	segmentSpeedAt,
	segmentSpeedAtTime,
	setSegmentSpeed,
} from "./segment-speed";
import { deriveSegments, type Segment } from "./segments";
import { timeMapFromSegments } from "./time-map";

function seg(start: number, end: number, index: number): Segment {
	return { start, end, index };
}

describe("clampSpeed", () => {
	it("keeps in-range values", () => {
		expect(clampSpeed(2)).toBe(2);
	});
	it("clamps to the supported bounds", () => {
		expect(clampSpeed(99)).toBe(MAX_SEGMENT_SPEED);
		expect(clampSpeed(0.01)).toBe(MIN_SEGMENT_SPEED);
	});
	it("resets a non-positive or non-finite value to 1", () => {
		expect(clampSpeed(0)).toBe(1);
		expect(clampSpeed(-3)).toBe(1);
		expect(clampSpeed(Number.NaN)).toBe(1);
	});
});

describe("segmentSpeedAt", () => {
	const overrides: SegmentSpeed[] = [{ start: 4, speed: 2 }];
	it("returns the matching speed within tolerance", () => {
		expect(segmentSpeedAt(overrides, 4)).toBe(2);
		expect(segmentSpeedAt(overrides, 4.00005)).toBe(2);
	});
	it("returns 1 for an unmatched anchor", () => {
		expect(segmentSpeedAt(overrides, 5)).toBe(1);
		expect(segmentSpeedAt([], 4)).toBe(1);
	});
});

describe("segmentSpeedAtTime", () => {
	// Two kept segments [0,4] and [4,10]; only the second is sped up.
	const segments = [seg(0, 4, 0), seg(4, 10, 1)];
	const overrides: SegmentSpeed[] = [{ start: 4, speed: 2 }];

	it("returns the speed of the segment containing the time", () => {
		expect(segmentSpeedAtTime(segments, overrides, 1)).toBe(1);
		expect(segmentSpeedAtTime(segments, overrides, 7)).toBe(2);
	});
	it("forward-biases a seam onto the following segment", () => {
		expect(segmentSpeedAtTime(segments, overrides, 4)).toBe(2);
	});
	it("holds the last segment's speed at/after the final frame", () => {
		expect(segmentSpeedAtTime(segments, overrides, 10)).toBe(2);
		expect(segmentSpeedAtTime(segments, overrides, 99)).toBe(2);
	});
	it("defaults to 1 with no segments", () => {
		expect(segmentSpeedAtTime([], overrides, 3)).toBe(1);
	});
});

describe("setSegmentSpeed", () => {
	it("inserts a new override, sorted by start", () => {
		const out = setSegmentSpeed([{ start: 6, speed: 2 }], 2, 0.5);
		expect(out).toEqual([
			{ start: 2, speed: 0.5 },
			{ start: 6, speed: 2 },
		]);
	});
	it("replaces an existing anchor in place", () => {
		const out = setSegmentSpeed([{ start: 2, speed: 2 }], 2, 3);
		expect(out).toEqual([{ start: 2, speed: 3 }]);
	});
	it("removes the entry when set back to 1 (stays sparse)", () => {
		expect(setSegmentSpeed([{ start: 2, speed: 2 }], 2, 1)).toEqual([]);
	});
	it("clamps the stored value", () => {
		expect(setSegmentSpeed([], 2, 99)).toEqual([{ start: 2, speed: MAX_SEGMENT_SPEED }]);
	});
	it("does not mutate the input", () => {
		const input: SegmentSpeed[] = [{ start: 2, speed: 2 }];
		setSegmentSpeed(input, 2, 3);
		expect(input).toEqual([{ start: 2, speed: 2 }]);
	});
});

describe("pruneSegmentSpeeds", () => {
	it("keeps anchors that still match a segment start", () => {
		const segs = [seg(2, 4, 0), seg(4, 8, 1)];
		const overrides: SegmentSpeed[] = [
			{ start: 2, speed: 2 },
			{ start: 9, speed: 0.5 },
		];
		expect(pruneSegmentSpeeds(overrides, segs)).toEqual([{ start: 2, speed: 2 }]);
	});
	it("returns empty for empty input", () => {
		expect(pruneSegmentSpeeds([], [seg(0, 1, 0)])).toEqual([]);
	});
});

describe("speed parity (shared fixtures with Rust export)", () => {
	// The Rust export test loads these verbatim: the playback map's output duration must equal the export's warped duration.
	for (const c of speedParity.cases) {
		it(`warped duration: ${c.name}`, () => {
			const segments = deriveSegments({
				trimStart: 0,
				trimEnd: c.trimEnd,
				cuts: c.cuts.map(([start, end], i) => ({
					id: `fx-${i}`,
					start,
					end,
					source: "manual" as const,
				})),
				splitPoints: c.splitPoints,
			});
			const overrides = c.segmentSpeeds.map(([start, speed]) => ({ start, speed }));
			const map = timeMapFromSegments(segments, buildSpeedOf(segments, overrides));
			expect(map.outputDuration).toBeCloseTo(c.expectedOutputDuration, 6);
		});
	}
});

describe("buildSpeedOf", () => {
	const segs = [seg(2, 4, 0), seg(4, 8, 1)];
	const overrides: SegmentSpeed[] = [{ start: 4, speed: 2 }];
	it("maps a segment index to its anchored speed", () => {
		const speedOf = buildSpeedOf(segs, overrides);
		expect(speedOf(0)).toBe(1);
		expect(speedOf(1)).toBe(2);
	});
	it("returns 1 for an out-of-range index", () => {
		expect(buildSpeedOf(segs, overrides)(9)).toBe(1);
	});
});
