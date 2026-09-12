import { describe, expect, it } from "vitest";
import type { Annotation } from "../../stores/editor-store.svelte";
import { evalOpacity } from "./eval";

const LINEAR = { x1: 0, y1: 0, x2: 1, y2: 1 };

function annotation(over: Partial<Annotation> = {}): Annotation {
	return {
		id: "a1",
		start: 10,
		end: 12,
		rampIn: 0,
		rampOut: 0,
		easeIn: { ...LINEAR },
		easeOut: { ...LINEAR },
		stroke: { width: 2, color: "#ffffff" },
		fill: "transparent",
		hidden: false,
		kind: { kind: "rect", x: 0, y: 0, w: 0.2, h: 0.2, radius: 0 },
		...over,
	} as Annotation;
}

describe("evalOpacity", () => {
	/**
	 * Rust's `annotation_alpha` treats both ends as inclusive, so the export drew
	 * an un-faded annotation on its own first frame while the preview did not.
	 * Adding one at the playhead left nothing on screen until you seeked past it.
	 */
	it("shows an annotation with no fade at its own start and end", () => {
		const a = annotation();

		expect(evalOpacity(a, 10)).toBe(1);
		expect(evalOpacity(a, 11)).toBe(1);
		expect(evalOpacity(a, 12)).toBe(1);
	});

	it("draws nothing outside the range", () => {
		const a = annotation();

		expect(evalOpacity(a, 9.99)).toBe(0);
		expect(evalOpacity(a, 12.01)).toBe(0);
	});

	it("still starts a fade at zero, which is what a fade means", () => {
		const a = annotation({ rampIn: 0.5, rampOut: 0.5 });

		expect(evalOpacity(a, 10)).toBe(0);
		expect(evalOpacity(a, 10.25)).toBeCloseTo(0.5, 5);
		expect(evalOpacity(a, 11)).toBe(1);
		expect(evalOpacity(a, 11.75)).toBeCloseTo(0.5, 5);
		expect(evalOpacity(a, 12)).toBe(0);
	});

	it("scales the whole curve by the master opacity", () => {
		const a = annotation({ opacity: 0.5 });

		expect(evalOpacity(a, 11)).toBe(0.5);
	});

	/** Each ramp is capped at half, or a short annotation would never reach full. */
	it("caps a ramp longer than half the annotation", () => {
		const a = annotation({ start: 0, end: 2, rampIn: 10, rampOut: 10 });

		expect(evalOpacity(a, 1)).toBe(1);
		expect(evalOpacity(a, 0.5)).toBeCloseTo(0.5, 5);
	});
});
