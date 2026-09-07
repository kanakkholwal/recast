import { describe, expect, it } from "vitest";
import { clampValue, dragDelta, formatValue, parseInputValue } from "./draggable-value.logic";

describe("clampValue", () => {
	it("passes through without bounds", () => {
		expect(clampValue(42)).toBe(42);
		expect(clampValue(-42)).toBe(-42);
	});

	it("clamps to min and max", () => {
		expect(clampValue(150, 0, 100)).toBe(100);
		expect(clampValue(-5, 0, 100)).toBe(0);
		expect(clampValue(50, 0, 100)).toBe(50);
	});
});

describe("dragDelta", () => {
	it("moves one step per px", () => {
		expect(dragDelta(10, 0.5)).toBe(5);
		expect(dragDelta(-4, 1)).toBe(-4);
	});

	it("scales ×10 coarse and ×0.1 fine", () => {
		expect(dragDelta(10, 0.5, { coarse: true })).toBe(50);
		expect(dragDelta(10, 0.5, { fine: true })).toBeCloseTo(0.5);
	});

	it("coarse wins when both modifiers are held", () => {
		expect(dragDelta(1, 1, { coarse: true, fine: true })).toBe(10);
	});
});

describe("parseInputValue", () => {
	it("parses numbers, including decimals and negatives", () => {
		expect(parseInputValue("42", 0)).toBe(42);
		expect(parseInputValue("-3.5", 0)).toBe(-3.5);
	});

	it("keeps the fallback on garbage", () => {
		expect(parseInputValue("", 7)).toBe(7);
		expect(parseInputValue("abc", 7)).toBe(7);
	});

	it("accepts leading-number strings the way the old number inputs did", () => {
		expect(parseInputValue("12px", 0)).toBe(12);
	});
});

describe("formatValue", () => {
	it("fixes to the requested precision", () => {
		expect(formatValue(12.3456, 2)).toBe("12.35");
		expect(formatValue(5, 0)).toBe("5");
	});
});
