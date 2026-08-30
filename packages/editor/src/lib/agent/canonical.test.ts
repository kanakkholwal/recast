import { describe, expect, it } from "vitest";
import { canonicalize, sameRenderState } from "./canonical";

describe("canonicalize", () => {
	it("treats an explicit null as an omitted key", () => {
		// The exact shape the recording panel froze on: TS writes null where Rust's skip_serializing_if omits it.
		expect(sameRenderState({ trimStart: 0, cameraOverlay: null }, { trimStart: 0 })).toBe(true);
	});

	it("ignores key order", () => {
		expect(sameRenderState({ a: 1, b: 2 }, { b: 2, a: 1 })).toBe(true);
	});

	it("still reports a real difference", () => {
		expect(sameRenderState({ trimEnd: 10 }, { trimEnd: 12 })).toBe(false);
	});

	it("does not confuse an absent key with a falsy value", () => {
		expect(sameRenderState({ padding: 0 }, {})).toBe(false);
	});

	it("normalises nested objects and arrays", () => {
		const a = { cuts: [{ start: 1, end: 2, note: null }] };
		const b = { cuts: [{ end: 2, start: 1 }] };
		expect(sameRenderState(a, b)).toBe(true);
	});

	it("preserves array order", () => {
		expect(sameRenderState({ splitPoints: [1, 2] }, { splitPoints: [2, 1] })).toBe(false);
	});

	it("leaves primitives alone", () => {
		expect(canonicalize(5)).toBe(5);
		expect(canonicalize("x")).toBe("x");
		expect(canonicalize(null)).toBe(null);
	});
});
