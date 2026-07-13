import { describe, expect, it } from "vitest";
import fixture from "./__fixtures__/caption-parity.json";
import { chunkWords, resolveCaptionAnimation } from "./chunking";
import { karaokeCentiseconds, spokenWordCount } from "./highlight";
import { breakIntoLines } from "./linebreak";
import type { CaptionAnimation, TranscriptWord } from "./types";

interface Case {
	name: string;
	words: TranscriptWord[];
	animation: Partial<CaptionAnimation>;
	maxCharsPerLine: number;
	maxLines: number;
	expected: {
		chunks: number[][];
		lines: number[][];
		karaokeCs: number[][];
		spokenAt: { t: number; count: number }[];
	};
}

// The same values a Rust test asserts against, so preview and export cannot
// drift. If you change a heuristic, update the fixture and BOTH sides.
describe("caption parity fixture", () => {
	const cases = (fixture as { cases: Case[] }).cases;

	for (const c of cases) {
		describe(c.name, () => {
			const anim = resolveCaptionAnimation({
				...resolveCaptionAnimation(undefined),
				...c.animation,
			} as CaptionAnimation);

			it("chunks words as expected", () => {
				const runs = chunkWords(c.words, anim);
				const asIndices = runs.map((run) =>
					run.words.map((w) => c.words.findIndex((cw) => cw.start === w.start && cw.text === w.text)),
				);
				expect(asIndices).toEqual(c.expected.chunks);
			});

			it("breaks lines as expected", () => {
				expect(breakIntoLines(c.words, c.maxCharsPerLine, c.maxLines)).toEqual(c.expected.lines);
			});

			it("emits the expected karaoke centiseconds per chunk", () => {
				const runs = chunkWords(c.words, anim);
				const cs = runs.map((run) => karaokeCentiseconds(run.words, run.start));
				expect(cs).toEqual(c.expected.karaokeCs);
			});

			it("counts spoken words at sampled times", () => {
				for (const s of c.expected.spokenAt) {
					expect(spokenWordCount(c.words, s.t)).toBe(s.count);
				}
			});
		});
	}
});
