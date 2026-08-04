import { describe, expect, it } from "vitest";
import { LINEAR, type Easing } from "../easing/cubic-bezier";
import type { Segment } from "../timeline/segments";
import type { SceneAnimSpec, SegmentAnim } from "./segment-anim";
import { evalSceneAt, evalSegmentTransform, SCENE_IDENTITY } from "./eval";
import sceneParity from "./__fixtures__/scene-parity.json";

const lin: SceneAnimSpec = { kind: "fade", durationMs: 1000, easing: LINEAR };

describe("evalSegmentTransform", () => {
	it("is identity with no animation", () => {
		expect(evalSegmentTransform(null, 1, 0, 10)).toEqual(SCENE_IDENTITY);
	});
	it("is identity during the hold between ramps", () => {
		const anim: SegmentAnim = { start: 0, in: lin, out: lin };
		expect(evalSegmentTransform(anim, 5, 0, 10)).toEqual(SCENE_IDENTITY);
	});
	it("caps each ramp to 40% of the window so a hold always remains", () => {
		// 1s clip, both ramps 1s → each caps to 0.4s; the centre is a hold, so the
		// two never overlap (the anti-wobble guard).
		const anim: SegmentAnim = { start: 0, in: lin, out: lin };
		expect(evalSegmentTransform(anim, 0.2, 0, 1).opacity).toBeCloseTo(0.5, 6); // mid entrance
		expect(evalSegmentTransform(anim, 0.5, 0, 1)).toEqual(SCENE_IDENTITY); // hold
		expect(evalSegmentTransform(anim, 0.8, 0, 1).opacity).toBeCloseTo(0.5, 6); // mid exit
	});
	it("keeps a too-short segment static (no wobble from silence-cut fragments)", () => {
		const anim: SegmentAnim = { start: 0, in: { ...lin, durationMs: 5000 } };
		expect(evalSegmentTransform(anim, 0.05, 0, 0.15)).toEqual(SCENE_IDENTITY);
	});
});

describe("evalSceneAt", () => {
	// Two contiguous original segments: [0,4] and [4,10].
	const segments: Segment[] = [
		{ start: 0, end: 4, index: 0 },
		{ start: 4, end: 10, index: 1 },
	];
	it("is identity with no overrides", () => {
		expect(evalSceneAt(segments, [], 2)).toEqual(SCENE_IDENTITY);
	});
	it("routes a time to the animation of its containing segment", () => {
		const anims: SegmentAnim[] = [
			{ start: 4, in: { kind: "fade", durationMs: 1000, easing: LINEAR } },
		];
		// Second segment starts at 4; 0.5s in → opacity 0.5.
		expect(evalSceneAt(segments, anims, 4.5).opacity).toBeCloseTo(0.5, 6);
		// First segment has no animation.
		expect(evalSceneAt(segments, anims, 1)).toEqual(SCENE_IDENTITY);
	});
	it("gives the final segment the end boundary", () => {
		const anims: SegmentAnim[] = [
			{ start: 4, out: { kind: "fade", durationMs: 1000, easing: LINEAR } },
		];
		// t == last segment end → exit fully played out (opacity 0).
		expect(evalSceneAt(segments, anims, 10).opacity).toBeCloseTo(0, 6);
	});
});

describe("scene parity (shared fixture with Rust export)", () => {
	const toEasing = (a: number[]): Easing => ({ x1: a[0], y1: a[1], x2: a[2], y2: a[3] });
	const toSpec = (raw: Record<string, unknown> | undefined): SceneAnimSpec | undefined =>
		raw
			? ({
					kind: raw.kind,
					durationMs: raw.durationMs,
					easing: toEasing(raw.easing as number[]),
					dir: raw.dir,
					intensity: raw.intensity,
				} as SceneAnimSpec)
			: undefined;

	for (const c of sceneParity.cases) {
		it(`transform: ${c.name}`, () => {
			const anim: SegmentAnim = {
				start: c.window[0],
				in: toSpec((c as Record<string, unknown>).in as Record<string, unknown>),
				out: toSpec((c as Record<string, unknown>).out as Record<string, unknown>),
			};
			for (const s of c.samples) {
				const tf = evalSegmentTransform(anim, s.t, c.window[0], c.window[1]);
				const rotate = (s as { rotate?: number }).rotate ?? 0;
				expect(tf.opacity).toBeCloseTo(s.opacity, 6);
				expect(tf.translateX).toBeCloseTo(s.translateX, 6);
				expect(tf.translateY).toBeCloseTo(s.translateY, 6);
				expect(tf.scale).toBeCloseTo(s.scale, 6);
				expect(tf.rotate).toBeCloseTo(rotate, 6);
			}
		});
	}
});
