import { describe, expect, it } from "vitest";
import { hexToRgba } from "./color.logic";

describe("hexToRgba", () => {
	it("parses #rrggbb to normalised channels with default alpha", () => {
		expect(hexToRgba("#ffffff")).toEqual([1, 1, 1, 1]);
		expect(hexToRgba("#000000")).toEqual([0, 0, 0, 1]);
		const [r, g, b] = hexToRgba("#ff8000");
		expect(r).toBe(1);
		expect(g).toBeCloseTo(128 / 255, 5);
		expect(b).toBe(0);
	});
	it("reads the alpha byte from #rrggbbaa", () => {
		expect(hexToRgba("#00000080")[3]).toBeCloseTo(128 / 255, 5);
	});
	it("honours the alpha argument for 6-digit hex", () => {
		expect(hexToRgba("#ffffff", 0.5)).toEqual([1, 1, 1, 0.5]);
	});
	it("falls back to #111111 on malformed input", () => {
		expect(hexToRgba("nope", 0.3)).toEqual([17 / 255, 17 / 255, 17 / 255, 0.3]);
	});
});
