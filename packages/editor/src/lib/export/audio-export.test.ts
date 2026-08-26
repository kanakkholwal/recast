import type { Region } from "@recast/media";
import { outputToSource } from "@recast/media";
import { describe, expect, it } from "vitest";
import {
	applyFade,
	applyGain,
	audioOutputDuration,
	planAudioSpans,
	resampleLinear,
} from "./audio-export";

describe("planAudioSpans", () => {
	it("lays kept regions end to end with no gap", () => {
		const spans = planAudioSpans([
			{ start: 1, end: 3 },
			{ start: 5, end: 6 },
		]);
		expect(spans.map((s) => [s.outputStart, s.outputEnd])).toEqual([
			[0, 2],
			[2, 3],
		]);
		expect(audioOutputDuration(spans)).toBe(3);
	});

	it("compresses a sped-up region's output time", () => {
		const spans = planAudioSpans([{ start: 0, end: 4, speed: 2 }]);
		expect(spans[0].outputEnd).toBe(2);
		expect(spans[0].rate).toBe(2);
	});

	it("treats a missing or non-positive speed as 1x", () => {
		expect(planAudioSpans([{ start: 0, end: 2, speed: 0 }])[0].rate).toBe(1);
		expect(planAudioSpans([{ start: 0, end: 2, speed: -1 }])[0].rate).toBe(1);
	});

	it("drops a zero-length region rather than emitting an empty span", () => {
		expect(planAudioSpans([{ start: 2, end: 2 }])).toEqual([]);
	});
});

// The parity requirement: audio must land on the same output timeline the
// picture does. `outputToSource` is what the preview clock uses, so the two
// have to agree at every span boundary.
describe("audio/video parity", () => {
	const cases: Region[][] = [
		[{ start: 0, end: 10 }],
		[
			{ start: 0, end: 3 },
			{ start: 7, end: 12 },
		],
		[
			{ start: 0, end: 4, speed: 2 },
			{ start: 4, end: 6 },
			{ start: 9, end: 13, speed: 0.5 },
		],
	];

	it("starts each span at the source time the video clock reads just inside it", () => {
		for (const regions of cases) {
			for (const span of planAudioSpans(regions)) {
				// Well inside, not on the edge: `outputToSource` holds a tolerance
				// band around a boundary and resolves it to the PREVIOUS region's
				// end, so the boundary itself is ambiguous by design.
				const delta = 0.01;
				const justInside = span.outputStart + delta;
				if (justInside >= span.outputEnd) continue;
				const expected = span.sourceStart + delta * span.rate;
				expect(outputToSource(regions, justInside)).toBeCloseTo(expected, 6);
			}
		}
	});

	it("resolves an exact boundary to the previous region's end", () => {
		const regions: Region[] = [
			{ start: 0, end: 3 },
			{ start: 7, end: 12 },
		];
		expect(outputToSource(regions, 3)).toBe(3);
	});

	it("agrees on total output duration", () => {
		for (const regions of cases) {
			const spans = planAudioSpans(regions);
			const total = regions.reduce(
				(acc, r) => acc + (r.end - r.start) / (r.speed && r.speed > 0 ? r.speed : 1),
				0,
			);
			expect(audioOutputDuration(spans)).toBeCloseTo(total, 6);
		}
	});

	it("maps a mid-span output time back to the right source time", () => {
		const regions = cases[2];
		const spans = planAudioSpans(regions);
		for (const span of spans) {
			const mid = (span.outputStart + span.outputEnd) / 2;
			const expected = span.sourceStart + (mid - span.outputStart) * span.rate;
			expect(outputToSource(regions, mid)).toBeCloseTo(expected, 6);
		}
	});
});

describe("resampleLinear", () => {
	it("returns the input untouched at 1x", () => {
		const input = new Float32Array([1, 2, 3]);
		expect(resampleLinear(input, 1)).toBe(input);
	});

	it("halves the sample count at 2x", () => {
		const input = new Float32Array([0, 1, 2, 3, 4, 5]);
		expect(resampleLinear(input, 2)).toEqual(new Float32Array([0, 2, 4]));
	});

	it("interpolates between samples when slowing down", () => {
		const out = resampleLinear(new Float32Array([0, 10]), 0.5);
		expect(Array.from(out)).toEqual([0, 5, 10, 10]);
	});

	// Reading past the end would produce NaN and a burst of noise at the tail.
	it("clamps the final interpolation to the last sample", () => {
		const out = resampleLinear(new Float32Array([1, 2, 3]), 0.5);
		expect(out.every((v) => Number.isFinite(v))).toBe(true);
	});
});

describe("applyFade", () => {
	it("ramps in from silence and out to silence", () => {
		const ch = new Float32Array(100).fill(1);
		applyFade(ch, 100, 1, 0.2, 0.2);
		expect(ch[0]).toBe(0);
		expect(ch[10]).toBeCloseTo(0.5, 5);
		expect(ch[50]).toBe(1);
		expect(ch[99]).toBeCloseTo(0.05, 5);
	});

	it("leaves the middle untouched when there is no fade", () => {
		const ch = new Float32Array(10).fill(0.5);
		applyFade(ch, 10, 1, 0, 0);
		expect(Array.from(ch)).toEqual(Array(10).fill(0.5));
	});

	// A chunk decoded mid-timeline must fade by its absolute position, not its
	// offset within the chunk.
	it("honours the chunk's offset into the output", () => {
		const ch = new Float32Array(10).fill(1);
		applyFade(ch, 10, 2, 0.5, 0, 1);
		expect(ch[0]).toBe(1);
	});
});

describe("applyGain", () => {
	it("scales every sample", () => {
		const ch = new Float32Array([1, -1, 0.5]);
		applyGain(ch, 0.5);
		expect(Array.from(ch)).toEqual([0.5, -0.5, 0.25]);
	});

	it("mutes at zero", () => {
		const ch = new Float32Array([1, 1]);
		applyGain(ch, 0);
		expect(Array.from(ch)).toEqual([0, 0]);
	});
});
