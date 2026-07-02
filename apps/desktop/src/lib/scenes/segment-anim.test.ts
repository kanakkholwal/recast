import { describe, expect, it } from "vitest";
import { EASE_IN, EASE_IN_OUT, EASE_OUT } from "../easing/cubic-bezier";
import type { Segment } from "../timeline/segments";
import {
	clampAnimMs,
	DEFAULT_ANIM_MS,
	defaultSpec,
	MAX_ANIM_MS,
	MIN_ANIM_MS,
	pruneSegmentAnims,
	retuneAnimsForTone,
	type SceneAnimSpec,
	type SegmentAnim,
	segmentAnimAt,
	setSegmentAnim,
} from "./segment-anim";

function seg(start: number, end: number, index: number): Segment {
	return { start, end, index };
}

const fade: SceneAnimSpec = { kind: "fade", durationMs: 400, easing: EASE_OUT };
const slide: SceneAnimSpec = { kind: "slide", durationMs: 400, easing: EASE_IN, dir: "right" };

describe("clampAnimMs", () => {
	it("keeps in-range values", () => {
		expect(clampAnimMs(500)).toBe(500);
	});
	it("clamps to the supported bounds", () => {
		expect(clampAnimMs(99999)).toBe(MAX_ANIM_MS);
		expect(clampAnimMs(1)).toBe(MIN_ANIM_MS);
	});
	it("falls back to the default for a non-finite value", () => {
		expect(clampAnimMs(Number.NaN)).toBe(DEFAULT_ANIM_MS);
	});
});

describe("defaultSpec", () => {
	it("gives slide a direction that differs by side", () => {
		expect(defaultSpec("slide", "in").dir).toBe("left");
		expect(defaultSpec("slide", "out").dir).toBe("right");
	});
	it("uses a bouncy ease for pop regardless of side", () => {
		const inPop = defaultSpec("pop", "in");
		const outPop = defaultSpec("pop", "out");
		expect(inPop.easing).toEqual(outPop.easing);
	});
});

describe("motion tone", () => {
	it("leaves balanced identical to the original defaults", () => {
		const s = defaultSpec("slide", "in", "balanced");
		expect(s.durationMs).toBe(DEFAULT_ANIM_MS);
		expect(s.easing).toEqual(EASE_OUT);
		expect(s.intensity).toBeUndefined();
	});
	it("subtle is slower, gentler, and smoothly eased", () => {
		const s = defaultSpec("slide", "in", "subtle");
		expect(s.durationMs).toBe(clampAnimMs(DEFAULT_ANIM_MS * 1.25));
		expect(s.easing).toEqual(EASE_IN_OUT);
		expect(s.intensity).toBeCloseTo(0.36, 6); // 0.6 default × 0.6
	});
	it("energetic is quicker and bigger", () => {
		const s = defaultSpec("slide", "in", "energetic");
		expect(s.durationMs).toBe(clampAnimMs(DEFAULT_ANIM_MS * 0.8));
		expect(s.intensity).toBeCloseTo(0.75, 6); // 0.6 default × 1.25
	});
	it("retunes all animations to the tone but preserves kind and slide direction", () => {
		const anims: SegmentAnim[] = [
			{ start: 0, in: { kind: "slide", durationMs: 900, easing: EASE_IN, dir: "up", intensity: 1.2 } },
			{ start: 5, out: { kind: "fade", durationMs: 900, easing: EASE_IN } },
		];
		const out = retuneAnimsForTone(anims, "subtle");
		expect(out[0].in?.kind).toBe("slide");
		expect(out[0].in?.dir).toBe("up"); // direction kept
		expect(out[0].in?.durationMs).toBe(clampAnimMs(DEFAULT_ANIM_MS * 1.25)); // restyled
		expect(out[1].out?.kind).toBe("fade");
		expect(out[1].out?.easing).toEqual(EASE_IN_OUT);
	});
});

describe("segmentAnimAt", () => {
	const overrides: SegmentAnim[] = [{ start: 4, in: fade }];
	it("returns the matching entry within tolerance", () => {
		expect(segmentAnimAt(overrides, 4)?.in).toBe(fade);
		expect(segmentAnimAt(overrides, 4.00005)?.in).toBe(fade);
	});
	it("returns null for an unmatched anchor", () => {
		expect(segmentAnimAt(overrides, 5)).toBeNull();
		expect(segmentAnimAt([], 4)).toBeNull();
	});
});

describe("setSegmentAnim", () => {
	it("inserts a new entry, sorted by start", () => {
		const out = setSegmentAnim([{ start: 6, in: fade }], 2, "in", slide);
		expect(out.map((o) => o.start)).toEqual([2, 6]);
		expect(out[0].in).toBe(slide);
	});
	it("sets one side without clobbering the other", () => {
		let out = setSegmentAnim([], 2, "in", fade);
		out = setSegmentAnim(out, 2, "out", slide);
		expect(out).toHaveLength(1);
		expect(out[0].in).toBe(fade);
		expect(out[0].out).toBe(slide);
	});
	it("clears one side and keeps the other", () => {
		const start = setSegmentAnim(setSegmentAnim([], 2, "in", fade), 2, "out", slide);
		const out = setSegmentAnim(start, 2, "in", null);
		expect(out[0].in).toBeUndefined();
		expect(out[0].out).toBe(slide);
	});
	it("drops the entry once both sides are cleared (stays sparse)", () => {
		const start = setSegmentAnim([], 2, "in", fade);
		expect(setSegmentAnim(start, 2, "in", null)).toEqual([]);
	});
	it("does not mutate the input", () => {
		const input: SegmentAnim[] = [{ start: 2, in: fade }];
		setSegmentAnim(input, 2, "out", slide);
		expect(input).toEqual([{ start: 2, in: fade }]);
	});
});

describe("pruneSegmentAnims", () => {
	it("keeps anchors that still match a segment start", () => {
		const segs = [seg(2, 4, 0), seg(4, 8, 1)];
		const overrides: SegmentAnim[] = [
			{ start: 2, in: fade },
			{ start: 9, out: slide },
		];
		expect(pruneSegmentAnims(overrides, segs)).toEqual([{ start: 2, in: fade }]);
	});
	it("returns empty for empty input", () => {
		expect(pruneSegmentAnims([], [seg(0, 1, 0)])).toEqual([]);
	});
});
