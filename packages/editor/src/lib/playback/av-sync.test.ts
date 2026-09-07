import { describe, expect, it } from "vitest";
import {
	AUDIO_STALL_LIMIT_SEC,
	AudioStallMonitor,
	AV_RESYNC_THRESHOLD_SEC,
	resolveAvSync,
} from "./av-sync";

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
		// Anchored at 0 so the subtraction is exact: 10 + 0.06 - 10 lands a hair above the threshold and would resync.
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

	it("stops mastering on a stalled audio clock instead of pinning the picture", () => {
		// A suspended AudioContext freezes currentTime, which without this drags the picture back every frame: a total freeze.
		const d = resolveAvSync({
			videoTime: 30,
			audioTime: 10,
			playing,
			audioStalledSec: AUDIO_STALL_LIMIT_SEC + 0.1,
		});
		expect(d.resync).toBe(false);
		expect(d.audioStalled).toBe(true);
		expect(d.driftSec).toBeCloseTo(20, 5);
	});

	it("still resyncs while the stall is within tolerance", () => {
		const d = resolveAvSync({
			videoTime: 10.2,
			audioTime: 10,
			playing,
			audioStalledSec: AUDIO_STALL_LIMIT_SEC / 2,
		});
		expect(d.resync).toBe(true);
		expect(d.audioStalled).toBe(false);
	});
});

describe("AudioStallMonitor", () => {
	it("reports no stall while the clock advances", () => {
		const m = new AudioStallMonitor();
		expect(m.observe(1, true, 0)).toBe(0);
		expect(m.observe(1.5, true, 500)).toBe(0);
		expect(m.observe(2, true, 1000)).toBe(0);
	});

	it("accumulates stall time while the clock sits still", () => {
		const m = new AudioStallMonitor();
		m.observe(5, true, 0);
		expect(m.observe(5, true, 400)).toBeCloseTo(0.4, 5);
		expect(m.observe(5, true, 1200)).toBeCloseTo(1.2, 5);
	});

	it("clears the stall as soon as the clock moves again", () => {
		const m = new AudioStallMonitor();
		m.observe(5, true, 0);
		expect(m.observe(5, true, 900)).toBeCloseTo(0.9, 5);
		expect(m.observe(5.1, true, 1000)).toBe(0);
	});

	it("does not accrue stall time while paused or unscheduled", () => {
		const m = new AudioStallMonitor();
		m.observe(5, true, 0);
		expect(m.observe(5, false, 900)).toBe(0);
		expect(m.observe(null, true, 1800)).toBe(0);
	});
});
