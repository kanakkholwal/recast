import { describe, expect, it } from "vitest";
import {
	originalToOutput as cutOriginalToOutput,
	normalizeCuts,
	type TimelineCut,
} from "./cuts";
import { deriveSegments } from "./segments";
import {
	buildGapMap,
	buildTimeMap,
	displayTimeMap,
	originalToOutput,
	outputToOriginal,
	spanAtOriginal,
	timeMapFromSegments,
} from "./time-map";
import parityFixtures from "./__fixtures__/cut-parity.json";

function cut(start: number, end: number, id = `${start}-${end}`): TimelineCut {
	return { id, start, end, source: "manual" };
}

function span(origStart: number, origEnd: number, speed = 1) {
	return { origStart, origEnd, speed };
}

describe("originalToOutput binary search parity", () => {
	// Reference linear scan: exactly the pre-optimization implementation. The
	// binary-search version must agree with it for every input, including gaps,
	// span interiors, seam-exact times, and out-of-range.
	function linearOriginalToOutput(
		map: ReturnType<typeof buildTimeMap>,
		t: number,
	): number {
		for (const s of map.spans) {
			if (t < s.origStart) return s.outStart;
			if (t <= s.origEnd) return s.outStart + (t - s.origStart) / s.speed;
		}
		return map.outputDuration;
	}

	it("matches the linear scan across randomized maps and probe times", () => {
		// Deterministic LCG so failures reproduce; no Math.random.
		let seed = 0x2545f491;
		const rand = () => {
			seed = (seed * 1103515245 + 12345) & 0x7fffffff;
			return seed / 0x7fffffff;
		};
		for (let trial = 0; trial < 2000; trial++) {
			const n = 1 + Math.floor(rand() * 12);
			const spans: ReturnType<typeof span>[] = [];
			let cursor = rand() * 3;
			for (let i = 0; i < n; i++) {
				const gap = rand() * 2; // removed region before this span
				const len = 0.2 + rand() * 3;
				const start = cursor + gap;
				spans.push(span(start, start + len, 0.5 + rand() * 2));
				cursor = start + len;
			}
			const map = buildTimeMap(spans);
			// Probe interiors, gaps, seam-exact endpoints, and out-of-range.
			const probes = [-1, cursor + 1];
			for (const s of map.spans) {
				probes.push(s.origStart, s.origEnd, (s.origStart + s.origEnd) / 2, s.origStart - 0.01);
			}
			for (const t of probes) {
				expect(originalToOutput(map, t)).toBeCloseTo(linearOriginalToOutput(map, t), 9);
			}
		}
	});
});

describe("buildTimeMap", () => {
	it("lays kept spans end-to-end on the output axis", () => {
		const map = buildTimeMap([span(0, 2), span(5, 6)]);
		expect(map.spans.map((s) => [s.outStart, s.outEnd])).toEqual([
			[0, 2],
			[2, 3],
		]);
		expect(map.outputDuration).toBeCloseTo(3);
	});

	it("sorts spans by original start", () => {
		const map = buildTimeMap([span(5, 6), span(0, 2)]);
		expect(map.spans.map((s) => s.origStart)).toEqual([0, 5]);
	});

	it("drops zero-length spans", () => {
		expect(buildTimeMap([span(2, 2)]).spans).toHaveLength(0);
	});

	it("a 2x span occupies half the output width", () => {
		const map = buildTimeMap([span(0, 4, 2)]);
		expect(map.outputDuration).toBeCloseTo(2);
	});

	it("falls back to 1x for a non-positive or non-finite speed", () => {
		expect(buildTimeMap([span(0, 4, 0)]).outputDuration).toBeCloseTo(4);
		expect(
			buildTimeMap([span(0, 4, Number.POSITIVE_INFINITY)]).outputDuration,
		).toBeCloseTo(4);
	});
});

describe("originalToOutput / outputToOriginal (general map)", () => {
	const map = buildTimeMap([span(0, 4, 2), span(6, 10, 1)]); // out: [0,2] then [2,6]

	it("applies per-span slope", () => {
		expect(originalToOutput(map, 2)).toBeCloseTo(1); // half-way through 2x span
		expect(originalToOutput(map, 8)).toBeCloseTo(4); // half-way through 1x span
	});

	it("collapses a removed-gap time onto the next seam", () => {
		// The [4,6] gap has no output image; both edges map to the seam at out=2.
		expect(originalToOutput(map, 5)).toBeCloseTo(2);
	});

	it("round-trips kept times", () => {
		for (const t of [0, 1, 3, 6, 7, 9, 10]) {
			expect(outputToOriginal(map, originalToOutput(map, t))).toBeCloseTo(t);
		}
	});

	it("right-biases an exact internal seam", () => {
		// out=2 is both the 2x span's end and the 1x span's start → next span wins.
		expect(outputToOriginal(map, 2)).toBeCloseTo(6);
	});

	it("clamps output outside the kept range", () => {
		expect(outputToOriginal(map, -1)).toBeCloseTo(0);
		expect(outputToOriginal(map, 99)).toBeCloseTo(10);
	});

	it("maps everything to 0 when the map is empty (all cut away)", () => {
		// A fully-cut timeline yields no kept spans; both directions degrade to 0
		// rather than reading past an empty span list.
		const empty = buildTimeMap([]);
		expect(empty.spans).toHaveLength(0);
		expect(empty.outputDuration).toBe(0);
		expect(originalToOutput(empty, 5)).toBe(0);
		expect(outputToOriginal(empty, 5)).toBe(0);
	});

	it("is monotonic non-decreasing in original time", () => {
		let prev = -Infinity;
		for (let t = 0; t <= 10; t += 0.1) {
			const o = originalToOutput(map, t);
			expect(o).toBeGreaterThanOrEqual(prev - 1e-9);
			prev = o;
		}
	});
});

describe("spanAtOriginal", () => {
	const map = buildTimeMap([span(0, 4, 2), span(6, 10)]);
	it("finds the covering span", () => {
		expect(spanAtOriginal(map, 1)?.origStart).toBe(0);
		expect(spanAtOriginal(map, 7)?.origStart).toBe(6);
	});
	it("returns null inside a removed gap", () => {
		expect(spanAtOriginal(map, 5)).toBeNull();
	});
});

describe("speed=1 reduces exactly to the cut translation map", () => {
	// Same shared fixtures the Rust export and cuts.test.ts assert against: at
	// speed 1 the general map's output duration must equal the kept duration, and
	// its mapping must match cuts.originalToOutput up to the trim-start offset.
	for (const c of parityFixtures.cases) {
		it(`matches fixture: ${c.name}`, () => {
			const cuts = c.cuts.map(([s, e], i) => cut(s, e, `fx-${i}`));
			const segments = deriveSegments({
				trimStart: c.trimStart,
				trimEnd: c.trimEnd,
				cuts,
				splitPoints: [],
			});
			const map = timeMapFromSegments(segments);

			expect(map.outputDuration).toBeCloseTo(c.expectedKeptDuration, 6);

			// The general map's output axis starts at trimStart; the cut map's starts
			// at original 0. They must agree once that constant offset is removed.
			const offset = cutOriginalToOutput(cuts, c.trimStart);
			const normalized = normalizeCuts(cuts);
			for (const seg of segments) {
				for (const t of [seg.start, (seg.start + seg.end) / 2, seg.end]) {
					expect(originalToOutput(map, t)).toBeCloseTo(
						cutOriginalToOutput(normalized, t) - offset,
						6,
					);
				}
			}
		});
	}
});

describe("displayTimeMap (trim-drag axis) reduces to the full-duration cut map at 1x", () => {
	// While trimming the timeline swaps onto this full-recording axis; at 1x it
	// must match the cut translation map over [0, duration] so the layout matches.
	const DURATION = 12;
	for (const c of parityFixtures.cases) {
		if (c.trimEnd > DURATION) continue;
		it(`matches fixture: ${c.name}`, () => {
			const cuts = c.cuts.map(([s, e], i) => cut(s, e, `fx-${i}`));
			const segments = deriveSegments({
				trimStart: c.trimStart,
				trimEnd: c.trimEnd,
				cuts,
				splitPoints: [],
			});
			const map = displayTimeMap({
				trimStart: c.trimStart,
				trimEnd: c.trimEnd,
				durationSec: DURATION,
				segments,
				cuts,
			});
			expect(map.outputDuration).toBeCloseTo(
				cutOriginalToOutput(cuts, DURATION),
				6,
			);
			for (let t = 0; t <= DURATION; t += 0.37) {
				expect(originalToOutput(map, t)).toBeCloseTo(
					cutOriginalToOutput(cuts, t),
					6,
				);
			}
		});
	}
});

describe("buildGapMap (the show-cut-gaps render axis)", () => {
	it("re-spaces kept spans by the removed duration so a cut gets real width", () => {
		// Two kept spans with 3s removed between them (a cut over original [2,5]).
		const collapsed = buildTimeMap([
			{ origStart: 0, origEnd: 2, speed: 1 },
			{ origStart: 5, origEnd: 7, speed: 1 },
		]);
		expect(collapsed.outputDuration).toBeCloseTo(4); // ripple
		const gap = buildGapMap(collapsed);
		expect(gap.outputDuration).toBeCloseTo(7); // 4 kept + 3 gap
		// First span unchanged; second pushed right by the 3s gap.
		expect(originalToOutput(gap, 2)).toBeCloseTo(2); // end of first, left of gap
		expect(originalToOutput(gap, 5)).toBeCloseTo(5); // start of second, right of gap
		// Kept widths preserved (2s each).
		expect(gap.spans[0].outEnd - gap.spans[0].outStart).toBeCloseTo(2);
		expect(gap.spans[1].outEnd - gap.spans[1].outStart).toBeCloseTo(2);
	});

	it("is a no-op when there is no removed time (splits touch)", () => {
		const contiguous = buildTimeMap([
			{ origStart: 0, origEnd: 4, speed: 1 },
			{ origStart: 4, origEnd: 10, speed: 1 },
		]);
		const gap = buildGapMap(contiguous);
		expect(gap.outputDuration).toBeCloseTo(contiguous.outputDuration);
		expect(gap.spans.map((s) => s.outStart)).toEqual(
			contiguous.spans.map((s) => s.outStart),
		);
	});

	it("keeps a sped segment's warped width, only inserting the gap", () => {
		// [0,2]@1x then removed [2,4] then [4,8]@2x → kept widths 2 and 2, gap 2.
		const collapsed = buildTimeMap([
			{ origStart: 0, origEnd: 2, speed: 1 },
			{ origStart: 4, origEnd: 8, speed: 2 },
		]);
		const gap = buildGapMap(collapsed);
		expect(gap.spans[1].outStart).toBeCloseTo(4); // 2 (first) + 2 (gap)
		expect(gap.spans[1].outEnd - gap.spans[1].outStart).toBeCloseTo(2); // 4s @2x
		expect(gap.outputDuration).toBeCloseTo(6);
	});
});

describe("timeMapFromSegments warps a sped-up segment (kept axis)", () => {
	it("narrows a 2x segment and shortens the output", () => {
		const segments = deriveSegments({
			trimStart: 0,
			trimEnd: 10,
			cuts: [],
			splitPoints: [4],
		});
		// Kept axis: output 0 == first segment start; [0,4]@1x=4 then [4,10]@2x=3.
		const map = timeMapFromSegments(segments, (i) => (i === 1 ? 2 : 1));
		expect(map.outputDuration).toBeCloseTo(7);
		expect(originalToOutput(map, 4)).toBeCloseTo(4);
		expect(originalToOutput(map, 10)).toBeCloseTo(7);
	});
});
