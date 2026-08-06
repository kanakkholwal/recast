import { describe, expect, it } from "vitest";
import { keptRegions, planAudioSchedule } from "./audio-schedule";

describe("keptRegions", () => {
	it("returns the whole trim range when there are no cuts", () => {
		expect(keptRegions(0, 10, [])).toEqual([{ start: 0, end: 10 }]);
	});

	it("removes a single interior cut, leaving two regions", () => {
		expect(keptRegions(0, 10, [{ start: 3, end: 5 }])).toEqual([
			{ start: 0, end: 3 },
			{ start: 5, end: 10 },
		]);
	});

	it("clips cuts to the trim range", () => {
		expect(
			keptRegions(2, 8, [
				{ start: 0, end: 3 },
				{ start: 7, end: 12 },
			]),
		).toEqual([{ start: 3, end: 7 }]);
	});

	it("merges overlapping/adjacent cuts", () => {
		expect(
			keptRegions(0, 10, [
				{ start: 3, end: 5 },
				{ start: 4, end: 6 },
			]),
		).toEqual([
			{ start: 0, end: 3 },
			{ start: 6, end: 10 },
		]);
	});

	it("returns [] when the whole range is cut", () => {
		expect(keptRegions(0, 10, [{ start: 0, end: 10 }])).toEqual([]);
		expect(keptRegions(5, 5, [])).toEqual([]);
	});
});

describe("planAudioSchedule", () => {
	// Two kept regions: [0,3] orig (output 0–3) and [5,10] orig (output 3–8).
	const regions = [
		{ start: 0, end: 3 },
		{ start: 5, end: 10 },
	];

	it("schedules every region from the start of playback", () => {
		const plan = planAudioSchedule(regions, 0);
		expect(plan).toEqual([
			{ whenDelay: 0, bufferOffset: 0, duration: 3, rate: 1, outStart: 0, outEnd: 3 },
			{ whenDelay: 3, bufferOffset: 5, duration: 5, rate: 1, outStart: 3, outEnd: 8 },
		]);
	});

	it("starts mid-region immediately at the right buffer offset", () => {
		// Output time 4 is 1s into the second region (orig 5..10) → buffer offset 6.
		const plan = planAudioSchedule(regions, 4);
		expect(plan).toEqual([
			{ whenDelay: 0, bufferOffset: 6, duration: 4, rate: 1, outStart: 3, outEnd: 8 },
		]);
	});

	it("skips regions fully behind the playhead", () => {
		const plan = planAudioSchedule(regions, 3.5);
		expect(plan).toHaveLength(1);
		expect(plan[0].outStart).toBe(3);
	});

	it("maps output time across a cut to the post-cut buffer offset", () => {
		// Output 3 is exactly the cut boundary → second region starts now at orig 5.
		const plan = planAudioSchedule(regions, 3);
		expect(plan[0]).toMatchObject({ whenDelay: 0, bufferOffset: 5, duration: 5 });
	});

	it("returns nothing once playback is past the end", () => {
		expect(planAudioSchedule(regions, 8)).toEqual([]);
	});
});

describe("planAudioSchedule with per-segment speed", () => {
	it("a 2x region occupies half the output and plays at rate 2", () => {
		// [0,4] source at 2x → output 0–2; whole 4s of source plays at rate 2.
		const plan = planAudioSchedule([{ start: 0, end: 4, speed: 2 }], 0);
		expect(plan).toEqual([
			{ whenDelay: 0, bufferOffset: 0, duration: 4, rate: 2, outStart: 0, outEnd: 2 },
		]);
	});

	it("warps later regions' output positions by upstream speeds", () => {
		// [0,4]@1 (output 0–4) then [4,8]@2 (output 4–6).
		const plan = planAudioSchedule(
			[
				{ start: 0, end: 4, speed: 1 },
				{ start: 4, end: 8, speed: 2 },
			],
			0,
		);
		expect(plan).toEqual([
			{ whenDelay: 0, bufferOffset: 0, duration: 4, rate: 1, outStart: 0, outEnd: 4 },
			{ whenDelay: 4, bufferOffset: 4, duration: 4, rate: 2, outStart: 4, outEnd: 6 },
		]);
	});

	it("starts mid sped-up region at the speed-scaled buffer offset", () => {
		// [0,4]@2 → output 0–2. From output 1 (half-way) → 2s of source consumed.
		const plan = planAudioSchedule([{ start: 0, end: 4, speed: 2 }], 1);
		expect(plan).toEqual([
			{ whenDelay: 0, bufferOffset: 2, duration: 2, rate: 2, outStart: 0, outEnd: 2 },
		]);
	});

	it("treats absent/zero speed as 1x", () => {
		expect(planAudioSchedule([{ start: 0, end: 2, speed: 0 }], 0)[0].rate).toBe(1);
		expect(planAudioSchedule([{ start: 0, end: 2 }], 0)[0].rate).toBe(1);
	});
});
