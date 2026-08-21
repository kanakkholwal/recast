import { describe, expect, it } from "vitest";
import {
	ANIMATION_PRESETS,
	CATEGORY_LABELS,
	DEFAULT_PROPS,
	type AnimationPreset,
	presetById,
	presetsByCategory,
	propsAtTime,
	propsToTransform,
} from "./animation";

/** One track, two keyframes, linear so the maths is checkable by hand. */
const linear: AnimationPreset = {
	id: "test",
	name: "Test",
	category: "fade",
	duration: 1000,
	tracks: [
		{
			keyframes: [
				{ time: 0, props: { opacity: 0, scale: 0.5 }, easing: "linear" },
				{ time: 1000, props: { opacity: 1, scale: 1.5 }, easing: "linear" },
			],
		},
	],
};

describe("propsAtTime", () => {
	it("returns the endpoints exactly", () => {
		expect(propsAtTime(linear, 0).opacity).toBe(0);
		expect(propsAtTime(linear, 1000).opacity).toBe(1);
	});

	it("interpolates between keyframes", () => {
		const mid = propsAtTime(linear, 500);
		expect(mid.opacity).toBeCloseTo(0.5, 6);
		expect(mid.scale).toBeCloseTo(1, 6);
	});

	it("holds the nearest keyframe outside the track's range", () => {
		expect(propsAtTime(linear, -100).opacity).toBe(0);
		expect(propsAtTime(linear, 99_999).opacity).toBe(1);
	});

	it("leaves untouched properties at their defaults", () => {
		// The track animates opacity and scale only; everything else must stay
		// neutral or the stage would jump when a preset is applied.
		const at = propsAtTime(linear, 500);
		expect(at.rotateX).toBe(DEFAULT_PROPS.rotateX);
		expect(at.translateY).toBe(DEFAULT_PROPS.translateY);
		expect(at.perspective).toBe(DEFAULT_PROPS.perspective);
	});

	it("does not depend on the keyframes being stored in time order", () => {
		const unsorted: AnimationPreset = {
			...linear,
			tracks: [{ keyframes: [...linear.tracks[0].keyframes].reverse() }],
		};
		expect(propsAtTime(unsorted, 500).opacity).toBeCloseTo(0.5, 6);
	});

	it("applies the INCOMING keyframe's easing, not the outgoing one", () => {
		const eased: AnimationPreset = {
			...linear,
			tracks: [
				{
					keyframes: [
						{ time: 0, props: { opacity: 0 }, easing: "linear" },
						// ease-in = t², so halfway through time is a quarter of the way
						// through the value.
						{ time: 1000, props: { opacity: 1 }, easing: "ease-in" },
					],
				},
			],
		};
		expect(propsAtTime(eased, 500).opacity).toBeCloseTo(0.25, 6);
	});
});

describe("propsToTransform", () => {
	it("emits the properties CSS applies, and leaves perspective to the parent", () => {
		const css = propsToTransform({ ...DEFAULT_PROPS, rotateY: 30, scale: 1.2, translateX: 10 });
		expect(css).toContain("rotateY(30deg)");
		expect(css).toContain("translate(10%, 0%)");
		expect(css).toContain("scale(1.2)");
		expect(css).not.toContain("perspective");
	});
});

describe("shipped presets", () => {
	it("have unique ids", () => {
		const ids = ANIMATION_PRESETS.map((p) => p.id);
		expect(new Set(ids).size).toBe(ids.length);
	});

	it("every preset resolves by id, and an unknown one is null", () => {
		for (const p of ANIMATION_PRESETS) expect(presetById(p.id)?.id).toBe(p.id);
		expect(presetById("nope")).toBeNull();
		expect(presetById(null)).toBeNull();
	});

	it("carry a positive duration and at least one keyframe", () => {
		for (const p of ANIMATION_PRESETS) {
			expect(p.duration, p.id).toBeGreaterThan(0);
			expect(p.tracks.length, p.id).toBeGreaterThan(0);
			for (const track of p.tracks) expect(track.keyframes.length, p.id).toBeGreaterThan(0);
		}
	});

	it("settle on the neutral pose at the end so the still matches the export", () => {
		// The video export ends on the last frame; if a preset finished mid-flight
		// the exported still would not match the editor's resting stage.
		for (const p of ANIMATION_PRESETS) {
			const end = propsAtTime(p, p.duration);
			expect(end.opacity, p.id).toBeCloseTo(1, 6);
		}
	});

	it("group under labelled categories, with no empty group", () => {
		const groups = presetsByCategory();
		for (const g of groups) {
			expect(g.presets.length, g.category).toBeGreaterThan(0);
			expect(g.label).toBe(CATEGORY_LABELS[g.category]);
		}
		expect(groups.reduce((n, g) => n + g.presets.length, 0)).toBe(ANIMATION_PRESETS.length);
	});
});
