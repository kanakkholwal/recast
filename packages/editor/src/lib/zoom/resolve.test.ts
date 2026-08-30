import { describe, expect, it } from "vitest";
import { activeZoomIndex, overlappingZoomIds, type ZoomWindow } from "./resolve";

const R = (id: string, start: number, end: number, hidden = false): ZoomWindow => ({
	id,
	start,
	end,
	hidden,
});

describe("activeZoomIndex", () => {
	it("returns -1 outside every region", () => {
		expect(activeZoomIndex([R("a", 2, 4)], 1)).toBe(-1);
		expect(activeZoomIndex([R("a", 2, 4)], 5)).toBe(-1);
	});

	it("treats the boundaries as outside, matching the ramp math", () => {
		expect(activeZoomIndex([R("a", 2, 4)], 2)).toBe(-1);
		expect(activeZoomIndex([R("a", 2, 4)], 4)).toBe(-1);
	});

	it("skips hidden regions", () => {
		expect(activeZoomIndex([R("a", 2, 4, true)], 3)).toBe(-1);
	});

	// The nested region is the more specific intent, and the rule must not depend on invisible creation order.
	it("gives a nested region priority over the one enclosing it", () => {
		const enclosing = R("outer", 0, 10);
		const nested = R("inner", 4, 6);
		expect(activeZoomIndex([enclosing, nested], 5)).toBe(1);
		expect(activeZoomIndex([nested, enclosing], 5)).toBe(0);
	});

	it("is independent of array order for partial overlaps", () => {
		const first = R("a", 0, 6);
		const second = R("b", 4, 10);
		expect(activeZoomIndex([first, second], 5)).toBe(1);
		expect(activeZoomIndex([second, first], 5)).toBe(0);
	});

	it("breaks a same-start tie on the later array entry", () => {
		expect(activeZoomIndex([R("a", 0, 5), R("b", 0, 5)], 2)).toBe(1);
	});
});

describe("overlappingZoomIds", () => {
	it("is empty when regions only touch at the boundary", () => {
		expect(overlappingZoomIds([R("a", 0, 4), R("b", 4, 8)])).toEqual([]);
	});

	it("reports both sides of an overlap", () => {
		expect(overlappingZoomIds([R("a", 0, 6), R("b", 4, 10)]).sort()).toEqual(["a", "b"]);
	});

	it("ignores hidden regions", () => {
		expect(overlappingZoomIds([R("a", 0, 6), R("b", 4, 10, true)])).toEqual([]);
	});

	it("reports a nested pair", () => {
		expect(overlappingZoomIds([R("a", 0, 10), R("b", 4, 6)]).sort()).toEqual(["a", "b"]);
	});
});
