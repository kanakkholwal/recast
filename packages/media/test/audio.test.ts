import { describe, expect, it } from 'vitest';
import {
	keptRegions,
	planAudioSchedule,
	type Region,
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
