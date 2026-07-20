import { afterEach, describe, expect, it, vi } from 'vitest';
import {
	keptRegions,
	planAudioSchedule,
	type Region,
} from '../src/audio/schedule';
import { createAudioScheduler } from '../src/audio/scheduler';

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
/**
 * `load()` used to run an unbounded fetch/decode loop with no cancellation,
 * so an editor close mid-load kept fetching and leaked the AudioContext.
 */
describe('AudioScheduler.load cancellation', () => {
	function stubAudio(onClose: () => void) {
		class FakeCtx {
			destination = {};
			createGain() {
				return { connect() {}, gain: { value: 1 } };
			}
			async decodeAudioData() {
				return { numberOfChannels: 1, sampleRate: 48000, length: 1, getChannelData: () => new Float32Array(1) };
			}
			async close() {
				onClose();
			}
		}
		vi.stubGlobal('AudioContext', FakeCtx);
	}

	afterEach(() => vi.unstubAllGlobals());

	it('rejects with MediaError(cancelled) when the signal aborts first', async () => {
		let closed = false;
		stubAudio(() => {
			closed = true;
		});
		vi.stubGlobal('fetch', async () => ({ ok: true, arrayBuffer: async () => new ArrayBuffer(8) }));
		const scheduler = await createAudioScheduler();
		const ac = new AbortController();
		ac.abort();
		await expect(scheduler.load(['a.wav'], ac.signal)).rejects.toMatchObject({
			name: 'MediaError',
			code: 'cancelled',
		});
		expect(closed).toBe(true);
	});

	it('converts a mid-flight fetch abort into MediaError(cancelled)', async () => {
		let closed = false;
		stubAudio(() => {
			closed = true;
		});
		const ac = new AbortController();
		vi.stubGlobal('fetch', async (_url: string, init?: { signal?: AbortSignal }) => {
			ac.abort();
			throw Object.assign(new Error('aborted'), { name: 'AbortError', signal: init?.signal });
		});
		const scheduler = await createAudioScheduler();
		await expect(scheduler.load(['a.wav'], ac.signal)).rejects.toMatchObject({
			name: 'MediaError',
			code: 'cancelled',
		});
		expect(closed).toBe(true);
	});

	it('still throws bad-input (not cancelled) when nothing decodes', async () => {
		stubAudio(() => {});
		vi.stubGlobal('fetch', async () => ({ ok: false }));
		const scheduler = await createAudioScheduler();
		await expect(scheduler.load(['a.wav'])).rejects.toMatchObject({ code: 'bad-input' });
	});
});
