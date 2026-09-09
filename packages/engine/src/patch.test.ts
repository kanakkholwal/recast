import { describe, expect, it } from "vitest";
import { isEmptyPatch, shallowPatch } from "./patch";

describe("shallowPatch", () => {
	it("names changed primitives and re-identified objects, nothing else", () => {
		const cuts = [{ start: 1, end: 2 }];
		const prev = { padding: 4, trimEnd: 10, cuts, shadow: { blur: 1 } };
		const next = { padding: 12, trimEnd: 10, cuts, shadow: { blur: 1 } };
		expect(shallowPatch(prev, next)).toEqual({ padding: 12, shadow: { blur: 1 } });
		expect(isEmptyPatch(shallowPatch(prev, prev))).toBe(true);
	});

	it("marks a field that went away with null and ignores undefined", () => {
		expect(shallowPatch({ a: 1, b: 2 }, { a: 1, b: undefined })).toEqual({ b: null });
		expect(shallowPatch({ a: 1 }, { a: 1, c: undefined })).toEqual({});
	});

	it("treats NaN as equal to itself so a stuck value is not re-sent every frame", () => {
		expect(isEmptyPatch(shallowPatch({ a: Number.NaN }, { a: Number.NaN }))).toBe(true);
	});
});
