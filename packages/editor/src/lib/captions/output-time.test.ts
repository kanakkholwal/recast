import { describe, expect, it } from "vitest";
import type { Transcript, TranscriptSegment, TranscriptWord } from "../wire-types";
import { buildTimeMap } from "../timeline/time-map";
import { toOutputTimeTranscript } from "./output-time";

function word(start: number, end: number, text: string): TranscriptWord {
	return { start, end, text };
}

function seg(id: string, start: number, end: number, words: TranscriptWord[]): TranscriptSegment {
	return { id, start, end, text: words.map((w) => w.text).join(" "), words };
}

function transcript(segments: TranscriptSegment[]): Transcript {
	return { segments } as Transcript;
}

/** Kept [0,12] and [16,24] — i.e. a cut removing [12,16). */
const MAP = buildTimeMap([
	{ origStart: 0, origEnd: 12, speed: 1 },
	{ origStart: 16, origEnd: 24, speed: 1 },
]);

describe("toOutputTimeTranscript", () => {
	it("splits a cue the cut broke in half instead of stretching it across the seam", () => {
		const src = transcript([
			seg("a", 10, 18, [
				word(10, 11, "before"),
				word(11, 12, "seam"),
				word(16, 17, "after"),
				word(17, 18, "seam"),
			]),
		]);

		const out = toOutputTimeTranscript(MAP, src);

		expect(out.segments).toHaveLength(2);
		// Left half stays where it was; right half lands at the seam (output 12).
		expect(out.segments[0]).toMatchObject({ start: 10, end: 12, text: "before seam" });
		expect(out.segments[1]).toMatchObject({ start: 12, end: 14, text: "after seam" });
		// Each piece is its own cue, so ids can't collide in the SRT/VTT.
		expect(out.segments[0].id).not.toEqual(out.segments[1].id);
	});

	it("drops words spoken inside the cut rather than emitting them at zero length", () => {
		const src = transcript([
			seg("a", 10, 18, [
				word(10, 11, "kept"),
				word(13, 15, "removed"), // wholly inside the cut
				word(16, 17, "kept2"),
			]),
		]);

		const out = toOutputTimeTranscript(MAP, src);

		const texts = out.segments.flatMap((s) => s.words.map((w) => w.text));
		expect(texts).toEqual(["kept", "kept2"]);
		for (const s of out.segments) {
			for (const w of s.words) expect(w.end).toBeGreaterThan(w.start);
		}
	});

	it("emits cues in order and never overlapping", () => {
		// Non-overlapping in SOURCE time, and straddling the cut — the old
		// endpoint-only remap turned this pair into overlapping output cues.
		const src = transcript([
			seg("a", 10, 19, [word(10, 11, "a1"), word(18, 19, "a2")]),
			seg("b", 19.5, 22, [word(19.5, 20, "b1"), word(21, 22, "b2")]),
		]);

		const out = toOutputTimeTranscript(MAP, src);

		for (let i = 1; i < out.segments.length; i++) {
			expect(out.segments[i].start).toBeGreaterThanOrEqual(out.segments[i - 1].end - 1e-9);
		}
	});

	it("drops a cue that falls entirely inside a cut", () => {
		const src = transcript([seg("gone", 13, 15, [word(13, 15, "removed")])]);
		expect(toOutputTimeTranscript(MAP, src).segments).toEqual([]);
	});

	it("leaves an uncut cue's id and text untouched", () => {
		const src = transcript([seg("a", 2, 5, [word(2, 3, "hello"), word(3, 5, "there")])]);
		const out = toOutputTimeTranscript(MAP, src);
		expect(out.segments).toHaveLength(1);
		expect(out.segments[0]).toMatchObject({ id: "a", text: "hello there", start: 2, end: 5 });
	});

	it("warps by per-segment speed", () => {
		// [0,12] at 2x → 6s of output; the cue at [4,8] lands at [2,4].
		const fast = buildTimeMap([{ origStart: 0, origEnd: 12, speed: 2 }]);
		const src = transcript([seg("a", 4, 8, [word(4, 8, "quick")])]);
		const out = toOutputTimeTranscript(fast, src);
		expect(out.segments[0]).toMatchObject({ start: 2, end: 4 });
	});
});
