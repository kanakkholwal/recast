import { describe, expect, it } from "vitest";
import { REF_ASPECTS, REF_GRADIENTS, REF_MAGIC, REF_MESH, REF_SOLIDS } from "./backgrounds-data";
import { FONT_FAMILIES } from "./fonts";
import { TRANSFORM_PRESET_CATEGORIES } from "./transform-presets";
import { IMAGE_BACKGROUND_CATEGORIES, OVERLAY_SHADOWS } from "./image-backgrounds";
import { DEFAULT_PROPS, keyframesToPreset, propsAtTime } from "./animation";

/** Counts transcribed from screenshot-studio. Asserting them as independent
 * literals means any accidental drift (a dropped preset, a bad regen) fails the
 * build instead of silently shrinking the picker. */
describe("data parity with screenshot-studio", () => {
	it("background tables match upstream counts", () => {
		expect(REF_GRADIENTS).toHaveLength(102);
		expect(REF_MAGIC).toHaveLength(100);
		expect(REF_MESH).toHaveLength(12);
		expect(REF_SOLIDS).toHaveLength(33);
		expect(REF_ASPECTS).toHaveLength(25); // 24 upstream (custom excluded) + our "Auto"
	});

	it("font library is the full 32 families", () => {
		expect(FONT_FAMILIES).toHaveLength(32);
	});

	it("transform gallery is 39 presets across 7 categories", () => {
		expect(TRANSFORM_PRESET_CATEGORIES).toHaveLength(7);
		const total = TRANSFORM_PRESET_CATEGORIES.reduce((n, c) => n + c.presets.length, 0);
		expect(total).toBe(39);
	});

	it("bundled image assets match what shipped", () => {
		const bg = IMAGE_BACKGROUND_CATEGORIES.reduce((n, c) => n + c.images.length, 0);
		expect(bg).toBe(45);
		expect(OVERLAY_SHADOWS).toHaveLength(19);
	});

	it("ids are unique within each background table", () => {
		for (const table of [REF_GRADIENTS, REF_MAGIC, REF_MESH, REF_SOLIDS]) {
			const ids = new Set(table.map((p) => p.id));
			expect(ids.size).toBe(table.length);
		}
	});
});

describe("user keyframes → synthetic preset", () => {
	const props = (o: Partial<typeof DEFAULT_PROPS>) => ({ ...DEFAULT_PROPS, ...o });

	it("returns null below one keyframe", () => {
		expect(keyframesToPreset([])).toBeNull();
	});

	it("duration is the last keyframe time and interpolation runs between them", () => {
		const preset = keyframesToPreset([
			{ id: "a", time: 0, props: props({ scale: 1, rotateY: 0 }), easing: "linear" },
			{ id: "b", time: 1000, props: props({ scale: 2, rotateY: 40 }), easing: "linear" },
		]);
		expect(preset).not.toBeNull();
		expect(preset?.duration).toBe(1000);
		const mid = propsAtTime(preset!, 500);
		expect(mid.scale).toBeCloseTo(1.5, 5);
		expect(mid.rotateY).toBeCloseTo(20, 5);
	});
});
