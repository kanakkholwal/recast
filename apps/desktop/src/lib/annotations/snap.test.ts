import { describe, expect, it } from "vitest";
import { FRAME_ANCHORS, snap, snapBox, type SnapAnchor } from "./snap";

describe("snap (single point)", () => {
	it("snaps to the nearest anchor within tolerance", () => {
		const r = snap(0.503, 0.2, FRAME_ANCHORS, 0.01);
		expect(r.x).toBe(0.5);
		expect(r.snapped).toBe(true);
	});

	it("leaves the point alone outside tolerance", () => {
		const r = snap(0.53, 0.2, FRAME_ANCHORS, 0.01);
		expect(r.x).toBe(0.53);
	});

	it("is a no-op when disabled", () => {
		const r = snap(0.501, 0.501, FRAME_ANCHORS, 0.01, false);
		expect(r).toEqual({ x: 0.501, y: 0.501, guides: [], snapped: false });
	});
});

describe("snapBox (move)", () => {
	it("snaps the box's LEFT edge to the frame-left anchor", () => {
		// left edge at 0.004 → snaps to 0 (frame-left), whole box shifts by -0.004.
		const res = snapBox(0.004, 0.3, 0.2, 0.1, FRAME_ANCHORS, 0.01);
		expect(res.x).toBeCloseTo(0, 6);
		expect(res.snapped).toBe(true);
	});

	it("snaps the box's CENTER to the frame center", () => {
		// center x at 0.5 - 0.003; box w=0.2 → left ~0.397. Center should snap to 0.5.
		const res = snapBox(0.397, 0.1, 0.2, 0.2, FRAME_ANCHORS, 0.01);
		expect(res.x + 0.2 / 2).toBeCloseTo(0.5, 6);
	});

	it("snaps the RIGHT edge to the frame-right anchor", () => {
		// right edge at 0.996 → snaps to 1; left shifts to 0.8.
		const res = snapBox(0.796, 0.3, 0.2, 0.1, FRAME_ANCHORS, 0.01);
		expect(res.x + 0.2).toBeCloseTo(1, 6);
	});

	it("does not move the box when no edge is within tolerance", () => {
		// Edges 0.31/0.385/0.46 and 0.31/0.355/0.4 miss every frame anchor.
		const res = snapBox(0.31, 0.31, 0.15, 0.09, FRAME_ANCHORS, 0.005);
		expect(res.x).toBe(0.31);
		expect(res.y).toBe(0.31);
		expect(res.snapped).toBe(false);
	});

	it("snaps to another annotation's edge, not just the frame", () => {
		const anchors: SnapAnchor[] = [{ axis: "x", value: 0.42 }];
		// Box left at 0.417 → snaps to the peer edge 0.42.
		const res = snapBox(0.417, 0.1, 0.1, 0.1, anchors, 0.01);
		expect(res.x).toBeCloseTo(0.42, 6);
		expect(res.guides).toHaveLength(1);
	});

	it("is a no-op when disabled", () => {
		const res = snapBox(0.004, 0.004, 0.2, 0.2, FRAME_ANCHORS, 0.01, false);
		expect(res).toEqual({ x: 0.004, y: 0.004, guides: [], snapped: false });
	});
});
