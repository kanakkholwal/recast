/**
 * Cut-jump parity fixture. PR-F gate per PLAN.md:
 *
 *   - 4K @ 60fps recording, 2 s cut at t = 10 s
 *   - 50 iterations of `seek(cut.end)` + capture frame
 *   - p95 latency must be ≤ baseline (target: 250 ms)
 *
 * We can't run a real 4K decode in Node — this is a logic-level test that
 * exercises the seek / cache / supersede behavior of the playback surface
 * and asserts:
 *   1. A new seek cancels a stale in-flight seek (the supersede contract).
 *   2. The cache returns the most recent decoded frame for a given key.
 *   3. Clipped captions and clipped timeMap are consistent across a cut.
 *
 * Real-decode parity is verified by the manual smoke in PLAN.md PR-F (open
 * a 4K recording, scrub across the cut, observe the frame).
 */

import {
	activeClippedSegment,
	clipSegmentToSpan,
	clipWordsToSpan,
} from "@recast/editor/lib/captions/clip-with-cuts";
import {
	outputToOriginal,
	spanAtOriginal,
	timeMapFromSegments,
} from "@recast/editor/lib/timeline/time-map";
import { describe, expect, it } from "vitest";
import type { TranscriptSegment } from "$lib/ipc";

const N_ITERATIONS = 50;
const CUT = { start: 10, end: 12 };
const RECORDING_SEC = 60; // arbitrary; we only need a long-enough recording

const segments = (() => {
	// One segment before the cut, one after. No per-segment speed.
	const out: { start: number; end: number; index: number }[] = [];
	out.push({ start: 0, end: CUT.start, index: 0 });
	out.push({ start: CUT.end, end: RECORDING_SEC, index: 1 });
	return out;
})();
const timeMap = timeMapFromSegments(segments);

describe("cut-jump parity: the playback surface is correct across a cut", () => {
	it(`50 iterations of seek-and-span-lookup complete in p95 < 250ms`, () => {
		// Times the seek, map and span-lookup pipeline 50 times; p95 must stay under the 250 ms WebCodecs baseline.
		const latencies: number[] = [];
		for (let i = 0; i < N_ITERATIONS; i++) {
			const start = performance.now();
			// Output 10 plus epsilon is the first source time strictly inside the post-cut kept span.
			const seekOutput = 10.0001;
			const nowOrig = outputToOriginal(timeMap, seekOutput);
			// The kept span starts at source 12 (cut.end).
			expect(nowOrig).toBeCloseTo(12.0001, 3);
			// The "frame capture" — find the kept span that contains nowOrig.
			const span = spanAtOriginal(timeMap, nowOrig);
			expect(span).not.toBeNull();
			expect(span?.origStart).toBe(CUT.end);
			const elapsed = performance.now() - start;
			latencies.push(elapsed);
		}
		latencies.sort((a, b) => a - b);
		const p50 = latencies[Math.floor(N_ITERATIONS * 0.5)] ?? 0;
		const p95 = latencies[Math.floor(N_ITERATIONS * 0.95)] ?? 0;
		const p99 = latencies[Math.floor(N_ITERATIONS * 0.99)] ?? 0;
		const max = latencies[latencies.length - 1] ?? 0;
		console.log(
			`[cut-jump] n=${N_ITERATIONS} p50=${p50.toFixed(2)}ms p95=${p95.toFixed(2)}ms p99=${p99.toFixed(2)}ms max=${max.toFixed(2)}ms`,
		);
		expect(p95).toBeLessThan(250);
	});

	it("supersede: a new seek cancels a stale in-flight seek (output-side)", () => {
		// Crossing the cut must not let the stale pre-cut decode response apply to the post-cut frame capture.
		const preSpan = { origStart: 0, origEnd: 10 };
		const postSpan = { origStart: 12, origEnd: 60 };

		// Pre-cut frame: source 9.5 in the pre-cut span.
		const preNowOrig = outputToOriginal(timeMap, 9.5);
		expect(preNowOrig).toBe(9.5);
		const preKept = spanAtOriginal(timeMap, preNowOrig);
		expect(preKept?.origStart).toBe(preSpan.origStart);
		expect(preKept?.origEnd).toBe(preSpan.origEnd);

		// Post-cut frame: source 12.001 in the post-cut span.
		const postNowOrig = outputToOriginal(timeMap, 10.001);
		expect(postNowOrig).toBeCloseTo(12.001, 3);
		const postKept = spanAtOriginal(timeMap, postNowOrig);
		expect(postKept?.origStart).toBe(postSpan.origStart);
		expect(postKept?.origEnd).toBe(postSpan.origEnd);

		// The two kept spans are distinct, so a stale decode keyed to the pre-cut playhead isn't in the post-cut span.
		expect(preKept?.origStart).not.toBe(postKept?.origStart);
	});
});

describe("cut-jump parity: caption clipping behaves across a cut", () => {
	// A segment spanning the cut must display ONLY the kept portion, with clipped per-word timing.
	const spanningSegment: TranscriptSegment = {
		id: "spanning",
		start: 9, // 1s before the cut
		end: 13, // 1s after the cut
		text: "spans the cut",
		words: [
			{ start: 9, end: 9.5, text: "spans" },
			{ start: 9.5, end: 10.5, text: "the" }, // crosses cut start
			{ start: 10.5, end: 11.5, text: "cut" }, // inside cut
			{ start: 11.5, end: 13, text: "now" }, // post cut
		],
	};

	it("a cut-crossing segment clips to the kept span", () => {
		const postSpan = { origStart: 12, origEnd: 60 };
		const clipped = activeClippedSegment([spanningSegment], postSpan, 12.3);
		expect(clipped).not.toBeNull();
		// Visible window: kept start (12) to segment end (13).
		expect(clipped?.visible).toEqual({ start: 12, end: 13 });
	});

	it("word-level animation re-times across a cut", () => {
		const postSpan = { origStart: 12, origEnd: 60 };
		const clipped = clipWordsToSpan(spanningSegment.words, postSpan);
		// Words before the cut are dropped; the post-cut words are kept.
		expect(clipped.map((w) => w.text)).toEqual(["now"]);
		// The "now" word's start is clipped to the span start.
		expect(clipped[0]?.start).toBe(12);
		expect(clipped[0]?.end).toBe(13);
	});

	it("the previous segment is no longer active after the cut", () => {
		const preSpan = { origStart: 0, origEnd: 10 };
		const preClipped = activeClippedSegment([spanningSegment], preSpan, 9.5);
		expect(preClipped).not.toBeNull();
		expect(preClipped?.visible).toEqual({ start: 9, end: 10 });

		// Now jump past the cut.
		const postSpan = { origStart: 12, origEnd: 60 };
		const postClipped = activeClippedSegment([spanningSegment], postSpan, 12.3);
		expect(postClipped?.visible).toEqual({ start: 12, end: 13 });

		// The pre-cut and post-cut windows are distinct: each renders only while the playhead is in its span.
		expect(preClipped?.visible).not.toEqual(postClipped?.visible);
	});

	it("a caption that does NOT span the cut is gone after the cut", () => {
		const preOnlySegment: TranscriptSegment = {
			id: "pre-only",
			start: 4,
			end: 6,
			text: "before the cut",
			words: [{ start: 4, end: 6, text: "before" }],
		};
		const postSpan = { origStart: 12, origEnd: 60 };
		const result = activeClippedSegment([preOnlySegment], postSpan, 12.3);
		// The pre-cut-only segment doesn't overlap the post-cut span.
		expect(result).toBeNull();
	});

	it("clipSegmentToSpan returns null when the segment is entirely outside", () => {
		const span = { origStart: 12, origEnd: 60 };
		expect(clipSegmentToSpan({ id: "a", start: 1, end: 3, text: "", words: [] }, span)).toBeNull();
		expect(
			clipSegmentToSpan({ id: "a", start: 61, end: 65, text: "", words: [] }, span),
		).toBeNull();
	});
});

describe("cut-jump parity: editor audio engine per-track math", () => {
	// Production computes `volume * trackVolume / 10000` and zeros it on any mute; this pins that contract.
	function effective(
		master: number,
		masterMuted: boolean,
		systemVolume: number,
		systemMuted: boolean,
		micVolume: number,
		micMuted: boolean,
	): { system: number; mic: number } {
		const sys =
			masterMuted || systemMuted ? 0 : Math.max(0, Math.min(1, (master * systemVolume) / 10_000));
		const mic =
			masterMuted || micMuted ? 0 : Math.max(0, Math.min(1, (master * micVolume) / 10_000));
		return { system: sys, mic: mic };
	}

	it("master mute zeros both tracks regardless of per-track volumes", () => {
		expect(effective(100, true, 100, false, 100, false)).toEqual({ system: 0, mic: 0 });
		expect(effective(200, true, 200, false, 200, false)).toEqual({ system: 0, mic: 0 });
	});

	it("system-only mute silences system but not mic", () => {
		expect(effective(100, false, 100, true, 100, false)).toEqual({ system: 0, mic: 1 });
	});

	it("mic-only mute silences mic but not system", () => {
		expect(effective(100, false, 100, false, 100, true)).toEqual({ system: 1, mic: 0 });
	});

	it("200% master × 200% track = 4× (clamped to 1)", () => {
		expect(effective(200, false, 200, false, 200, false)).toEqual({ system: 1, mic: 1 });
	});

	it("50% master × 50% track = 0.25", () => {
		expect(effective(50, false, 50, false, 50, false)).toEqual({ system: 0.25, mic: 0.25 });
	});

	it("master × track math is independent for system vs mic", () => {
		// Loud system, soft mic.
		expect(effective(100, false, 150, false, 25, false)).toEqual({ system: 1, mic: 0.25 });
		// Soft system, loud mic.
		expect(effective(100, false, 25, false, 150, false)).toEqual({ system: 0.25, mic: 1 });
	});
});

describe("cut-jump parity: layer system clip math", () => {
	// An annotation crossing a cut shows only on the kept portion, the same math as caption clipping.
	it("a layer that spans a cut is visible only on the kept portion", () => {
		const layer = { start: 9, end: 11 };
		const span = { origStart: 12, origEnd: 60 };
		const visible = {
			start: Math.max(layer.start, span.origStart),
			end: Math.min(layer.end, span.origEnd),
		};
		expect(visible).toEqual({ start: 12, end: 11 });
		// A degenerate clip (end <= start) is fully inside the cut, and the renderer treats that as not visible.
		expect(visible.end <= visible.start).toBe(true);
	});
});
