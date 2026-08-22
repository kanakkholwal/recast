import { describe, expect, it } from "vitest";
import { barWidth, formatPct } from "./format";

describe("formatPct", () => {
	it("collapses float noise to a readable bucket", () => {
		expect(formatPct(0.0009770365977601614)).toBe("<1%");
	});

	it("renders an exact zero as zero, not as a sliver", () => {
		expect(formatPct(0)).toBe("0%");
		expect(formatPct(null)).toBe("0%");
	});

	it("rounds and clamps", () => {
		expect(formatPct(49.6)).toBe("50%");
		expect(formatPct(140)).toBe("100%");
		expect(formatPct(-5)).toBe("0%");
	});
});

describe("barWidth", () => {
	it("keeps non-zero usage visible", () => {
		expect(barWidth(0.001)).toBe(2);
		expect(barWidth(0)).toBe(0);
	});

	it("clamps to the track", () => {
		expect(barWidth(250)).toBe(100);
		expect(barWidth(-1)).toBe(0);
	});
});
