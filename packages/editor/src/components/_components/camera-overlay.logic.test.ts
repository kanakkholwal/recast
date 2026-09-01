import { describe, expect, it } from "vitest";
import {
	applyZoomFollow,
	CAMERA_SHADOW_BLUR_FRACTION,
	CAMERA_SHADOW_MAX_OPACITY,
	CAMERA_SHADOW_OFFSET_FRACTION,
	type CameraKeyframe,
	cameraBubbleDelta,
	cameraFollowScaleAt,
	cameraPlacementAt,
	cameraShadowStyle,
	clampPlacement,
	keyframesFromMotionSegments,
	MAX_CAMERA_SIZE,
	MAX_RECORDED_MOVE_SECS,
	MIN_CAMERA_SIZE,
	resizeCameraSquare,
	upsertCameraKeyframe,
} from "./camera-overlay.logic";
import placementCases from "../../../../../fixtures/camera-placement.json";

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
		// The tl anchor is the bottom-right corner: drag the top-left inward and the result must stay square in pixels.
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
	const existing: CameraKeyframe[] = [
		{ atSec: 1, placement: { x: 0.1, y: 0, width: 0.2, height: 0.2 } },
		{ atSec: 3, placement: { x: 0.7, y: 0, width: 0.2, height: 0.2 } },
	];

	it("inserts a new keyframe in sorted order", () => {
		const r = upsertCameraKeyframe(existing, 2, { x: 0.4, y: 0, width: 0.2, height: 0.2 });
		expect(r.map((k) => k.atSec)).toEqual([1, 2, 3]);
	});

	it("replaces a keyframe within epsilon of an existing time", () => {
		const r = upsertCameraKeyframe(existing, 3.01, { x: 0.9, y: 0, width: 0.2, height: 0.2 });
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

	it("drifts along the SCREEN direction, not the UV one, on a wide frame (D-2)", () => {
		const aspect = 16 / 9;
		const b = { x: 0.425, y: 0.267, width: 0.15, height: 0.15 * aspect };
		const zoom = { scale: 1.5, cx: 0.3, cy: 0.3 };
		const r = applyZoomFollow(b, zoom, { enabled: true, strength: 1 }, aspect);
		const baseH = b.width * aspect;
		const bc = { x: b.x + b.width / 2, y: b.y + baseH / 2 };
		// Both vectors in screen pixels (videoH as the unit).
		const away = [(bc.x - zoom.cx) * aspect, bc.y - zoom.cy];
		const drift = [(r.x + r.width / 2 - bc.x) * aspect, r.y + r.height / 2 - bc.y];
		const mag = Math.hypot(away[0], away[1]) * Math.hypot(drift[0], drift[1]);
		expect(mag).toBeGreaterThan(1e-9);
		// Collinear (cross ~ 0) and pointing away (dot > 0).
		expect(Math.abs((away[0] * drift[1] - away[1] * drift[0]) / mag)).toBeLessThan(1e-3);
		expect(away[0] * drift[0] + away[1] * drift[1]).toBeGreaterThan(0);
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
		// Two moves with a pause between, which is what the segment walk used to describe while nothing rendered.
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

describe("clampPlacement", () => {
	/** A live capture wrote `x: 1` with a 0.16-wide bubble, which draws the whole
	 *  thing past the right edge. The drag path clamps; recorded data did not. */
	it("pulls a placement that starts off the right edge back inside", () => {
		expect(clampPlacement({ x: 1, y: 0.86, width: 0.16, height: 0.29 })).toEqual({
			x: 0.84,
			y: 0.71,
			width: 0.16,
			height: 0.29,
		});
	});

	it("leaves a placement that already fits alone", () => {
		const p = { x: 0.2, y: 0.3, width: 0.16, height: 0.29 };
		expect(clampPlacement(p)).toEqual(p);
	});

	it("clamps a negative origin to the top-left", () => {
		expect(clampPlacement({ x: -0.4, y: -0.1, width: 0.2, height: 0.2 })).toEqual({
			x: 0,
			y: 0,
			width: 0.2,
			height: 0.2,
		});
	});
});

describe("recorded motion segments", () => {
	it("clamps the recorded endpoints rather than gliding off-canvas", () => {
		const frames = keyframesFromMotionSegments(
			[
				{
					start: 0,
					end: 10,
					fromX: 1,
					fromY: 0.86,
					fromWidth: 0.16,
					fromHeight: 0.29,
					toX: 0.79,
					toY: 0.5,
					toWidth: 0.16,
					toHeight: 0.29,
					easeIn: { x1: 0.25, y1: 0.1, x2: 0.25, y2: 1 },
					easeOut: { x1: 0.25, y1: 0.1, x2: 0.25, y2: 1 },
				},
			],
			{ x: 0.5, y: 0.5, width: 0.16, height: 0.29 },
		);
		expect(frames[0].placement.x).toBeCloseTo(0.84, 6);
		expect(frames[0].placement.y).toBeCloseTo(0.71, 6);
	});
});

describe("cameraBubbleDelta", () => {
	const at = (x: number, y: number, width = 0.2) => ({ x, y, width, height: width });

	it("is the identity when the bubble sits on its layout box", () => {
		const d = cameraBubbleDelta(at(0.4, 0.4), at(0.4, 0.4), 1);
		expect(d).toEqual({ tx: 0, ty: 0, scale: 1 });
	});

	/** The whole point of freezing the layout box: wherever it sits, the delta
	 *  has to put the bubble at the same place on screen. */
	it("lands the bubble in the same place from any layout box", () => {
		const drawn = at(0.7, 0.25);
		const a = cameraBubbleDelta(at(0.1, 0.1), drawn, 1);
		const b = cameraBubbleDelta(at(0.5, 0.5), drawn, 1);
		const screenX = (layoutX: number, tx: number) => layoutX + (tx / 100) * 0.2;
		expect(screenX(0.1, a.tx)).toBeCloseTo(drawn.x, 9);
		expect(screenX(0.5, b.tx)).toBeCloseTo(drawn.x, 9);
	});

	/** A percentage translate on the Y axis resolves against the element's
	 *  HEIGHT, which is square in pixels, not in UV. */
	it("scales the vertical translate by the bubble's UV height", () => {
		const wide = cameraBubbleDelta(at(0.2, 0.2), at(0.2, 0.3), 16 / 9);
		const square = cameraBubbleDelta(at(0.2, 0.2), at(0.2, 0.3), 1);
		expect(wide.ty).toBeLessThan(square.ty);
	});

	it("reports a grow as a scale rather than a size change", () => {
		const d = cameraBubbleDelta(at(0.2, 0.2, 0.2), at(0.2, 0.2, 0.3), 1);
		expect(d.scale).toBeCloseTo(1.5, 9);
	});

	it("refuses to divide by a zero-width layout box", () => {
		expect(cameraBubbleDelta(at(0.2, 0.2, 0), at(0.5, 0.5), 1)).toEqual({
			tx: 0,
			ty: 0,
			scale: 1,
		});
	});
});

describe("repairing an over-long recorded move", () => {
	const glide = (start: number, end: number) => ({
		start,
		end,
		fromX: 0.2,
		fromY: 0.2,
		fromWidth: 0.16,
		fromHeight: 0.29,
		toX: 0.6,
		toY: 0.5,
		toWidth: 0.16,
		toHeight: 0.29,
		easeIn: { x1: 0.25, y1: 0.1, x2: 0.25, y2: 1 },
		easeOut: { x1: 0.25, y1: 0.1, x2: 0.25, y2: 1 },
	});
	const startPlacement = { x: 0.2, y: 0.2, width: 0.16, height: 0.29 };

	/** Projects that recorded before the dead zone landed carry one segment for
	 *  the whole take. Replaying it verbatim drifts the bubble across the entire
	 *  video, which is what the file says and not what happened. */
	it("holds the start placement instead of gliding for the whole recording", () => {
		const frames = keyframesFromMotionSegments([glide(0.15, 153.47)], startPlacement);
		const times = frames.map((f) => f.atSec);
		expect(times[times.length - 1]).toBeCloseTo(153.47, 6);
		expect(times[times.length - 2]).toBeCloseTo(153.47 - MAX_RECORDED_MOVE_SECS, 6);
		// Held at the start placement until the move begins.
		expect(cameraPlacementAt(startPlacement, frames, 60).x).toBeCloseTo(0.2, 6);
		expect(cameraPlacementAt(startPlacement, frames, 153.47).x).toBeCloseTo(0.6, 6);
	});

	it("leaves a move short enough to be a real drag alone", () => {
		const frames = keyframesFromMotionSegments([glide(1, 3)], startPlacement);
		expect(frames.map((f) => f.atSec)).toEqual([1, 3]);
	});
});

// Shared with `crates/recast-compositor/src/camera.rs`: the previewed bubble and the burned-in one were three implementations, and D-2 was fixed in only one.
describe("zoom-follow parity with the Rust compositor", () => {
	interface Case {
		base: { x: number; y: number; width: number; height: number };
		scale: number;
		cx: number;
		cy: number;
		strength: number;
		aspect: number;
		expect: { x: number; y: number; width: number; height: number };
	}

	it("has enough cases to catch a drift", () => {
		expect((placementCases as Case[]).length).toBeGreaterThanOrEqual(8);
	});

	for (const [i, c] of (placementCases as Case[]).entries()) {
		it(`case ${i}: scale ${c.scale} focus (${c.cx},${c.cy}) strength ${c.strength}`, () => {
			const got = applyZoomFollow(
				c.base,
				{ scale: c.scale, cx: c.cx, cy: c.cy },
				{ enabled: true, strength: c.strength },
				c.aspect,
			);
			expect(got.x).toBeCloseTo(c.expect.x, 12);
			expect(got.y).toBeCloseTo(c.expect.y, 12);
			expect(got.width).toBeCloseTo(c.expect.width, 12);
			expect(got.height).toBeCloseTo(c.expect.height, 12);
		});
	}
});
