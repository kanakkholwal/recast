import { describe, expect, it } from "vitest";
import { scaleTranscript, transcriptTimeScale } from "./normalize";
import type { Transcript } from "../wire-types";

function tx(): Transcript {
	return {
		segments: [
			{
				id: "s1",
				start: 10,
				end: 20,
				text: "hi there",
				words: [
					{ start: 10, end: 11, text: "hi" },
					{ start: 11, end: 20, text: "there" },
				],
			},
		],
	} as unknown as Transcript;
}

describe("transcriptTimeScale", () => {
	it("returns video/audio when within the trust window", () => {
		// 147.95 / 148.51 ≈ 0.99623 (the measured CFR gap).
		expect(transcriptTimeScale(147.95, 148.51)).toBeCloseTo(0.99623, 4);
	});

	it("falls back to 1 when a duration is missing or the gap is implausible", () => {
		expect(transcriptTimeScale(null, 148.51)).toBe(1);
		expect(transcriptTimeScale(147.95, 0)).toBe(1);
		expect(transcriptTimeScale(100, 148.51)).toBe(1); // >5% gap → distrust the probe
	});
});

describe("scaleTranscript", () => {
	it("multiplies every segment + word time by the scale", () => {
		const out = scaleTranscript(tx(), 0.5)!;
		expect(out.segments[0].start).toBe(5);
		expect(out.segments[0].end).toBe(10);
		expect(out.segments[0].words.map((w) => [w.start, w.end])).toEqual([
			[5, 5.5],
			[5.5, 10],
		]);
	});

	it("returns the SAME object for an identity scale (referential stability)", () => {
		const t = tx();
		expect(scaleTranscript(t, 1)).toBe(t);
		expect(scaleTranscript(null, 0.9)).toBeNull();
	});
});
