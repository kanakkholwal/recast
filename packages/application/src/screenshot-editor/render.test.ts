import { describe, expect, it } from "vitest";
import type { ImageFilters, ImageStylePreset, Shadow } from "./types";
import {
	borderCss,
	filtersCss,
	hexWithAlpha,
	shadowCss,
	styleFrameBackground,
	transformCss,
} from "./render";

const NEUTRAL: ImageFilters = {
	brightness: 100,
	contrast: 100,
	saturate: 100,
	grayscale: 0,
	sepia: 0,
	hueRotate: 0,
	invert: 0,
	blur: 0,
};

describe("filtersCss", () => {
	it("is 'none' when every adjustment is neutral", () => {
		// Not the empty string: an empty `filter` is invalid CSS and the browser
		// would keep whatever was there before.
		expect(filtersCss(NEUTRAL)).toBe("none");
	});

	it("emits only the adjustments that moved", () => {
		expect(filtersCss({ ...NEUTRAL, contrast: 120 })).toBe("contrast(120%)");
		expect(filtersCss({ ...NEUTRAL, blur: 4, grayscale: 50 })).toBe("grayscale(50%) blur(4px)");
	});

	it("keeps a stable order so preview and export produce identical pixels", () => {
		const css = filtersCss({ ...NEUTRAL, blur: 2, brightness: 110, saturate: 90 });
		expect(css).toBe("brightness(110%) saturate(90%) blur(2px)");
	});
});

describe("hexWithAlpha", () => {
	it("expands shorthand hex", () => {
		expect(hexWithAlpha("#0af", 1)).toBe("rgba(0, 170, 255, 1)");
	});

	it("converts full hex, case-insensitively, and trims", () => {
		expect(hexWithAlpha("  #FF8000 ", 0.5)).toBe("rgba(255, 128, 0, 0.5)");
	});

	it("clamps alpha into range", () => {
		expect(hexWithAlpha("#000000", 4)).toBe("rgba(0, 0, 0, 1)");
		expect(hexWithAlpha("#000000", -1)).toBe("rgba(0, 0, 0, 0)");
	});

	it("passes non-hex colours through untouched", () => {
		// Callers may hand in any CSS colour; mangling it would blank the element.
		expect(hexWithAlpha("rebeccapurple", 0.5)).toBe("rebeccapurple");
		expect(hexWithAlpha("rgb(1, 2, 3)", 0.5)).toBe("rgb(1, 2, 3)");
		expect(hexWithAlpha("#12345", 0.5)).toBe("#12345");
	});
});

describe("shadowCss", () => {
	const base: Shadow = { x: 0, y: 20, blur: 40, spread: -10, color: "#000000", opacity: 0.4 };

	it("composes the offsets, blur, spread and colour", () => {
		expect(shadowCss(base)).toBe("0px 20px 40px -10px rgba(0, 0, 0, 0.4)");
	});

	it("is 'none' at zero opacity rather than a transparent shadow", () => {
		// A `0 0 0 rgba(...,0)` still forces a paint layer; "none" does not.
		expect(shadowCss({ ...base, opacity: 0 })).toBe("none");
	});
});

describe("borderCss", () => {
	it("is 'none' at zero width", () => {
		expect(borderCss({ width: 0, color: "#fff" })).toBe("none");
	});

	it("composes width and colour", () => {
		expect(borderCss({ width: 2, color: "#fff" })).toBe("2px solid #fff");
	});
});

describe("transformCss", () => {
	it("emits all three rotations and the scale", () => {
		expect(
			transformCss({
				perspective: 1600,
				rotateX: 5,
				rotateY: -10,
				rotateZ: 0,
				scale: 1.1,
				translateX: 0,
				translateY: 0,
			}),
		).toBe("rotateX(5deg) rotateY(-10deg) rotateZ(0deg) scale(1.1)");
	});

	it("prepends a percent translate only when non-zero", () => {
		expect(
			transformCss({
				perspective: 1600,
				rotateX: 0,
				rotateY: 0,
				rotateZ: 0,
				scale: 1,
				translateX: 5,
				translateY: -3,
			}),
		).toBe("translate(5%, -3%) rotateX(0deg) rotateY(0deg) rotateZ(0deg) scale(1)");
	});
});

const style = (preset: ImageStylePreset, opacity: number) => ({ preset, padding: 0, opacity });

describe("styleFrameBackground", () => {
	it("tints the glass presets with the style's own opacity", () => {
		expect(styleFrameBackground(style("glass-light", 0.2))).toBe("rgba(255, 255, 255, 0.2)");
		expect(styleFrameBackground(style("glass-dark", 0.3))).toBe("rgba(0, 0, 0, 0.3)");
	});

	it("keeps the border presets solid, ignoring opacity", () => {
		expect(styleFrameBackground(style("border-light", 0.1))).toBe("rgb(255, 255, 255)");
		expect(styleFrameBackground(style("border-dark", 0.1))).toBe("rgb(26, 26, 26)");
	});

	it("falls back to transparent for the default preset", () => {
		expect(styleFrameBackground(style("default", 0.5))).toBe("transparent");
	});
});
