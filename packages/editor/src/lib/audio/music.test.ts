import { describe, expect, it } from "vitest";
import { musicFadeFactor } from "../playback/audio-engine";
import {
	type AudioClip,
	clipDisplayName,
	clipEndSec,
	clipGain,
	clipPlaySec,
	collectCredits,
	defaultAudioClip,
	isVoiceClip,
	moveClip,
	splitClip,
	trimClipLeft,
	trimClipRight,
	voiceClip,
} from "./music";

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

describe("clip timeline editing (move / trim / split)", () => {
	// A 20s output. A concrete 4s clip starting at 5s (source offset 1s), no loop.
	function clip(over: Partial<AudioClip> = {}): AudioClip {
		return {
			...defaultAudioClip("c", { kind: "local", path: "/x.mp3" }),
			startOutputSec: 5,
			offsetSec: 1,
			durationSec: 4,
			loop: false,
			...over,
		};
	}
	const OUT = 20;

	it("clipPlaySec/clipEndSec: explicit duration, else fill to output end", () => {
		expect(clipPlaySec(clip(), OUT)).toBe(4);
		expect(clipEndSec(clip(), OUT)).toBe(9);
		const fill = clip({ durationSec: 0, startOutputSec: 8 });
		expect(clipPlaySec(fill, OUT)).toBe(12); // 20 - 8
	});

	it("move materializes a fill clip's length and clamps in-bounds", () => {
		const moved = moveClip(clip(), 12, OUT);
		expect(moved.startOutputSec).toBe(12);
		expect(moved.durationSec).toBe(4); // length preserved
		// Can't push a 4s clip past the 20s end: start clamps to 16.
		expect(moveClip(clip(), 100, OUT).startOutputSec).toBe(16);
		expect(moveClip(clip(), -5, OUT).startOutputSec).toBe(0);
	});

	it("trim right sets duration and clamps to [min, output end]", () => {
		expect(trimClipRight(clip(), 11, OUT).durationSec).toBe(6); // end 11 - start 5
		expect(trimClipRight(clip(), 100, OUT).durationSec).toBe(15); // clamp to output end
		expect(trimClipRight(clip(), 5, OUT).durationSec).toBeCloseTo(0.1, 6); // min
	});

	it("trim left keeps the end fixed; non-loop advances offset, loop keeps it", () => {
		const t = trimClipLeft(clip(), 7, OUT); // end stays 9
		expect(t.startOutputSec).toBe(7);
		expect(t.durationSec).toBe(2);
		expect(t.offsetSec).toBe(3); // 1 + (7 - 5)
		// Looping clip keeps its source offset (loop restarts, no silent tail).
		expect(trimClipLeft(clip({ loop: true }), 7, OUT).offsetSec).toBe(1);
		// Offset never goes negative when trimming the edge earlier.
		expect(trimClipLeft(clip({ startOutputSec: 5, offsetSec: 1 }), 3, OUT).offsetSec).toBe(0);
	});

	it("split cuts a clip in two at an output time, moving fades off the seam", () => {
		const [l, r] = splitClip(clip(), 7, OUT, "new")!;
		expect(l.id).toBe("c");
		expect(l.startOutputSec).toBe(5);
		expect(l.durationSec).toBe(2);
		expect(l.fadeOut).toBe(0); // seam has no fade
		expect(r.id).toBe("new");
		expect(r.startOutputSec).toBe(7);
		expect(r.durationSec).toBe(2);
		expect(r.offsetSec).toBe(3); // non-loop → source continues (1 + 2)
		expect(r.fadeIn).toBe(0);
	});

	it("split keeps a looping clip's offset and rejects out-of-range points", () => {
		expect(splitClip(clip({ loop: true }), 7, OUT, "n")![1].offsetSec).toBe(1);
		expect(splitClip(clip(), 5.05, OUT, "n")).toBeNull(); // too close to start
		expect(splitClip(clip(), 12, OUT, "n")).toBeNull(); // past the clip end
	});
});

describe("voice clips (detached recording audio)", () => {
	it("voiceClip is a linear, unity-gain, non-looping clip flagged as voice", () => {
		const v = voiceClip("v", "/rec/system.wav", { offsetSec: 2, gain: 80 });
		expect(v.role).toBe("voice");
		expect(isVoiceClip(v)).toBe(true);
		expect(v.loop).toBe(false);
		expect(v.gain).toBe(80); // override wins
		expect(v.offsetSec).toBe(2);
		expect(v.fadeIn).toBe(0);
	});

	it("music clips (incl. legacy role-less) are not voice", () => {
		expect(isVoiceClip(defaultAudioClip("m", { kind: "local", path: "/x.mp3" }))).toBe(false);
		const legacy = { ...defaultAudioClip("l", { kind: "local", path: "/x.mp3" }) };
		delete (legacy as { role?: unknown }).role;
		expect(isVoiceClip(legacy as AudioClip)).toBe(false);
	});
});

describe("collectCredits", () => {
	function provider(id: string, attribution?: string, license?: string): AudioClip {
		return {
			...defaultAudioClip(id, {
				kind: "provider",
				providerId: "jamendo",
				trackId: id,
				assetPath: `/${id}.mp3`,
				attribution,
				license,
			}),
		};
	}

	it("credits provider clips and carries the license url", () => {
		const credits = collectCredits([
			provider("1", '"Sunrise" by Nova (Jamendo)', "https://cc/by/4.0"),
		]);
		expect(credits).toHaveLength(1);
		expect(credits[0].attribution).toContain("Sunrise");
		expect(credits[0].license).toBe("https://cc/by/4.0");
	});

	it("skips local imports (no attribution needed)", () => {
		expect(collectCredits([defaultAudioClip("l", { kind: "local", path: "/x.mp3" })])).toEqual([]);
	});

	it("dedupes the same attribution line", () => {
		const line = '"Sunrise" by Nova (Jamendo)';
		expect(collectCredits([provider("1", line), provider("2", line)])).toHaveLength(1);
	});

	it("drops blank/whitespace attribution and normalizes license to null", () => {
		expect(collectCredits([provider("1", "   ")])).toEqual([]);
		expect(collectCredits([provider("1", "Track", "   ")])[0].license).toBeNull();
	});
});
