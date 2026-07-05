import { describe, expect, it } from "vitest";
import type { TimelineCut } from "$lib/timeline/cuts";
import {
	buildFpsOptions,
	clampSourceFps,
	computeExportDurations,
	computeRemovedDuration,
	nextLoopCount,
} from "./export-dialog.logic";

function cut(start: number, end: number, id = `${start}-${end}`): TimelineCut {
	return { id, start, end, source: "manual" };
}

describe("computeRemovedDuration", () => {
	it("is zero with no cuts", () => {
		expect(computeRemovedDuration([], 0, 10)).toBe(0);
	});

	it("sums disjoint cuts inside the window", () => {
		expect(computeRemovedDuration([cut(1, 2), cut(4, 5)], 0, 10)).toBeCloseTo(2, 5);
	});

	it("clamps cuts to the trim window", () => {
		// [0,3] clamps to [1,3] (trimStart 1), [8,12] clamps to [8,9] (clipEnd 9).
		expect(computeRemovedDuration([cut(0, 3), cut(8, 12)], 1, 9)).toBeCloseTo(3, 5);
	});

	it("merges overlapping cuts so shared time is not double-counted", () => {
		// [1,4] ∪ [3,6] = [1,6] = 5s, not 3 + 3.
		expect(computeRemovedDuration([cut(1, 4), cut(3, 6)], 0, 10)).toBeCloseTo(5, 5);
	});

	it("drops cuts fully outside the window", () => {
		expect(computeRemovedDuration([cut(20, 25)], 0, 10)).toBe(0);
	});
});

describe("buildFpsOptions", () => {
	it("offers only Original when the source is at/below the lowest rate", () => {
		const opts = buildFpsOptions(24);
		expect(opts).toHaveLength(1);
		expect(opts[0]).toEqual({ value: null, label: "Original", desc: "24 fps" });
	});

	it("adds standard rates strictly below the source", () => {
		const opts = buildFpsOptions(60);
		// Original + 30 + 24 (60 is not < 60).
		expect(opts.map((o) => o.value)).toEqual([null, 30, 24]);
		expect(opts[opts.length - 1].desc).toBe("Cinematic");
	});

	it("includes 60 for a high-rate source", () => {
		expect(buildFpsOptions(120).map((o) => o.value)).toEqual([null, 60, 30, 24]);
	});
});

describe("clampSourceFps", () => {
	it("defaults to 60 when unknown and never drops below 1", () => {
		expect(clampSourceFps(undefined)).toBe(60);
		expect(clampSourceFps(null)).toBe(60);
		expect(clampSourceFps(0.2)).toBe(1);
		expect(clampSourceFps(29.97)).toBe(30);
	});
});

describe("computeExportDurations", () => {
	it("derives clip and post-cut output lengths", () => {
		expect(computeExportDurations(10, 2, 3)).toEqual({
			clipDuration: 8,
			outputDuration: 5,
		});
	});

	it("clamps to zero", () => {
		expect(computeExportDurations(2, 5, 0)).toEqual({
			clipDuration: 0,
			outputDuration: 0,
		});
	});
});

describe("nextLoopCount", () => {
	it("cycles 1→5 then wraps to 1", () => {
		expect(nextLoopCount(1)).toBe(2);
		expect(nextLoopCount(4)).toBe(5);
		expect(nextLoopCount(5)).toBe(1);
	});

	it("starts finite counting at 1 from infinite/once", () => {
		expect(nextLoopCount("infinite")).toBe(1);
		expect(nextLoopCount("once")).toBe(1);
	});
});
