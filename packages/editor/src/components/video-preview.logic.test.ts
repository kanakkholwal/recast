import { describe, expect, it } from "vitest";
import type { Easing } from "../lib/easing/cubic-bezier";
// Relative runtime import: no `$lib` alias in the standalone vitest config.
import { LINEAR } from "../lib/easing/cubic-bezier";
import type { ZoomRegion } from "../stores/editor-store.svelte";
import {
	cameraStall,
	type CursorSampleJS,
	classifyMbError,
	evaluateZoomAt,
	type IdlePeriodJS,
	idleAlphaAt,
	interpolateCursor,
	resolutionTier,
	shouldRecoverMbSource,
	zoomScaleAt,
} from "./video-preview.logic";
import zoomCases from "../../../../fixtures/zoom-scale.json";

// A linear-eased region makes the scale a plain lerp, locking the ramp shape without depending on a bezier's curvature.
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

describe("shouldRecoverMbSource", () => {
	it("rebuilds on a transient GPU-reset class failure", () => {
		expect(shouldRecoverMbSource("internal", 0)).toBe(true);
		expect(shouldRecoverMbSource("worker-died", 0)).toBe(true);
		expect(shouldRecoverMbSource("decode-failed", 0)).toBe(true);
	});

	it("degrades permanently on an unrecoverable file/codec failure", () => {
		expect(shouldRecoverMbSource("unsupported", 0)).toBe(false);
		expect(shouldRecoverMbSource("bad-input", 0)).toBe(false);
		expect(shouldRecoverMbSource("too-large", 0)).toBe(false);
	});

	it("stops retrying once the attempt budget is spent", () => {
		expect(shouldRecoverMbSource("internal", 2)).toBe(true); // 3rd attempt
		expect(shouldRecoverMbSource("internal", 3)).toBe(false); // exhausted
		expect(shouldRecoverMbSource("internal", 1, 1)).toBe(false); // custom cap
	});
});

describe("classifyMbError", () => {
	it("maps messages to PII-safe reason codes", () => {
		expect(classifyMbError(new Error("worker unavailable"))).toBe("unsupported");
		expect(classifyMbError(new Error("worker script failed to load: /x.js"))).toBe("worker_failed");
		expect(classifyMbError(new Error("no video track found"))).toBe("no_video_track");
		expect(classifyMbError(new Error("decoder config unsupported"))).toBe("codec_unsupported");
		expect(classifyMbError(new Error("fetch failed"))).toBe("fetch_failed");
		expect(classifyMbError("something else")).toBe("decode_error");
	});
});

describe("overlapping zoom regions", () => {
	const overlapping = (over: Record<string, unknown>) =>
		({
			id: "z",
			start: 0,
			end: 10,
			scale: 2,
			rampIn: 0.5,
			rampOut: 0.5,
			centerX: 0.5,
			centerY: 0.5,
			easeIn: { x1: 0.25, y1: 0.1, x2: 0.25, y2: 1 },
			easeOut: { x1: 0.25, y1: 0.1, x2: 0.25, y2: 1 },
			motionBlur: 0,
			...over,
		}) as never;

	/** Latest-start-wins handed over at the incoming overlapping's ramp START, which
	 *  snapped the zoom to ~1 for a frame. Everything riding it flickered. */
	it("hands over without a step", () => {
		const regions = [
			overlapping({ start: 1, end: 8, scale: 2 }),
			overlapping({ start: 5, end: 12, scale: 2.5 }),
		];
		let previous: number | null = null;
		for (let t = 0.5; t < 12.5; t += 1 / 60) {
			const { scale } = evaluateZoomAt(regions, t);
			if (previous !== null) expect(Math.abs(scale - previous)).toBeLessThanOrEqual(0.15);
			previous = scale;
		}
	});

	it("keeps the tighter overlapping in force", () => {
		const regions = [
			overlapping({ start: 0, end: 10, scale: 3, rampIn: 0, rampOut: 0 }),
			overlapping({ start: 4, end: 6, scale: 1.2, rampIn: 0, rampOut: 0 }),
		];
		expect(evaluateZoomAt(regions, 5).scale).toBeCloseTo(3, 6);
	});

	/** Rust picks the same winner, or the preview and the export disagree about
	 *  where the frame is pointing. */
	it("matches the compositor on a nested overlapping", () => {
		const regions = [
			overlapping({ start: 0, end: 10, scale: 1.5, rampIn: 0, rampOut: 0 }),
			overlapping({ start: 4, end: 6, scale: 3, rampIn: 0, rampOut: 0 }),
		];
		expect(evaluateZoomAt(regions, 5).scale).toBeCloseTo(3, 6);
	});
});

// Shared with `crates/recast-scene/src/v1/nodes.rs`: the eased ramp is written twice, and a drift means the preview zooms at a rate the file does not.
describe("zoom ramp parity with the Rust scene", () => {
	interface Case {
		ease: Easing;
		start: number;
		end: number;
		scale: number;
		rampIn: number;
		rampOut: number;
		t: number;
		expect: number;
	}

	it("has enough cases to catch a drift", () => {
		expect((zoomCases as Case[]).length).toBeGreaterThanOrEqual(100);
	});

	// Rust evaluates the bezier in f32 and TypeScript in f64, so the curves agree to single precision, not to the bit.
	it("eases every case to within single precision", () => {
		for (const c of zoomCases as Case[]) {
			const got = zoomScaleAt(
				region({
					start: c.start,
					end: c.end,
					scale: c.scale,
					rampIn: c.rampIn,
					rampOut: c.rampOut,
					easeIn: c.ease,
					easeOut: c.ease,
				}),
				c.t,
			);
			const inForce = c.t > c.start && c.t < c.end;
			expect(inForce ? got : 1).toBeCloseTo(c.expect, 5);
		}
	});
});

describe("cameraStall", () => {
	const ok = {
		enabled: true,
		hasSrc: true,
		elementMounted: true,
		readyState: 4,
		videoWidth: 1280,
		gated: true,
		frameReady: false,
		boundInEngine: true,
	};

	it("says nothing when the camera is drawing", () => {
		expect(cameraStall(ok)).toBeNull();
	});

	// A camera nobody asked for is not a fault, so it must stay silent.
	it("says nothing when the camera is switched off", () => {
		expect(cameraStall({ ...ok, enabled: false, hasSrc: false })).toBeNull();
	});

	it("names each broken link, earliest first", () => {
		expect(cameraStall({ ...ok, hasSrc: false })).toMatch(/no camera file/);
		expect(cameraStall({ ...ok, elementMounted: false })).toMatch(/never mounted/);
		expect(cameraStall({ ...ok, readyState: 1 })).toMatch(/readyState 1/);
		expect(cameraStall({ ...ok, videoWidth: 0 })).toMatch(/zero width/);
		expect(cameraStall({ ...ok, boundInEngine: false, frameReady: true })).toMatch(/bound none/);
	});

	// The gate is normal between presented frames; it only counts as a stall while nothing has ever been bound.
	it("reports the gate only when it is the earliest break", () => {
		expect(cameraStall({ ...ok, frameReady: false, boundInEngine: true })).toBeNull();
		expect(cameraStall({ ...ok, gated: true, frameReady: false, boundInEngine: false })).toMatch(
			/no new camera frame/,
		);
	});

	// Reported symptom: the file is attached and decoding, but nothing shows.
	it("blames the engine when every host-side link is healthy", () => {
		expect(cameraStall({ ...ok, boundInEngine: false, frameReady: true })).toMatch(/bound none/);
	});
});
