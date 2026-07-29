import { describe, expect, it } from 'vitest';
import {
	keptRegions,
	missingRanges,
	outputToSource,
	planAudioSchedule,
	planAudioScheduleWindow,
	sliceChunksForPlayback,
	type AudioChunk,
	type Region,
	type ScheduledChunk,
} from '../src/audio/schedule';

/**
 * Pure-function tests for the audio scheduling math. The actual
 * `AudioWorkletProcessor` runs on the audio thread and can't be tested
 * under Node; these tests cover the math the worklet AND the fallback
 * scheduler both rely on.
 */
describe('keptRegions', () => {
	it('returns the full trim range when no cuts are present', () => {
		expect(keptRegions(0, 10, [])).toEqual([{ start: 0, end: 10 }]);
	});

	it('returns the trimmed region when cuts is empty after filtering', () => {
		expect(keptRegions(5, 10, [{ start: 1, end: 4 }])).toEqual([{ start: 5, end: 10 }]);
	});

	it('removes a single cut from the middle', () => {
		const cuts: Region[] = [{ start: 4, end: 6 }];
		expect(keptRegions(0, 10, cuts)).toEqual([
			{ start: 0, end: 4 },
			{ start: 6, end: 10 },
		]);
	});

	it('clips cuts to the trim range', () => {
		const cuts: Region[] = [{ start: -5, end: 3 }, { start: 7, end: 15 }];
		expect(keptRegions(0, 10, cuts)).toEqual([{ start: 3, end: 7 }]);
	});

	it('merges overlapping cuts', () => {
		const cuts: Region[] = [
			{ start: 2, end: 5 },
			{ start: 4, end: 7 },
		];
		expect(keptRegions(0, 10, cuts)).toEqual([
			{ start: 0, end: 2 },
			{ start: 7, end: 10 },
		]);
	});

	it('filters cuts outside the trim range entirely', () => {
		const cuts: Region[] = [
			{ start: -10, end: -5 },
			{ start: 15, end: 20 },
		];
		expect(keptRegions(0, 10, cuts)).toEqual([{ start: 0, end: 10 }]);
	});

	it('returns empty when cuts cover the trim range', () => {
		expect(keptRegions(0, 10, [{ start: 0, end: 10 }])).toEqual([]);
	});
});

describe('planAudioSchedule', () => {
	it('returns all chunks when starting from output time 0', () => {
		const regions: Region[] = [
			{ start: 0, end: 4 },
			{ start: 6, end: 10 },
		];
		const chunks = planAudioSchedule(regions, 0);
		expect(chunks).toHaveLength(2);
		expect(chunks[0]?.bufferOffset).toBe(0);
		expect(chunks[0]?.duration).toBe(4);
		expect(chunks[0]?.whenDelay).toBe(0);
		expect(chunks[1]?.bufferOffset).toBe(6);
		expect(chunks[1]?.duration).toBe(4);
		expect(chunks[1]?.whenDelay).toBe(4);
	});

	it('skips chunks fully behind the playhead', () => {
		const regions: Region[] = [
			{ start: 0, end: 4 },
			{ start: 6, end: 10 },
		];
		const chunks = planAudioSchedule(regions, 5);
		// The first chunk (output 0..4) is fully behind; the second starts at 4
		// output, so it begins immediately with a 2s offset into its source.
		expect(chunks).toHaveLength(1);
		expect(chunks[0]?.bufferOffset).toBe(7);
		expect(chunks[0]?.duration).toBe(3);
	});

	it('applies per-region speed to source duration', () => {
		const regions: Region[] = [{ start: 0, end: 4, speed: 2 }];
		const chunks = planAudioSchedule(regions, 0);
		expect(chunks[0]?.duration).toBe(4);
		expect(chunks[0]?.rate).toBe(2);
		// 4 source seconds at 2x occupy 2 output seconds; the output-time
		// span is recorded for resync/debugging.
		expect(chunks[0]?.outEnd - chunks[0]?.outStart).toBe(2);
	});

	it('returns empty for empty input', () => {
		expect(planAudioSchedule([], 0)).toEqual([]);
		expect(planAudioSchedule([], 5)).toEqual([]);
	});

	it('skips degenerate regions (zero-length)', () => {
		expect(planAudioSchedule([{ start: 0, end: 0 }], 0)).toEqual([]);
	});

	it('defaults missing speed to 1', () => {
		const chunks = planAudioSchedule([{ start: 0, end: 4 }], 0);
		expect(chunks[0]?.rate).toBe(1);
	});

	it('skips non-positive speed (treated as 1×)', () => {
		const chunks = planAudioSchedule([{ start: 0, end: 4, speed: 0 }], 0);
		expect(chunks[0]?.rate).toBe(1);
	});
});

describe('planAudioScheduleWindow', () => {
	const regions: Region[] = [
		{ start: 0, end: 4 },
		{ start: 6, end: 10 },
	];

	it('equals planAudioSchedule on the scheduling-critical fields over the full window', () => {
		// outStart/outEnd differ by design (window clips them; planAudioSchedule keeps
		// the region's full span) — they're debug-only. The load-bearing fields match.
		const key = (c: ScheduledChunk) => ({ whenDelay: c.whenDelay, bufferOffset: c.bufferOffset, duration: c.duration, rate: c.rate });
		for (const from of [0, 2, 5, 8]) {
			expect(planAudioScheduleWindow(regions, from, from, Infinity).map(key)).toEqual(
				planAudioSchedule(regions, from).map(key),
			);
		}
	});

	it('clips regions to the output window and anchors whenDelay to the play-from time', () => {
		// Anchor at output 0, schedule only output [5, 7]. Region 2 occupies output
		// [4, 8] (source [6, 10]); the [5, 7] slice is source [7, 9].
		const chunks = planAudioScheduleWindow(regions, 0, 5, 7);
		expect(chunks).toHaveLength(1);
		expect(chunks[0]?.bufferOffset).toBeCloseTo(7, 6);
		expect(chunks[0]?.duration).toBeCloseTo(2, 6);
		expect(chunks[0]?.whenDelay).toBeCloseTo(5, 6); // anchored at 0, starts output 5
		expect(chunks[0]?.outStart).toBeCloseTo(5, 6);
		expect(chunks[0]?.outEnd).toBeCloseTo(7, 6);
	});

	it('returns nothing for an empty or inverted window', () => {
		expect(planAudioScheduleWindow(regions, 0, 5, 5)).toEqual([]);
		expect(planAudioScheduleWindow(regions, 0, 7, 5)).toEqual([]);
	});

	it('maps a speed-warped region to the correct source span', () => {
		const fast: Region[] = [{ start: 0, end: 8, speed: 2 }]; // output [0,4]
		const chunks = planAudioScheduleWindow(fast, 0, 1, 3);
		expect(chunks[0]?.rate).toBe(2);
		expect(chunks[0]?.bufferOffset).toBeCloseTo(2, 6); // 1 output * 2x
		expect(chunks[0]?.duration).toBeCloseTo(4, 6); // 2 output * 2x
	});
});

describe('missingRanges', () => {
	it('returns the whole window when nothing is resident', () => {
		expect(missingRanges([], 2, 8)).toEqual([{ start: 2, end: 8 }]);
	});

	it('returns nothing when a chunk fully covers the window', () => {
		expect(missingRanges([{ startSec: 0, durationSec: 10 }], 2, 8)).toEqual([]);
	});

	it('returns the gaps between resident chunks, clipped to the window', () => {
		const chunks: AudioChunk[] = [
			{ startSec: 0, durationSec: 3 },
			{ startSec: 6, durationSec: 3 },
		];
		expect(missingRanges(chunks, 2, 8)).toEqual([{ start: 3, end: 6 }]);
	});

	it('reports leading and trailing gaps', () => {
		const chunks: AudioChunk[] = [{ startSec: 4, durationSec: 2 }];
		expect(missingRanges(chunks, 2, 8)).toEqual([
			{ start: 2, end: 4 },
			{ start: 6, end: 8 },
		]);
	});
});

describe('outputToSource', () => {
	const regions: Region[] = [
		{ start: 0, end: 4 },
		{ start: 6, end: 10 },
	];

	it('maps output time into the source across a cut', () => {
		expect(outputToSource(regions, 0)).toBeCloseTo(0, 6);
		expect(outputToSource(regions, 3)).toBeCloseTo(3, 6);
		expect(outputToSource(regions, 4)).toBeCloseTo(4, 6); // boundary → end of region 1
		expect(outputToSource(regions, 5)).toBeCloseTo(7, 6); // 1s into region 2 (source 6)
	});

	it('applies region speed', () => {
		expect(outputToSource([{ start: 0, end: 8, speed: 2 }], 1)).toBeCloseTo(2, 6);
	});

	it('clamps past the end to the last region end', () => {
		expect(outputToSource(regions, 100)).toBeCloseTo(10, 6);
		expect(outputToSource([], 5)).toBe(0);
	});
});

function scheduled(over: Partial<ScheduledChunk>): ScheduledChunk {
	return { whenDelay: 0, bufferOffset: 0, duration: 0, rate: 1, outStart: 0, outEnd: 0, ...over };
}

describe('sliceChunksForPlayback', () => {
	it('plays entirely from a single chunk that covers the range', () => {
		const chunks: AudioChunk[] = [{ startSec: 0, durationSec: 10 }];
		const subs = sliceChunksForPlayback(scheduled({ bufferOffset: 2, duration: 4 }), chunks);
		expect(subs).toEqual([{ chunkIndex: 0, offsetInChunk: 2, playDuration: 4, whenDelay: 0, rate: 1 }]);
	});

	it('splits at a chunk boundary and offsets the second piece by the first piece output time', () => {
		const chunks: AudioChunk[] = [{ startSec: 0, durationSec: 5 }, { startSec: 5, durationSec: 5 }];
		const subs = sliceChunksForPlayback(scheduled({ bufferOffset: 3, duration: 4, whenDelay: 1 }), chunks);
		expect(subs).toEqual([
			{ chunkIndex: 0, offsetInChunk: 3, playDuration: 2, whenDelay: 1, rate: 1 },
			{ chunkIndex: 1, offsetInChunk: 0, playDuration: 2, whenDelay: 3, rate: 1 },
		]);
	});

	it('accounts for rate when offsetting the second piece (output time, not source)', () => {
		const chunks: AudioChunk[] = [{ startSec: 0, durationSec: 5 }, { startSec: 5, durationSec: 5 }];
		const subs = sliceChunksForPlayback(scheduled({ bufferOffset: 3, duration: 4, whenDelay: 1, rate: 2 }), chunks);
		expect(subs[1]?.whenDelay).toBeCloseTo(2, 6); // 1 + (5-3)/2
		expect(subs.every((s) => s.rate === 2)).toBe(true);
	});

	it('drops slices that fall in a hole in the store (silent gap, no mis-index)', () => {
		const chunks: AudioChunk[] = [{ startSec: 0, durationSec: 3 }, { startSec: 6, durationSec: 4 }];
		const subs = sliceChunksForPlayback(scheduled({ bufferOffset: 2, duration: 6 }), chunks);
		expect(subs).toEqual([
			{ chunkIndex: 0, offsetInChunk: 2, playDuration: 1, whenDelay: 0, rate: 1 },
			{ chunkIndex: 1, offsetInChunk: 0, playDuration: 2, whenDelay: 4, rate: 1 },
		]);
	});

	it('skips chunks that do not overlap the scheduled range', () => {
		const chunks: AudioChunk[] = [{ startSec: 20, durationSec: 5 }];
		expect(sliceChunksForPlayback(scheduled({ bufferOffset: 3, duration: 4 }), chunks)).toEqual([]);
	});

	it('reproduces the full duration when chunks tile the range contiguously', () => {
		const chunks: AudioChunk[] = [
			{ startSec: 0, durationSec: 2.5 },
			{ startSec: 2.5, durationSec: 2.5 },
			{ startSec: 5, durationSec: 2.5 },
		];
		const subs = sliceChunksForPlayback(scheduled({ bufferOffset: 1, duration: 5 }), chunks);
		const total = subs.reduce((s, p) => s + p.playDuration, 0);
		expect(total).toBeCloseTo(5, 6);
	});
});
