import { describe, expect, it } from "vitest";
import { musicFadeFactor } from "$lib/playback/audio-engine";
import { clipDisplayName, clipGain, defaultAudioClip } from "./music";

describe("music clip model", () => {
	it("gives background-music defaults (loops, sits under the voice, starts at 0)", () => {
		const c = defaultAudioClip("id", { kind: "local", path: "/x.mp3" });
		expect(c.startOutputSec).toBe(0);
		expect(c.loop).toBe(true);
		expect(c.gain).toBeLessThan(100); // under the voice
		expect(c.muted).toBe(false);
	});

	it("clipGain applies mute and clamps", () => {
		const c = defaultAudioClip("id", { kind: "local", path: "/x.mp3" });
		expect(clipGain({ ...c, gain: 50 })).toBeCloseTo(0.5, 6);
		expect(clipGain({ ...c, muted: true })).toBe(0);
		expect(clipGain({ ...c, gain: 500 })).toBe(2); // clamp
	});

	it("names a clip from its file basename (both slash styles)", () => {
		expect(clipDisplayName(defaultAudioClip("i", { kind: "local", path: "/a/b/song.mp3" }))).toBe(
			"song.mp3",
		);
		expect(
			clipDisplayName(defaultAudioClip("i", { kind: "local", path: "C:\\music\\track.wav" })),
		).toBe("track.wav");
	});
});

describe("musicFadeFactor (preview ↔ export parity)", () => {
	// 10s clip, 2s in, 3s out. Fades clamp to the play length (NOT half of it),
	// matching the export's per-clip afade.
	it("ramps in, holds, ramps out", () => {
		expect(musicFadeFactor(0, 10, 2, 3)).toBeCloseTo(0, 6);
		expect(musicFadeFactor(1, 10, 2, 3)).toBeCloseTo(0.5, 6);
		expect(musicFadeFactor(2, 10, 2, 3)).toBeCloseTo(1, 6);
		expect(musicFadeFactor(5, 10, 2, 3)).toBe(1);
		expect(musicFadeFactor(7, 10, 2, 3)).toBeCloseTo(1, 6); // fade-out begins at 10-3
		expect(musicFadeFactor(8.5, 10, 2, 3)).toBeCloseTo(0.5, 6);
		expect(musicFadeFactor(10, 10, 2, 3)).toBeCloseTo(0, 6);
	});

	it("clamps a fade to the play length, not half", () => {
		// 8s fade-in over a 4s clip → unity at t=4 (min(8,4)), unlike the master fade.
		expect(musicFadeFactor(4, 4, 8, 0)).toBeCloseTo(1, 6);
		expect(musicFadeFactor(2, 4, 8, 0)).toBeCloseTo(0.5, 6);
	});

	it("stays in [0,1]", () => {
		for (let t = -1; t <= 11; t += 0.5) {
			const f = musicFadeFactor(t, 10, 2, 3);
			expect(f).toBeGreaterThanOrEqual(0);
			expect(f).toBeLessThanOrEqual(1);
		}
	});
});
