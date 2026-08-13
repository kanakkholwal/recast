import { describe, expect, it } from "vitest";
import { fadeGainAt, MUSIC_BUFFER_MAX_SEC, musicPlaybackMode } from "./audio-engine";

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

describe("musicPlaybackMode", () => {
	it("buffers short beds and streams long imports", () => {
		expect(musicPlaybackMode(30)).toBe("buffer");
		expect(musicPlaybackMode(MUSIC_BUFFER_MAX_SEC)).toBe("buffer");
		expect(musicPlaybackMode(MUSIC_BUFFER_MAX_SEC + 0.1)).toBe("stream");
		// A 30-min stereo import decoded to 691 MB and stayed resident.
		expect(musicPlaybackMode(1800)).toBe("stream");
	});

	it("falls back to buffering when the duration is unknown", () => {
		// Metadata probes fail outside a browser and on some containers; the
		// buffered path is the one every other test pins.
		expect(musicPlaybackMode(Number.NaN)).toBe("buffer");
		expect(musicPlaybackMode(0)).toBe("buffer");
		expect(musicPlaybackMode(Number.POSITIVE_INFINITY)).toBe("buffer");
	});
});
