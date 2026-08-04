import { describe, expect, it } from "vitest";
import { isCursorAnimTouched, svgSwatchUrl } from "./cursor-panel.logic";

type Anim = Parameters<typeof isCursorAnimTouched>[0];
const defaults = { clickBounce: 0, sway: 0, motionBlur: 0, bounceSpeedMs: 220 } as Anim;

describe("isCursorAnimTouched", () => {
	it("is false at defaults so Reset stays hidden", () => {
		expect(isCursorAnimTouched(defaults)).toBe(false);
	});

	it("catches every knob, including bounce speed", () => {
		for (const [key, value] of [
			["clickBounce", 1],
			["sway", 0.2],
			["motionBlur", 0.5],
			["bounceSpeedMs", 300],
		] as const) {
			expect(isCursorAnimTouched({ ...defaults, [key]: value }), key).toBe(true);
		}
	});
});

describe("svgSwatchUrl", () => {
	it("produces an inert data URL rather than injectable markup", () => {
		const url = svgSwatchUrl('<svg><circle r="1"/></svg>');
		expect(url.startsWith("data:image/svg+xml;utf8,")).toBe(true);
		// Encoded, so an <img src> cannot be broken out of by pack-supplied SVG.
		expect(url).not.toContain("<svg>");
		expect(decodeURIComponent(url.replace("data:image/svg+xml;utf8,", ""))).toContain("<svg>");
	});

	it("collapses newlines and indentation to single spaces", () => {
		const decoded = decodeURIComponent(
			svgSwatchUrl("<svg>\n   <g/>\n</svg>").replace("data:image/svg+xml;utf8,", ""),
		);
		expect(decoded).toBe("<svg> <g/> </svg>");
	});
});
