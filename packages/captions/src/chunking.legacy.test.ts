import { describe, expect, it } from "vitest";
import {
	activeChunkIndex,
	activeWordIndex,
	chunkWords,
	DEFAULT_CAPTION_ANIMATION,
	isStaticAnimation,
	resolveCaptionAnimation,
} from "./chunking";
import type { CaptionAnimation } from "./types";

const W = (start: number, end: number, text = "w") => ({ start, end, text });
const anim = (over: Partial<CaptionAnimation>): CaptionAnimation => ({
	...DEFAULT_CAPTION_ANIMATION,
	...over,
});

describe("resolveCaptionAnimation", () => {
	it("returns the static default when undefined", () => {
		expect(resolveCaptionAnimation(undefined)).toEqual(DEFAULT_CAPTION_ANIMATION);
	});
	it("fills missing fields from the default", () => {
		const a = resolveCaptionAnimation({ chunk: "word" } as CaptionAnimation);
		expect(a.chunk).toBe("word");
		expect(a.entranceMs).toBe(DEFAULT_CAPTION_ANIMATION.entranceMs);
	});
});

describe("isStaticAnimation", () => {
	it("true only when line + no emphasis + no entrance", () => {
		expect(isStaticAnimation(DEFAULT_CAPTION_ANIMATION)).toBe(true);
		expect(isStaticAnimation(anim({ emphasis: "color" }))).toBe(false);
		expect(isStaticAnimation(anim({ entrance: "pop" }))).toBe(false);
		expect(isStaticAnimation(anim({ chunk: "word" }))).toBe(false);
	});
});

describe("chunkWords", () => {
	const words = [W(0, 1), W(1, 2), W(2, 3), W(3, 4), W(4, 5)];

	it("line → a single chunk with every word", () => {
		const runs = chunkWords(words, anim({ chunk: "line" }));
		expect(runs).toHaveLength(1);
		expect(runs[0].words).toHaveLength(5);
		expect(runs[0].start).toBe(0);
		expect(runs[0].end).toBe(5);
	});

	it("word → one chunk per word", () => {
		const runs = chunkWords(words, anim({ chunk: "word" }));
		expect(runs).toHaveLength(5);
		expect(runs.every((r) => r.words.length === 1)).toBe(true);
	});

	it("phrase → fixed-size greedy groups", () => {
		const runs = chunkWords(words, anim({ chunk: "phrase", chunkSize: 2 }));
		expect(runs.map((r) => r.words.length)).toEqual([2, 2, 1]);
		expect(runs[1].start).toBe(2);
		expect(runs[1].end).toBe(4);
	});

	it("empty input → no chunks", () => {
		expect(chunkWords([], DEFAULT_CAPTION_ANIMATION)).toEqual([]);
	});

	it("phrase clamps a bad chunkSize to at least 1", () => {
		const runs = chunkWords(words, anim({ chunk: "phrase", chunkSize: 0 }));
		expect(runs).toHaveLength(5);
	});
});

describe("activeChunkIndex", () => {
	const runs = chunkWords([W(0, 1), W(1, 2), W(5, 6)], anim({ chunk: "word" }));

	it("holds the previous chunk through the gap before the next", () => {
		expect(activeChunkIndex(runs, 0.5)).toBe(0);
		expect(activeChunkIndex(runs, 3)).toBe(1); // gap between word 2 (ends 2) and 3 (starts 5)
		expect(activeChunkIndex(runs, 5.5)).toBe(2);
	});
	it("first chunk before anything has started", () => {
		expect(activeChunkIndex(runs, -1)).toBe(0);
	});
	it("-1 for no chunks", () => {
		expect(activeChunkIndex([], 0)).toBe(-1);
	});
});

describe("activeWordIndex", () => {
	const words = [W(0, 1), W(1, 2), W(3, 4)];

	it("returns the word containing t", () => {
		expect(activeWordIndex(words, 1.5, true)).toBe(1);
	});
	it("holds the last started word through a gap when holdGaps", () => {
		expect(activeWordIndex(words, 2.5, true)).toBe(1);
	});
	it("returns -1 in a gap when not holding", () => {
		expect(activeWordIndex(words, 2.5, false)).toBe(-1);
	});
	it("-1 before the first word", () => {
		expect(activeWordIndex(words, -1, true)).toBe(-1);
	});
});
