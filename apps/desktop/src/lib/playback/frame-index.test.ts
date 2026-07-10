import { describe, expect, it } from "vitest";
import {
	buildKeyframes,
	buildPresOrder,
	type ChunkMeta,
	keyframeAtOrBefore,
	needsReset,
	sampleAtOrBefore,
} from "./frame-index";

/** Build a chunk with a presentation time (µs) and keyframe flag. */
function chunk(ctsUs: number, key = false): ChunkMeta {
	return { ctsUs, durUs: 33_333, key, data: new Uint8Array(0) };
}

describe("buildKeyframes", () => {
	it("returns decode-order indices of sync samples", () => {
		const chunks = [chunk(0, true), chunk(33), chunk(66), chunk(99, true)];
		expect(buildKeyframes(chunks)).toEqual([0, 3]);
	});

	it("is empty when there are no keyframes", () => {
		expect(buildKeyframes([chunk(0), chunk(1)])).toEqual([]);
	});
});

describe("buildPresOrder", () => {
	it("is identity when decode order == presentation order", () => {
		const chunks = [chunk(0), chunk(10), chunk(20)];
		expect(buildPresOrder(chunks)).toEqual([0, 1, 2]);
	});

	it("reorders when B-frames make cts differ from decode order", () => {
		// Classic IPBB: decode order [I@0, P@3, B@1, B@2], presentation 0<1<2<3.
		const chunks = [chunk(0, true), chunk(3000), chunk(1000), chunk(2000)];
		expect(buildPresOrder(chunks)).toEqual([0, 2, 3, 1]);
	});
});

describe("sampleAtOrBefore", () => {
	const chunks = [chunk(0, true), chunk(3000), chunk(1000), chunk(2000)];
	const pres = buildPresOrder(chunks); // [0,2,3,1] by cts 0,1000,2000,3000

	it("returns the decode-index whose cts is the greatest ≤ t", () => {
		expect(sampleAtOrBefore(chunks, pres, 0)).toBe(0);
		expect(sampleAtOrBefore(chunks, pres, 1500)).toBe(2); // cts 1000
		expect(sampleAtOrBefore(chunks, pres, 2000)).toBe(3); // exact cts 2000
		expect(sampleAtOrBefore(chunks, pres, 99_999)).toBe(1); // cts 3000, last
	});

	it("clamps to the earliest sample when t precedes everything", () => {
		expect(sampleAtOrBefore(chunks, pres, -500)).toBe(0);
	});
});

describe("keyframeAtOrBefore", () => {
	const keyframes = [0, 30, 60];

	it("returns the largest keyframe ≤ index", () => {
		expect(keyframeAtOrBefore(keyframes, 0)).toBe(0);
		expect(keyframeAtOrBefore(keyframes, 29)).toBe(0);
		expect(keyframeAtOrBefore(keyframes, 30)).toBe(30);
		expect(keyframeAtOrBefore(keyframes, 45)).toBe(30);
		expect(keyframeAtOrBefore(keyframes, 1000)).toBe(60);
	});
});

describe("needsReset", () => {
	it("resets when never primed", () => {
		expect(needsReset(-1, 0, 0)).toBe(true);
	});

	it("does NOT reset during continuous forward play within a GOP", () => {
		// Primed at kf 30, fed up to 50, target's keyframe is still 30.
		expect(needsReset(30, 50, 30)).toBe(false);
	});

	it("does NOT reset when crossing into a later GOP we've already fed past", () => {
		// Fed continuously to index 70; new GOP keyframe 60 is ≤ feedCursor.
		expect(needsReset(30, 70, 60)).toBe(false);
	});

	it("resets on a backward jump to an earlier keyframe", () => {
		expect(needsReset(60, 80, 30)).toBe(true);
	});

	it("resets on a forward jump across a gap (e.g. a cut) we haven't fed to", () => {
		// Playing in GOP@0 (fed to 20), user jumps to GOP@90: gap.
		expect(needsReset(0, 20, 90)).toBe(true);
	});

	it("does NOT reset on a forward GOP gap when it's lag, not a jump", () => {
		// Decoder fell behind: playhead crossed keyframe 90 while we've only fed to
		// 20. Same indices as the jump case, but the request advanced smoothly
		// (forwardIsJump=false) → keep streaming contiguously, don't reset+skip.
		expect(needsReset(0, 20, 90, false)).toBe(false);
	});

	it("still resets backward even when not a jump", () => {
		// A backward move is always a discontinuity regardless of the jump flag.
		expect(needsReset(60, 80, 30, false)).toBe(true);
	});
});
