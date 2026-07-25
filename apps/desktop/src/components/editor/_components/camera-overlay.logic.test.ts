import { describe, expect, it } from "vitest";
import {
	applyZoomFollow,
	MAX_CAMERA_SIZE,
	MIN_CAMERA_SIZE,
	resizeCameraSquare,
} from "./camera-overlay.logic";

const base = { x: 0.72, y: 0.08, width: 0.22, height: 0.22 };

describe("resizeCameraSquare", () => {
	it("keeps the diagonally-opposite corner fixed (drag bottom-right)", () => {
		// br handle → top-left (0.72,0.08) stays put; drag out to grow.
		const r = resizeCameraSquare(base, "br", 0.95, 0.4);
		expect(r.x).toBeCloseTo(0.72, 6);
		expect(r.y).toBeCloseTo(0.08, 6);
		expect(r.width).toBe(r.height); // square
		// Wants 0.32 but caps at the room to the right edge (1 - 0.72 = 0.28).
		expect(r.width).toBeCloseTo(0.28, 6);
	});

	it("anchors the far corner when dragging top-left", () => {
		// tl handle → bottom-right corner (0.94, 0.30) fixed.
		const r = resizeCameraSquare(base, "tl", 0.6, 0.1);
		const brX = base.x + base.width; // 0.94
		const brY = base.y + base.height; // 0.30
		expect(r.x + r.width).toBeCloseTo(brX, 6);
		expect(r.y + r.height).toBeCloseTo(brY, 6);
	});

	it("clamps to the min/max size and never leaves the frame", () => {
		expect(resizeCameraSquare(base, "br", 0.73, 0.081).width).toBeCloseTo(MIN_CAMERA_SIZE, 6);
		const big = resizeCameraSquare({ x: 0.1, y: 0.1, width: 0.2, height: 0.2 }, "br", 5, 5);
		expect(big.width).toBeLessThanOrEqual(MAX_CAMERA_SIZE + 1e-9);
		expect(big.x + big.width).toBeLessThanOrEqual(1 + 1e-9);
		expect(big.y + big.height).toBeLessThanOrEqual(1 + 1e-9);
	});
});

describe("applyZoomFollow", () => {
	it("is the identity at rest, disabled, or zero strength", () => {
		expect(applyZoomFollow(base, { scale: 1, cx: 0.5, cy: 0.5 }, { enabled: true, strength: 0.6 })).toEqual(base);
		expect(applyZoomFollow(base, { scale: 1.8, cx: 0.5, cy: 0.5 }, { enabled: false, strength: 0.6 })).toEqual(base);
		expect(applyZoomFollow(base, { scale: 1.8, cx: 0.5, cy: 0.5 }, { enabled: true, strength: 0 })).toEqual(base);
	});

	it("grows the bubble as the zoom ramps", () => {
		const r = applyZoomFollow(base, { scale: 1.5, cx: 0.2, cy: 0.8 }, { enabled: true, strength: 1 });
		// grow = 1 + (1.5-1)*1 = 1.5 → width 0.22*1.5 = 0.33
		expect(r.width).toBeCloseTo(0.33, 6);
		expect(r.width).toBe(r.height);
	});

	it("drifts AWAY from the zoom focus (with room to move)", () => {
		// Central bubble so growth + drift aren't clamped by an edge.
		const mid = { x: 0.4, y: 0.4, width: 0.15, height: 0.15 };
		const r = applyZoomFollow(mid, { scale: 1.3, cx: 0.1, cy: 0.1 }, { enabled: true, strength: 1 });
		const before = { cx: mid.x + mid.width / 2, cy: mid.y + mid.height / 2 };
		const after = { cx: r.x + r.width / 2, cy: r.y + r.height / 2 };
		expect(Math.hypot(after.cx - 0.1, after.cy - 0.1)).toBeGreaterThan(
			Math.hypot(before.cx - 0.1, before.cy - 0.1),
		);
	});

	it("keeps the grown+drifted bubble fully inside the frame", () => {
		for (const cx of [0, 0.5, 1]) {
			const r = applyZoomFollow(base, { scale: 2.5, cx, cy: cx }, { enabled: true, strength: 1 });
			expect(r.x).toBeGreaterThanOrEqual(0);
			expect(r.y).toBeGreaterThanOrEqual(0);
			expect(r.x + r.width).toBeLessThanOrEqual(1 + 1e-9);
			expect(r.y + r.height).toBeLessThanOrEqual(1 + 1e-9);
		}
	});
});
