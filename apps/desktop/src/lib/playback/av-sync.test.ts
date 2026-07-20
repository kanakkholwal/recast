import { describe, expect, it } from "vitest";
import { AV_RESYNC_THRESHOLD_SEC, resolveAvSync } from "./av-sync";

describe("resolveAvSync", () => {
	const playing = true;

	it("does not resync while paused, however far apart the clocks are", () => {
		const d = resolveAvSync({ videoTime: 10, audioTime: 5, playing: false });
		expect(d.resync).toBe(false);
		expect(d.driftSec).toBe(0);
	});

	it("does not resync when audio has nothing scheduled", () => {
		const d = resolveAvSync({ videoTime: 10, audioTime: null, playing });
		expect(d.resync).toBe(false);
	});

	it("tolerates drift inside the perceptual threshold", () => {
		const d = resolveAvSync({ videoTime: 10.03, audioTime: 10, playing });
		expect(d.resync).toBe(false);
		expect(d.driftSec).toBeCloseTo(0.03, 5);
	});

	it("resyncs onto audio when the picture runs ahead", () => {
		const d = resolveAvSync({ videoTime: 10.2, audioTime: 10, playing });
		expect(d.resync).toBe(true);
		expect(d.target).toBe(10);
		expect(d.driftSec).toBeCloseTo(0.2, 5);
	});

	it("resyncs onto audio when the picture falls behind", () => {
		const d = resolveAvSync({ videoTime: 9.8, audioTime: 10, playing });
		expect(d.resync).toBe(true);
		expect(d.target).toBe(10);
		expect(d.driftSec).toBeCloseTo(-0.2, 5);
	});

	it("targets audio, never a midpoint — audio is the master clock", () => {
		const d = resolveAvSync({ videoTime: 0, audioTime: 30, playing });
		expect(d.target).toBe(30);
	});

	it("treats exactly-at-threshold as in tolerance", () => {
		// Anchored at 0 so the subtraction is exact — `10 + 0.06 - 10` lands a
		// hair above the threshold in floating point and would resync.
		const d = resolveAvSync({
			videoTime: AV_RESYNC_THRESHOLD_SEC,
			audioTime: 0,
			playing,
		});
		expect(d.resync).toBe(false);
	});

	it("resyncs just past the threshold", () => {
		const d = resolveAvSync({
			videoTime: AV_RESYNC_THRESHOLD_SEC * 1.5,
			audioTime: 0,
			playing,
		});
		expect(d.resync).toBe(true);
	});

	it("honors a caller-supplied threshold", () => {
		const d = resolveAvSync({
			videoTime: 10.03,
			audioTime: 10,
			playing,
			thresholdSec: 0.01,
		});
		expect(d.resync).toBe(true);
	});

	it("ignores a non-finite audio clock", () => {
		const d = resolveAvSync({ videoTime: 10, audioTime: Number.NaN, playing });
		expect(d.resync).toBe(false);
	});
});
