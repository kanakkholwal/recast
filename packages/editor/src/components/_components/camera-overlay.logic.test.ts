import { describe, expect, it } from "vitest";
import {
	applyZoomFollow,
	CAMERA_SHADOW_BLUR_FRACTION,
	CAMERA_SHADOW_MAX_OPACITY,
	CAMERA_SHADOW_OFFSET_FRACTION,
	cameraFollowScaleAt,
	cameraPlacementAt,
	cameraShadowStyle,
	keyframesFromMotionSegments,
	MAX_CAMERA_SIZE,
	MIN_CAMERA_SIZE,
	resizeCameraSquare,
	upsertCameraKeyframe,
	type CameraKeyframe,
} from "./camera-overlay.logic";

const base = { x: 0.72, y: 0.08, width: 0.22, height: 0.22 };

describe("resizeCameraSquare", () => {
	it("keeps the diagonally-opposite corner fixed (drag bottom-right)", () => {
		// br handle → top-left (0.72,0.08) stays put; drag out to grow.
		const r = resizeCameraSquare(base, "br", 0.95, 0.4, 1);
		expect(r.x).toBeCloseTo(0.72, 6);
		expect(r.y).toBeCloseTo(0.08, 6);
		expect(r.width).toBe(r.height); // square in UV at aspect 1
		// Wants 0.32 but caps at the room to the right edge (1 - 0.72 = 0.28).
		expect(r.width).toBeCloseTo(0.28, 6);
	});

	it("anchors the far corner when dragging top-left", () => {
		// tl handle → bottom-right corner (0.94, 0.30) fixed.
		const r = resizeCameraSquare(base, "tl", 0.6, 0.1, 1);
		const brX = base.x + base.width; // 0.94
		const brY = base.y + base.height; // 0.30
		expect(r.x + r.width).toBeCloseTo(brX, 6);
		expect(r.y + r.height).toBeCloseTo(brY, 6);
	});

	it("clamps to the min/max size and never leaves the frame", () => {
		expect(resizeCameraSquare(base, "br", 0.73, 0.081, 1).width).toBeCloseTo(MIN_CAMERA_SIZE, 6);
		const big = resizeCameraSquare({ x: 0.1, y: 0.1, width: 0.2, height: 0.2 }, "br", 5, 5, 1);
		expect(big.width).toBeLessThanOrEqual(MAX_CAMERA_SIZE + 1e-9);
		expect(big.x + big.width).toBeLessThanOrEqual(1 + 1e-9);
		expect(big.y + big.height).toBeLessThanOrEqual(1 + 1e-9);
	});

	it("keeps the bubble square in PIXELS on a wide video (height = width*aspect)", () => {
		const aspect = 16 / 9;
		// tl anchor = bottom-right corner (0.94, 0.08 + 0.22*aspect). Drag the
		// top-left inward; the result must stay square in pixels.
		const r = resizeCameraSquare(base, "br", 0.86, 0.5, aspect);
		expect(r.height).toBeCloseTo(r.width * aspect, 6);
		// Anchor (top-left) stays fixed.
		expect(r.x).toBeCloseTo(0.72, 6);
		expect(r.y).toBeCloseTo(0.08, 6);
		expect(r.y + r.height).toBeLessThanOrEqual(1 + 1e-9);
	});
});

describe("cameraPlacementAt", () => {
	const p = (x: number) => ({ x, y: 0.1, width: 0.2, height: 0.2 });
	const kfs: CameraKeyframe[] = [
		{ atSec: 1, placement: p(0.1) },
		{ atSec: 3, placement: p(0.7) },
	];

	it("returns the static base when there are no keyframes", () => {
		expect(cameraPlacementAt(p(0.5), [], 2)).toEqual(p(0.5));
	});

	it("holds at the first/last keyframe outside the range", () => {
		expect(cameraPlacementAt(p(0.5), kfs, 0).x).toBeCloseTo(0.1, 6);
		expect(cameraPlacementAt(p(0.5), kfs, 5).x).toBeCloseTo(0.7, 6);
	});

	it("glides (eased) between keyframes — midpoint is the halfway position", () => {
		// smoothstep(0.5) = 0.5, so the midpoint x = (0.1+0.7)/2 = 0.4.
		expect(cameraPlacementAt(p(0.5), kfs, 2).x).toBeCloseTo(0.4, 6);
	});

	it("eases in near the start (slower than linear)", () => {
		// At 25% through, smoothstep(0.25)=0.15625 → x = 0.1 + 0.6*0.15625 = 0.19375.
		expect(cameraPlacementAt(p(0.5), kfs, 1.5).x).toBeCloseTo(0.19375, 6);
	});

	it("uses the supplied easing (LINEAR mirrors the Rust parity test)", () => {
		const linear = { x1: 0, y1: 0, x2: 1, y2: 1 };
		expect(cameraPlacementAt(p(0.5), kfs, 2, linear).x).toBeCloseTo(0.4, 6); // mid
		expect(cameraPlacementAt(p(0.5), kfs, 1.5, linear).x).toBeCloseTo(0.25, 6); // quarter
	});
});

describe("upsertCameraKeyframe", () => {
	const base: CameraKeyframe[] = [
		{ atSec: 1, placement: { x: 0.1, y: 0, width: 0.2, height: 0.2 } },
		{ atSec: 3, placement: { x: 0.7, y: 0, width: 0.2, height: 0.2 } },
	];

	it("inserts a new keyframe in sorted order", () => {
		const r = upsertCameraKeyframe(base, 2, { x: 0.4, y: 0, width: 0.2, height: 0.2 });
		expect(r.map((k) => k.atSec)).toEqual([1, 2, 3]);
	});

	it("replaces a keyframe within epsilon of an existing time", () => {
		const r = upsertCameraKeyframe(base, 3.01, { x: 0.9, y: 0, width: 0.2, height: 0.2 });
		expect(r).toHaveLength(2);
		expect(r[1].placement.x).toBeCloseTo(0.9, 6);
	});
});

describe("cameraShadowStyle", () => {
	it("is 'none' at or below zero strength", () => {
		expect(cameraShadowStyle(0)).toBe("none");
		expect(cameraShadowStyle(-1)).toBe("none");
	});

	it("scales blur, offset, and opacity by strength in cqmin (export-parity fractions)", () => {
		const s = 0.5;
		const style = cameraShadowStyle(s);
		const blur = (CAMERA_SHADOW_BLUR_FRACTION * s * 100).toFixed(2);
		const offset = (CAMERA_SHADOW_OFFSET_FRACTION * s * 100).toFixed(2);
		const opacity = (CAMERA_SHADOW_MAX_OPACITY * s).toFixed(3);
		expect(style).toBe(`0 ${offset}cqmin ${blur}cqmin rgba(0,0,0,${opacity})`);
	});
});

describe("cameraFollowScaleAt", () => {
	// Region 0..10s, peak 2×, focus (0.3,0.7). Mirrors the Rust parity test.
	const region = {
		start: 0,
		end: 10,
		scale: 2,
		rampIn: 0.4,
		rampOut: 0.4,
		easeIn: { x1: 0.42, y1: 0, x2: 0.58, y2: 1 },
		easeOut: { x1: 0.42, y1: 0, x2: 0.58, y2: 1 },
		centerX: 0.3,
		centerY: 0.7,
		hidden: false,
		motionBlur: 0,
		// biome-ignore lint/suspicious/noExplicitAny: test fixture is a partial ZoomRegion
	} as any;
	const linear = { x1: 0, y1: 0, x2: 1, y2: 1 };

	it("is identity outside any region", () => {
		expect(cameraFollowScaleAt([region], -1, 1, linear)).toEqual({ scale: 1, cx: 0.5, cy: 0.5 });
	});

	it("ramps on its own duration/easing (linear midpoint = half grow)", () => {
		// duration 1s, t=0.5 → activation 0.5 → scale 1 + 0.5*(2-1) = 1.5.
		const r = cameraFollowScaleAt([region], 0.5, 1, linear);
		expect(r.scale).toBeCloseTo(1.5, 6);
		expect(r.cx).toBeCloseTo(0.3, 6);
		expect(r.cy).toBeCloseTo(0.7, 6);
	});

	it("reaches full peak during the hold", () => {
		expect(cameraFollowScaleAt([region], 5, 1, linear).scale).toBeCloseTo(2, 6);
	});
});

describe("applyZoomFollow", () => {
	it("is the identity at rest, disabled, or zero strength", () => {
		expect(
			applyZoomFollow(base, { scale: 1, cx: 0.5, cy: 0.5 }, { enabled: true, strength: 0.6 }),
		).toEqual(base);
		expect(
			applyZoomFollow(base, { scale: 1.8, cx: 0.5, cy: 0.5 }, { enabled: false, strength: 0.6 }),
		).toEqual(base);
		expect(
			applyZoomFollow(base, { scale: 1.8, cx: 0.5, cy: 0.5 }, { enabled: true, strength: 0 }),
		).toEqual(base);
	});

	it("grows the bubble as the zoom ramps", () => {
		const r = applyZoomFollow(
			base,
			{ scale: 1.5, cx: 0.2, cy: 0.8 },
			{ enabled: true, strength: 1 },
		);
		// grow = 1 + (1.5-1)*1 = 1.5 → width 0.22*1.5 = 0.33
		expect(r.width).toBeCloseTo(0.33, 6);
		expect(r.width).toBe(r.height);
	});

	it("drifts AWAY from the zoom focus (with room to move)", () => {
		// Central bubble so growth + drift aren't clamped by an edge.
		const mid = { x: 0.4, y: 0.4, width: 0.15, height: 0.15 };
		const r = applyZoomFollow(
			mid,
			{ scale: 1.3, cx: 0.1, cy: 0.1 },
			{ enabled: true, strength: 1 },
		);
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

	it("derives the grown height from width*aspect on a wide video", () => {
		const aspect = 16 / 9;
		const mid = { x: 0.4, y: 0.4, width: 0.15, height: 0.15 };
		const r = applyZoomFollow(
			mid,
			{ scale: 1.5, cx: 0.1, cy: 0.1 },
			{ enabled: true, strength: 1 },
			aspect,
		);
		expect(r.width).toBeCloseTo(0.225, 6); // 0.15 * 1.5
		expect(r.height).toBeCloseTo(r.width * aspect, 6); // square in pixels
	});
});

describe("keyframesFromMotionSegments", () => {
	const EASE_LIN = { x1: 0, y1: 0, x2: 1, y2: 1 };
	const at = (t: number, x: number) => ({
		start: t,
		end: t + 1,
		fromX: x,
		fromY: 0,
		fromWidth: 0.2,
		fromHeight: 0.2,
		toX: x + 0.1,
		toY: 0,
		toWidth: 0.2,
		toHeight: 0.2,
		easeIn: EASE_LIN,
		easeOut: EASE_LIN,
		source: "live-recorded" as const,
	});
	const dflt = { x: 0.1, y: 0, width: 0.2, height: 0.2 };

	it("returns nothing for a recording with no moves", () => {
		expect(keyframesFromMotionSegments([], dflt)).toEqual([]);
	});

	it("turns each move into its two endpoints", () => {
		const kfs = keyframesFromMotionSegments([at(2, 0.1)], dflt);
		expect(kfs.map((k) => k.atSec)).toEqual([2, 3]);
		expect(kfs[0].placement.x).toBeCloseTo(0.1, 6);
		expect(kfs[1].placement.x).toBeCloseTo(0.2, 6);
	});

	it("reproduces the recorded path when evaluated", () => {
		// Two moves with a pause between: 0.1 → 0.2 over [2,3], hold, then
		// 0.2 → 0.3 over [5,6]. This is what the segment walk used to describe
		// and nothing rendered.
		const kfs = keyframesFromMotionSegments([at(2, 0.1), at(5, 0.2)], dflt);
		expect(cameraPlacementAt(dflt, kfs, 0, EASE_LIN).x).toBeCloseTo(0.1, 6);
		expect(cameraPlacementAt(dflt, kfs, 2.5, EASE_LIN).x).toBeCloseTo(0.15, 6);
		expect(cameraPlacementAt(dflt, kfs, 4, EASE_LIN).x).toBeCloseTo(0.2, 6);
		expect(cameraPlacementAt(dflt, kfs, 5.5, EASE_LIN).x).toBeCloseTo(0.25, 6);
		expect(cameraPlacementAt(dflt, kfs, 99, EASE_LIN).x).toBeCloseTo(0.3, 6);
	});

	it("collapses moves that meet at one instant, later wins", () => {
		const kfs = keyframesFromMotionSegments([at(2, 0.1), at(3, 0.2)], dflt);
		expect(kfs.map((k) => k.atSec)).toEqual([2, 3, 4]);
		expect(kfs[1].placement.x).toBeCloseTo(0.2, 6);
	});

	it("pins the resting placement only when the first move starts elsewhere", () => {
		const moved = keyframesFromMotionSegments([at(2, 0.5)], dflt);
		expect(moved[0]).toEqual({ atSec: 0, placement: dflt });
		// First move starts from the resting spot: no redundant keyframe.
		const same = keyframesFromMotionSegments([at(2, dflt.x)], dflt);
		expect(same[0].atSec).toBe(2);
	});
});
