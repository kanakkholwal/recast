import { describe, expect, it } from "vitest";
import { clampValue, dragDelta, formatValue, parseInputValue } from "@recast/ui/number-field-logic";

describe("clampValue", () => {
	it("clamps into range and passes through when unbounded", () => {
		expect(clampValue(5, 0, 10)).toBe(5);
		expect(clampValue(-1, 0, 10)).toBe(0);
		expect(clampValue(11, 0, 10)).toBe(10);
		expect(clampValue(99)).toBe(99);
	});
});

describe("dragDelta", () => {
	it("is one step per px, Shift ×10, Alt ×0.1", () => {
		expect(dragDelta(4, 1)).toBe(4);
		expect(dragDelta(4, 1, { coarse: true })).toBe(40);
		expect(dragDelta(4, 1, { fine: true })).toBe(0.4);
	});
});

describe("parseInputValue", () => {
	it("keeps the fallback on garbage so a stray keystroke never zeroes a field", () => {
		expect(parseInputValue("3.5", 0)).toBe(3.5);
		expect(parseInputValue("abc", 7)).toBe(7);
	});
});

describe("formatValue", () => {
	it("fixes to the requested decimals", () => {
		expect(formatValue(3, 0)).toBe("3");
		expect(formatValue(Math.PI, 1)).toBe("3.1");
	});
});
