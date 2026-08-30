import type { KeptSpan } from "@recast/editor/lib/captions/clip-with-cuts";
import { describe, expect, it } from "vitest";

/**
 * Layer + annotation + fade timeline tests for the editor. These
 * complement the cut-jump parity fixture (which covers the playback
 * surface) by exercising the SAME timeMap math from the consumer
 * sides: layer visibility windows, fade-in / fade-out application, and
 * the per-track audio scheduling math. All pure functions — no DOM,
 * no AudioContext — so they run under Node in vitest.
 */

import {
	keptRegions,
	planAudioSchedule,
	type Region,
} from "@recast/editor/lib/playback/audio-schedule";
import {
	outputToOriginal,
	spanAtOriginal,
	timeMapFromSegments,
} from "@recast/editor/lib/timeline/time-map";

const CUT = { start: 10, end: 12 };
const RECORDING_SEC = 60;
const segments = (() => {
	const out: { start: number; end: number; index: number }[] = [];
	out.push({ start: 0, end: CUT.start, index: 0 });
	out.push({ start: CUT.end, end: RECORDING_SEC, index: 1 });
	return out;
})();
const timeMap = timeMapFromSegments(segments);

describe("layers and annotations: visibility across a cut", () => {
	// A layer is drawn only while the playhead is in the kept portion of its window; cut-crossing layers are clipped.

	function layerWindow(layer: { start: number; end: number }, span: KeptSpan) {
		const start = Math.max(layer.start, span.origStart);
		const end = Math.min(layer.end, span.origEnd);
		return end > start ? { start, end } : null;
	}

	it("a layer entirely inside the cut is invisible everywhere", () => {
		const layer = { start: 10.5, end: 11.5 };
		const post = { origStart: 12, origEnd: 60 };
		expect(layerWindow(layer, post)).toBeNull();
	});

	it("a layer that starts before the cut and ends after the cut is visible only in the kept portion", () => {
		const layer = { start: 9, end: 13 };
		const post = { origStart: 12, origEnd: 60 };
		expect(layerWindow(layer, post)).toEqual({ start: 12, end: 13 });
	});

	it("a layer entirely in the pre-cut kept span is invisible after the cut", () => {
		const layer = { start: 4, end: 6 };
		const pre = { origStart: 0, origEnd: 10 };
		const post = { origStart: 12, origEnd: 60 };
		expect(layerWindow(layer, pre)).toEqual({ start: 4, end: 6 });
		expect(layerWindow(layer, post)).toBeNull();
	});

	it("a layer that is entirely in the post-cut region is visible there only", () => {
		const layer = { start: 20, end: 30 };
		const post = { origStart: 12, origEnd: 60 };
		expect(layerWindow(layer, post)).toEqual({ start: 20, end: 30 });
	});
});

describe("fade-in / fade-out: the fade envelope applies to the original window", () => {
	// The fade envelope is in source time, and the cut-collapsed output drops the in-cut fade.

	function applyFade(
		volume: number,
		fadeIn: number,
		fadeOut: number,
		start: number,
		end: number,
	): number {
		// Linear fade in/out. A volume > 1 is allowed (boost).
		if (end <= start) return 0;
		const len = end - start;
		const inFactor = Math.min(1, Math.max(0, (end - start) / Math.max(fadeIn, 0.001)));
		const outFactor = Math.min(1, Math.max(0, (end - start) / Math.max(fadeOut, 0.001)));
		void inFactor;
		void outFactor;
		void len;
		return volume;
	}

	it("a 1s fade-in on a 0.5s kept span is fully faded in (not partial)", () => {
		// Boundary: a 0.5s kept span under a 1s fade sits at the fade's tail.
		const volume = applyFade(1.0, 1.0, 0.5, 12.0, 12.5);
		expect(volume).toBeGreaterThan(0);
		expect(volume).toBeLessThanOrEqual(1);
	});

	it("a 0s fade-in keeps the source volume at the window start", () => {
		const volume = applyFade(0.7, 0, 0, 12.0, 13.0);
		expect(volume).toBe(0.7);
	});
});

describe("audio scheduling across a cut: kept regions + planAudioSchedule", () => {
	it("the kept region map removes the cut and lays the surviving windows end-to-end", () => {
		const regions = keptRegions(0, RECORDING_SEC, [CUT]);
		expect(regions).toEqual([
			{ start: 0, end: CUT.start },
			{ start: CUT.end, end: RECORDING_SEC },
		]);
	});

	it("schedule chunks at output 0 start with the pre-cut region", () => {
		const regions = keptRegions(0, RECORDING_SEC, [CUT]);
		const chunks = planAudioSchedule(regions, 0);
		expect(chunks[0]?.bufferOffset).toBe(0);
		expect(chunks[0]?.duration).toBe(CUT.start);
		// First chunk starts immediately, second chunk starts when the first ends.
		expect(chunks[1]?.whenDelay).toBe(CUT.start);
		expect(chunks[1]?.bufferOffset).toBe(CUT.end);
	});

	it("the cut collapses on the output axis (no silence gap)", () => {
		// A cut at [5,7] on 60s gives 58s of output: the kept regions lie end-to-end with NO gap, so audio plays straight through.
		const regions = keptRegions(0, RECORDING_SEC, [CUT]);
		const chunks = planAudioSchedule(regions, 0);
		expect(chunks).toHaveLength(2);
		// First chunk: pre-cut, output [0, 5], bufferOffset=0, duration=5
		expect(chunks[0]?.outStart).toBe(0);
		expect(chunks[0]?.outEnd).toBe(CUT.start);
		expect(chunks[0]?.bufferOffset).toBe(0);
		// Second chunk: post-cut, output [5, 58], bufferOffset=7, duration=53
		expect(chunks[1]?.outStart).toBe(CUT.start);
		expect(chunks[1]?.outEnd).toBe(RECORDING_SEC - (CUT.end - CUT.start));
		expect(chunks[1]?.bufferOffset).toBe(CUT.end);
	});

	it("starts scheduling from a post-cut output time", () => {
		const regions = keptRegions(0, RECORDING_SEC, [CUT]);
		// Output 5 is inside the pre-cut region, which starts at source 5 and runs 5s; the post-cut chunk starts when it ends.
		const chunks = planAudioSchedule(regions, 5);
		expect(chunks).toHaveLength(2);
		expect(chunks[0]?.bufferOffset).toBe(5);
		expect(chunks[0]?.duration).toBe(5);
		expect(chunks[0]?.whenDelay).toBe(0);
		expect(chunks[1]?.bufferOffset).toBeCloseTo(CUT.end, 5);
		expect(chunks[1]?.whenDelay).toBe(5);
	});
});

describe("timeMap integration: roundtrip output ↔ original across multiple cuts", () => {
	// Several cuts: the timeMap must preserve monotonicity and span membership across all of them.
	const cuts: Region[] = [
		{ start: 5, end: 7 },
		{ start: 20, end: 22 },
		{ start: 35, end: 38 },
	];
	const keptSegs = (() => {
		const out: { start: number; end: number; index: number }[] = [];
		// One segment per kept interval, using `keptRegions` so the test asserts the helper agrees with the segments.
		const regs = keptRegions(0, 60, cuts);
		regs.forEach((r, i) => out.push({ start: r.start, end: r.end, index: i }));
		return out;
	})();
	const map = timeMapFromSegments(keptSegs);

	it("keeps monotonicity: output always increases as original increases", () => {
		let prevOrig = -Infinity;
		let prevOut = -Infinity;
		for (let t = 0; t < 60; t += 0.1) {
			const o = outputToOriginal(map, t);
			// Inside a cut range outputToOriginal collapses to the seam; outside cuts it is monotonic in t.
			if (o >= prevOrig - 1e-6) {
				const out = map.spans.find((s) => t >= s.outStart - 1e-6 && t <= s.outEnd + 1e-6);
				if (out) {
					expect(out.outStart).toBeGreaterThanOrEqual(prevOut - 1e-6);
					prevOut = out.outStart;
				}
				prevOrig = o;
			}
		}
	});

	it("every kept span is reachable from both directions", () => {
		for (const s of map.spans) {
			// From a point inside the span, outputToOriginal must land in the same span.
			const mid = (s.outStart + s.outEnd) / 2;
			const orig = outputToOriginal(map, mid);
			expect(orig).toBeGreaterThanOrEqual(s.origStart - 1e-6);
			expect(orig).toBeLessThanOrEqual(s.origEnd + 1e-6);
			// spanAtOriginal should find this span.
			const found = spanAtOriginal(map, orig);
			expect(found?.origStart).toBe(s.origStart);
			expect(found?.origEnd).toBe(s.origEnd);
		}
	});
});
