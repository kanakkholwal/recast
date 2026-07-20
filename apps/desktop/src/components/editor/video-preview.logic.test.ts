import { describe, expect, it } from "vitest";
// Relative runtime import: no `$lib` alias in the standalone vitest config.
import { LINEAR } from "../../lib/easing/cubic-bezier";
import type { Easing } from "$lib/easing/cubic-bezier";
import type { ZoomRegion } from "$lib/stores/editor-store.svelte";
import {
	classifyMbError,
	evaluateZoomAt,
	idleAlphaAt,
	interpolateCursor,
	resolutionTier,
	type CursorSampleJS,
	type IdlePeriodJS,
} from "./video-preview.logic";

// A linear-eased region so the eased scale is a plain lerp, which locks the ramp
// shape (ramp-in → hold → ramp-out) without depending on a bezier's curvature.
function region(overrides: Partial<ZoomRegion> = {}): ZoomRegion {
	return {
		id: "z",
		start: 0,
		end: 4,
		scale: 2,
		easeIn: LINEAR,
		easeOut: LINEAR,
		rampIn: 1,
		rampOut: 1,
		centerX: 0.25,
		centerY: 0.75,
		motionBlur: 0.5,
		source: "manual",
		...overrides,
	};
}

describe("evaluateZoomAt", () => {
	it("is identity (scale 1, centre 0.5) outside every region", () => {
		const regions = [region()];
		expect(evaluateZoomAt(regions, -1)).toEqual({
			scale: 1,
			cx: 0.5,
			cy: 0.5,
			motionBlur: 0,
		});
		// At/after the boundaries the region is inactive (start/end exclusive).
		expect(evaluateZoomAt(regions, 0).scale).toBe(1);
		expect(evaluateZoomAt(regions, 4).scale).toBe(1);
	});

	it("ramps in linearly toward the target scale", () => {
		const z = evaluateZoomAt([region()], 0.5); // half-way through the 1s ramp-in
		expect(z.scale).toBeCloseTo(1.5, 5);
		expect(z.cx).toBe(0.25);
		expect(z.cy).toBe(0.75);
		expect(z.motionBlur).toBe(0.5);
	});

	it("holds at the full target scale between the ramps", () => {
		expect(evaluateZoomAt([region()], 2).scale).toBe(2);
	});

	it("ramps out linearly back toward 1", () => {
		// 0.5s before the end, half-way down the 1s ramp-out.
		expect(evaluateZoomAt([region()], 3.5).scale).toBeCloseTo(1.5, 5);
	});

	it("skips hidden regions", () => {
		expect(evaluateZoomAt([region({ hidden: true })], 2).scale).toBe(1);
	});
});

describe("interpolateCursor", () => {
	const samples: CursorSampleJS[] = [
		{ timestampUs: 0, x: 0, y: 0, visible: true, leftDown: false, rightDown: false },
		{ timestampUs: 1000, x: 100, y: 200, visible: false, leftDown: true, rightDown: false },
	];

	it("returns null with no samples", () => {
		expect(interpolateCursor([], null, 500)).toBeNull();
	});

	it("clamps to the first sample before the track starts", () => {
		expect(interpolateCursor(samples, null, -100)).toBe(samples[0]);
	});

	it("clamps to the last sample after the track ends", () => {
		expect(interpolateCursor(samples, null, 5000)).toBe(samples[1]);
	});

	it("returns an exact sample at its timestamp", () => {
		expect(interpolateCursor(samples, null, 1000)).toBe(samples[1]);
	});

	it("lerps position between two samples (no easing)", () => {
		const p = interpolateCursor(samples, null, 500)!;
		expect(p.x).toBeCloseTo(50, 5);
		expect(p.y).toBeCloseTo(100, 5);
		// tLinear === 0.5 is NOT < 0.5, so boolean states take the later sample.
		expect(p.visible).toBe(false);
		expect(p.leftDown).toBe(true);
	});

	it("flips boolean states at the midpoint of the linear param", () => {
		expect(interpolateCursor(samples, null, 250)!.leftDown).toBe(false);
		expect(interpolateCursor(samples, null, 750)!.leftDown).toBe(true);
	});

	it("reshapes the interpolation param with an easing but keeps endpoints", () => {
		const ease: Easing = { x1: 0.42, y1: 0, x2: 0.58, y2: 1 };
		const p = interpolateCursor(samples, ease, 500)!;
		// Symmetric ease is 0.5 at x=0.5, so position is unchanged at the midpoint.
		expect(p.x).toBeCloseTo(50, 3);
	});
});

describe("idleAlphaAt", () => {
	// One idle period [0, 2s]; timeout 0.5s → fade begins at 0.5s.
	const periods: IdlePeriodJS[] = [{ startUs: 0, endUs: 2_000_000 }];

	it("is fully visible before the idle threshold", () => {
		expect(idleAlphaAt(periods, 400_000, 0.5)).toBe(1);
	});

	it("ramps out over the fade window", () => {
		expect(idleAlphaAt(periods, 500_000, 0.5)).toBe(1); // fade start
		expect(idleAlphaAt(periods, 600_000, 0.5)).toBeCloseTo(0.5, 5); // mid
		expect(idleAlphaAt(periods, 700_000, 0.5)).toBe(0); // fully hidden
	});

	it("is fully hidden deep inside the idle span", () => {
		expect(idleAlphaAt(periods, 1_000_000, 0.5)).toBe(0);
	});

	it("ramps back in toward the end of the idle span", () => {
		expect(idleAlphaAt(periods, 1_900_000, 0.5)).toBeCloseTo(0.5, 5);
	});

	it("is fully visible again past the idle span", () => {
		expect(idleAlphaAt(periods, 2_100_000, 0.5)).toBe(1);
	});

	it("ignores a period shorter than the idle threshold", () => {
		const short: IdlePeriodJS[] = [{ startUs: 0, endUs: 400_000 }];
		expect(idleAlphaAt(short, 300_000, 0.5)).toBe(1);
	});
});

describe("resolutionTier", () => {
	it("buckets by the larger dimension", () => {
		expect(resolutionTier(1920, 1080)).toBe("1080p");
		expect(resolutionTier(1080, 1920)).toBe("1080p");
		expect(resolutionTier(3840, 2160)).toBe("4k");
		expect(resolutionTier(640, 480)).toBe("sd");
		expect(resolutionTier(5120, 2880)).toBe("5k");
		expect(resolutionTier(2560, 1440)).toBe("1440p");
		expect(resolutionTier(1280, 720)).toBe("720p");
	});
});

describe("classifyMbError", () => {
	it("maps messages to PII-safe reason codes", () => {
		expect(classifyMbError(new Error("worker unavailable"))).toBe("unsupported");
		expect(classifyMbError(new Error("no video track found"))).toBe("no_video_track");
		expect(classifyMbError(new Error("decoder config unsupported"))).toBe(
			"codec_unsupported",
		);
		expect(classifyMbError(new Error("fetch failed"))).toBe("fetch_failed");
		expect(classifyMbError("something else")).toBe("decode_error");
	});
});
