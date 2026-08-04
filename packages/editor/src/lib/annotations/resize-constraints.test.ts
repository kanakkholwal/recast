import { describe, expect, it } from "vitest";
import type { HandleName } from "./hit";
import {
	constrain45,
	constrainSquare,
	fitImageBox,
	isCornerHandle,
	lockAspect,
} from "./resize-constraints";

describe("isCornerHandle", () => {
	it("is true for the four corners only", () => {
		const corners: HandleName[] = ["nw", "ne", "se", "sw"];
		const edges: HandleName[] = ["n", "e", "s", "w", "body", "p1", "p2"];
		expect(corners.every(isCornerHandle)).toBe(true);
		expect(edges.some(isCornerHandle)).toBe(false);
	});
});

describe("constrainSquare", () => {
	it("produces equal on-screen pixels on a non-square frame", () => {
		// 1920x1080: a UV box should end up with w*1920 === h*1080.
		const { w, h } = constrainSquare(0.3, 0.1, 1920, 1080);
		expect(w * 1920).toBeCloseTo(h * 1080, 6);
	});

	it("sizes to the larger visual extent", () => {
		// width drives: 0.3*1920 = 576 vs 0.1*1080 = 108 → side = 576.
		const { w, h } = constrainSquare(0.3, 0.1, 1920, 1080);
		expect(w * 1920).toBeCloseTo(576, 6);
		expect(h * 1080).toBeCloseTo(576, 6);
	});

	it("preserves the sign of each axis (drag direction)", () => {
		const { w, h } = constrainSquare(-0.3, 0.1, 1920, 1080);
		expect(w).toBeLessThan(0);
		expect(h).toBeGreaterThan(0);
	});
});

describe("constrain45", () => {
	it("snaps a near-horizontal drag to exactly horizontal", () => {
		const r = constrain45(0.5, 0.5, 0.8, 0.52, 1000, 1000);
		expect(r.y).toBeCloseTo(0.5, 6);
		expect(r.x).toBeGreaterThan(0.5);
	});

	it("snaps a near-diagonal drag to 45° in visual space", () => {
		// Square frame → equal dx/dy after snapping to the diagonal.
		const r = constrain45(0, 0, 0.4, 0.45, 1000, 1000);
		expect(r.x).toBeCloseTo(r.y, 6);
	});
});

describe("lockAspect", () => {
	const start = { x: 0.2, y: 0.2, w: 0.4, h: 0.2 }; // 2:1

	it("keeps the starting aspect ratio when dragging the SE corner", () => {
		// Drag SE wider than tall; height should follow to hold 2:1.
		const out = lockAspect("se", start, 0.2, 0.2, 0.6, 0.1);
		expect(out.nw / out.nh).toBeCloseTo(2, 6);
		// SE keeps the top-left anchor fixed.
		expect(out.nx).toBeCloseTo(0.2, 6);
		expect(out.ny).toBeCloseTo(0.2, 6);
	});

	it("moves the anchored corner when dragging NW", () => {
		const out = lockAspect("nw", start, 0.1, 0.1, 0.5, 0.3);
		expect(out.nw / out.nh).toBeCloseTo(2, 6);
		// NW keeps the bottom-right (0.6, 0.4) fixed.
		expect(out.nx + out.nw).toBeCloseTo(0.6, 6);
		expect(out.ny + out.nh).toBeCloseTo(0.4, 6);
	});

	it("is a no-op for a degenerate start box", () => {
		const out = lockAspect("se", { x: 0, y: 0, w: 0, h: 0 }, 0, 0, 0.3, 0.1);
		expect(out).toEqual({ nx: 0, ny: 0, nw: 0.3, nh: 0.1 });
	});
});

describe("fitImageBox", () => {
	it("centers the box", () => {
		const box = fitImageBox({ w: 100, h: 100 }, 16 / 9);
		expect(box.x + box.w / 2).toBeCloseTo(0.5, 6);
		expect(box.y + box.h / 2).toBeCloseTo(0.5, 6);
	});

	it("keeps a square image square on-screen for a 16:9 frame", () => {
		const frameAspect = 16 / 9;
		const box = fitImageBox({ w: 200, h: 200 }, frameAspect);
		// On-screen pixel aspect = (w*frameW)/(h*frameH) = (w/h)*frameAspect === 1.
		expect((box.w / box.h) * frameAspect).toBeCloseTo(1, 6);
	});

	it("falls back to a square-ish target when natural size is unknown", () => {
		const box = fitImageBox(null, 16 / 9);
		expect(box.w).toBeCloseTo(0.4, 6);
		expect(box.h).toBeCloseTo(0.4, 6);
	});

	it("never exceeds the target extent", () => {
		const wide = fitImageBox({ w: 800, h: 100 }, 16 / 9);
		expect(wide.w).toBeLessThanOrEqual(0.4 + 1e-9);
		expect(wide.h).toBeLessThanOrEqual(0.4 + 1e-9);
	});
});
