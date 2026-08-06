import { describe, expect, it } from "vitest";
import { cursorForHandle, handlePositions, hitTestHandle, regionBox } from "./focus-overlay.logic";

describe("regionBox", () => {
	it("centres a 1/scale square and clamps inside the frame", () => {
		// scale 2 → side 0.5, centred → [0.25, 0.75]
		expect(regionBox({ scale: 2, centerX: 0.5, centerY: 0.5 })).toEqual({
			x: 0.25,
			y: 0.25,
			w: 0.5,
			h: 0.5,
		});
	});
	it("pushes an off-frame centre back inside", () => {
		const b = regionBox({ scale: 2, centerX: 0, centerY: 1 });
		expect(b.x).toBeCloseTo(0); // clamped to w/2 - w/2
		expect(b.y).toBeCloseTo(0.5);
	});
});

describe("handlePositions", () => {
	it("places the 8 anchors around the rect", () => {
		const h = handlePositions(0, 0, 10, 20);
		expect(h.nw).toEqual({ x: 0, y: 0 });
		expect(h.se).toEqual({ x: 10, y: 20 });
		expect(h.n).toEqual({ x: 5, y: 0 });
		expect(h.e).toEqual({ x: 10, y: 10 });
	});
});

describe("hitTestHandle", () => {
	it("detects a corner handle within slop", () => {
		expect(hitTestHandle({ x: 0, y: 0 }, 0, 0, 100, 100, 1)).toBe("nw");
	});
	it("returns body for an interior point", () => {
		expect(hitTestHandle({ x: 50, y: 50 }, 0, 0, 100, 100, 1)).toBe("body");
	});
	it("returns null outside the rect", () => {
		expect(hitTestHandle({ x: 200, y: 200 }, 0, 0, 100, 100, 1)).toBeNull();
	});
});

describe("cursorForHandle", () => {
	it("maps handles to resize cursors", () => {
		expect(cursorForHandle("nw")).toBe("nwse-resize");
		expect(cursorForHandle("e")).toBe("ew-resize");
		expect(cursorForHandle("body")).toBe("move");
		expect(cursorForHandle(null)).toBe("");
	});
});
