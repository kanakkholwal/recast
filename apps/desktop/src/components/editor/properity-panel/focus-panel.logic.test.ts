import { describe, expect, it } from "vitest";
import type { ZoomRegion } from "$lib/stores/editor-store.svelte";
import {
	focusWindow,
	isOutsideClip,
	regionMaxRamp,
	retimeEnd,
	retimeStart,
	scaleAt,
	sparklinePath,
} from "./focus-panel.logic";

const linear = { x1: 0, y1: 0, x2: 1, y2: 1 };
function region(over: Partial<ZoomRegion> = {}): ZoomRegion {
	return {
		start: 0,
		end: 10,
		scale: 2,
		rampIn: 2,
		rampOut: 2,
		easeIn: linear,
		easeOut: linear,
		...over,
	} as unknown as ZoomRegion;
}

describe("regionMaxRamp", () => {
	it("is half the region duration", () => {
		expect(regionMaxRamp(region({ start: 0, end: 10 }))).toBe(5);
		expect(regionMaxRamp(region({ start: 4, end: 4 }))).toBe(0);
	});
});

describe("scaleAt", () => {
	const r = region();
	it("is 1 outside the region", () => {
		expect(scaleAt(r, -1)).toBe(1);
		expect(scaleAt(r, 0)).toBe(1);
		expect(scaleAt(r, 10)).toBe(1);
		expect(scaleAt(r, 99)).toBe(1);
	});
	it("holds at full scale between the ramps", () => {
		// rampIn 2 → holdStart 2; rampOut 2 → holdEnd 8
		expect(scaleAt(r, 5)).toBe(2);
		expect(scaleAt(r, 2)).toBe(2);
	});
	it("rises from 1 toward scale during ramp-in (linear easing)", () => {
		// halfway through a linear ramp-in → scale 1.5
		expect(scaleAt(r, 1)).toBeCloseTo(1.5, 5);
	});
});

describe("sparklinePath", () => {
	it("emits a moveto then 40 linetos across the width", () => {
		const path = sparklinePath(region(), 100, 20);
		expect(path.startsWith("M ")).toBe(true);
		expect((path.match(/L /g) ?? []).length).toBe(40);
		expect(path).toContain("100.00"); // last sample reaches full width
	});
});

describe("retimeStart", () => {
	const r = { start: 4, end: 8 };

	it("moves the start to the playhead", () => {
		expect(retimeStart(r, 5, 0)).toEqual({ start: 5 });
	});

	it("clamps to the clip in-point rather than refusing", () => {
		expect(retimeStart(r, -3, 1)).toEqual({ start: 1 });
	});

	// Silently snapping to end-0.1 would look like the button mis-fired, so the
	// caller disables it instead.
	it("returns null when the playhead leaves no room before the end", () => {
		expect(retimeStart(r, 8, 0)).toBeNull();
		expect(retimeStart(r, 7.95, 0)).toBeNull();
	});

	it("returns null when the clip in-point itself leaves no room", () => {
		expect(retimeStart(r, 5, 8)).toBeNull();
	});
});

describe("retimeEnd", () => {
	const r = { start: 4, end: 8 };

	it("moves the end to the playhead", () => {
		expect(retimeEnd(r, 6, 20)).toEqual({ end: 6 });
	});

	it("clamps to the clip out-point rather than refusing", () => {
		expect(retimeEnd(r, 99, 9)).toEqual({ end: 9 });
	});

	it("returns null when the playhead leaves no room after the start", () => {
		expect(retimeEnd(r, 4, 20)).toBeNull();
		expect(retimeEnd(r, 4.05, 20)).toBeNull();
	});
});

describe("focusWindow", () => {
	it("is the whole frame at 1x", () => {
		expect(focusWindow(0.5, 0.5, 1)).toEqual({ left: 0, top: 0, size: 1 });
	});

	it("is centred only when the focus point is centred", () => {
		expect(focusWindow(0.5, 0.5, 2)).toEqual({ left: 0.25, top: 0.25, size: 0.5 });
	});

	// The affine PINS the focus point to its own screen position rather than
	// centring on it, so an edge focus point sits flush, never outside.
	it("sits flush against the edge the focus point is on", () => {
		expect(focusWindow(1, 0, 2)).toEqual({ left: 0.5, top: 0, size: 0.5 });
		expect(focusWindow(0, 1, 4)).toEqual({ left: 0, top: 0.75, size: 0.25 });
	});

	// Parity: the window must equal what the preview shader samples, which is
	// videoUV = (screenUV - c) / scale + c over screenUV in [0,1].
	it("matches the shader's focus-pinned affine", () => {
		const shaderAt = (uv: number, c: number, s: number) => (uv - c) / s + c;
		for (const c of [0, 0.2, 0.5, 0.83, 1]) {
			for (const s of [1.5, 2, 3]) {
				const w = focusWindow(c, c, s);
				expect(w.left).toBeCloseTo(shaderAt(0, c, s), 10);
				expect(w.left + w.size).toBeCloseTo(shaderAt(1, c, s), 10);
			}
		}
	});
});

describe("isOutsideClip", () => {
	it("is false for a span inside the clip", () => {
		expect(isOutsideClip({ start: 2, end: 5 }, 1, 9)).toBe(false);
	});

	it("is true when either edge escapes the clip", () => {
		expect(isOutsideClip({ start: 0.5, end: 5 }, 1, 9)).toBe(true);
		expect(isOutsideClip({ start: 2, end: 9.5 }, 1, 9)).toBe(true);
	});

	// Float drift from trim maths must not light up the warning on a span that
	// is flush with the clip edges.
	it("tolerates float drift at the edges", () => {
		expect(isOutsideClip({ start: 1 - 1e-9, end: 9 + 1e-9 }, 1, 9)).toBe(false);
	});
});
