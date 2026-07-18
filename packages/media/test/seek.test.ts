import { describe, expect, it } from 'vitest';
import { nextCutWithin, snapToSeekTarget } from '../src/seek';

/**
 * Pure-function tests for the seek helpers. Run as a Node test (no DOM,
 * no worker) so they stay fast.
 */
describe('snapToSeekTarget', () => {
	it('returns the input unchanged when no cuts overlap', () => {
		expect(snapToSeekTarget(5, [])).toBe(5);
		expect(snapToSeekTarget(5, [{ start: 10, end: 12 }])).toBe(5);
	});

	it('snaps inside a cut to the cut end', () => {
		expect(snapToSeekTarget(15, [{ start: 10, end: 20 }])).toBe(20);
		expect(
			snapToSeekTarget(12.5, [
				{ start: 0, end: 5 },
				{ start: 10, end: 20 },
			]),
		).toBe(20);
	});

	it('snaps when exactly on a cut start (inclusive)', () => {
		// The inside-cut predicate is `seconds >= cut.start && seconds < cut.end`,
		// so a seek that lands exactly on the cut start is still inside. The
		// caller is expected to clamp to `floorSec` first if it wants to
		// distinguish.
		expect(snapToSeekTarget(10, [{ start: 10, end: 20 }])).toBe(20);
		expect(snapToSeekTarget(20, [{ start: 10, end: 20 }])).toBe(20);
	});
});

describe('nextCutWithin', () => {
	it('returns null when no cut is in the lookahead window', () => {
		expect(
			nextCutWithin(0, 5, [{ start: 10, end: 12 }]),
		).toBeNull();
		expect(
			nextCutWithin(15, 1, [{ start: 10, end: 12 }]),
		).toBeNull();
	});

	it('returns the next cut whose start is in (seconds, seconds+lookahead]', () => {
		expect(
			nextCutWithin(10, 5, [{ start: 12, end: 14 }]),
		).toEqual({ start: 12, end: 14 });
	});

	it('returns the earliest such cut when several overlap', () => {
		// The cuts array is conventionally in start-time order (the timeline
		// builds it that way); we honor that ordering.
		const result = nextCutWithin(10, 10, [
			{ start: 12, end: 14 },
			{ start: 13, end: 15 },
		]);
		expect(result).toEqual({ start: 12, end: 14 });
	});

	it('excludes cuts entirely in the past', () => {
		expect(
			nextCutWithin(20, 10, [{ start: 5, end: 7 }, { start: 25, end: 27 }]),
		).toEqual({ start: 25, end: 27 });
	});
});