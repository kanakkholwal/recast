import { describe, expect, it } from "vitest";
import { clampAnimMs, DEFAULT_ANIM_MS, type SegmentAnim } from "./segment-anim";
import { seamTransitionAt, setSeamTransition } from "./seam";

describe("setSeamTransition", () => {
	it("writes a complementary slide pair for a push", () => {
		const out = setSeamTransition([], 0, 4, "push-left", "balanced");
		const left = out.find((a) => a.start === 0);
		const right = out.find((a) => a.start === 4);
		expect(left?.out).toMatchObject({ kind: "slide", dir: "left" });
		expect(right?.in).toMatchObject({ kind: "slide", dir: "right" });
	});
	it("clears both sides for none", () => {
		const set = setSeamTransition([], 0, 4, "push-up", "balanced");
		expect(setSeamTransition(set, 0, 4, "none", "balanced")).toEqual([]);
	});
	it("styles the slides with the motion tone", () => {
		const out = setSeamTransition([], 0, 4, "push-right", "energetic");
		const left = out.find((a) => a.start === 0);
		expect(left?.out?.durationMs).toBe(clampAnimMs(DEFAULT_ANIM_MS * 0.8));
	});
	it("only touches the seam's two segments, leaving others intact", () => {
		const existing: SegmentAnim[] = [{ start: 9, in: { kind: "fade", durationMs: 400, easing: { x1: 0, y1: 0, x2: 1, y2: 1 } } }];
		const out = setSeamTransition(existing, 0, 4, "push-left", "balanced");
		expect(out.find((a) => a.start === 9)?.in?.kind).toBe("fade");
	});
});

describe("seamTransitionAt", () => {
	it("round-trips every push kind", () => {
		for (const kind of ["push-left", "push-right", "push-up", "push-down"] as const) {
			const set = setSeamTransition([], 0, 4, kind, "balanced");
			expect(seamTransitionAt(set, 0, 4)).toBe(kind);
		}
	});
	it("is none when neither side animates", () => {
		expect(seamTransitionAt([], 0, 4)).toBe("none");
	});
	it("is custom when the sides don't form a recognised push pair", () => {
		const anims: SegmentAnim[] = [
			{ start: 0, out: { kind: "fade", durationMs: 400, easing: { x1: 0, y1: 0, x2: 1, y2: 1 } } },
		];
		expect(seamTransitionAt(anims, 0, 4)).toBe("custom");
	});
});
