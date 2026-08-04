import { describe, expect, it } from "vitest";
import { fadeGainAt } from "./audio-engine";

describe("fadeGainAt", () => {
	// 10s output, 2s fade in, 3s fade out.
	const inD = 2;
	const outD = 3;
	const dur = 10;

	it("ramps in linearly from 0 to 1", () => {
		expect(fadeGainAt(0, inD, outD, dur)).toBeCloseTo(0, 6);
		expect(fadeGainAt(1, inD, outD, dur)).toBeCloseTo(0.5, 6);
		expect(fadeGainAt(2, inD, outD, dur)).toBeCloseTo(1, 6);
	});

	it("holds at unity between the ramps", () => {
		expect(fadeGainAt(5, inD, outD, dur)).toBe(1);
	});

	it("ramps out linearly from 1 to 0", () => {
		expect(fadeGainAt(7, inD, outD, dur)).toBeCloseTo(1, 6); // fade-out begins at 10-3=7
		expect(fadeGainAt(8.5, inD, outD, dur)).toBeCloseTo(0.5, 6);
		expect(fadeGainAt(10, inD, outD, dur)).toBeCloseTo(0, 6);
	});

	it("clamps each fade to half the output duration", () => {
		// 8s fade-in over a 4s clip clamps to 2s, so unity is reached at t=2.
		expect(fadeGainAt(2, 8, 0, 4)).toBeCloseTo(1, 6);
		expect(fadeGainAt(1, 8, 0, 4)).toBeCloseTo(0.5, 6);
	});

	it("is unity with no fades or no duration", () => {
		expect(fadeGainAt(3, 0, 0, 10)).toBe(1);
		expect(fadeGainAt(3, 2, 3, 0)).toBe(1);
	});

	it("never leaves [0,1]", () => {
		for (let t = -1; t <= 11; t += 0.25) {
			const g = fadeGainAt(t, inD, outD, dur);
			expect(g).toBeGreaterThanOrEqual(0);
			expect(g).toBeLessThanOrEqual(1);
		}
	});
});
