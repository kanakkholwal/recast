import { describe, expect, it } from 'vitest';
import type { TranscriptSegment, TranscriptWord } from '$lib/ipc';
import {
	activeClippedSegment,
	clipSegmentToSpan,
	clipWordsToSpan,
	type KeptSpan,
} from './clip-with-cuts';

const SPAN: KeptSpan = { origStart: 5, origEnd: 10 };

function seg(start: number, end: number, words: TranscriptWord[] = []): TranscriptSegment {
	return { id: `s-${start}-${end}`, start, end, text: '', words };
}

function word(start: number, end: number, text = ''): TranscriptWord {
	return { start, end, text };
}

describe('clipSegmentToSpan', () => {
	it('returns null when the segment is entirely before the span', () => {
		expect(clipSegmentToSpan(seg(1, 3), SPAN)).toBeNull();
	});

	it('returns null when the segment is entirely after the span', () => {
		expect(clipSegmentToSpan(seg(11, 14), SPAN)).toBeNull();
	});

	it('returns the original segment when fully inside the span', () => {
		expect(clipSegmentToSpan(seg(6, 8), SPAN)).toEqual({ start: 6, end: 8 });
	});

	it('clips the start when the segment begins before the span', () => {
		// Segment [3, 7] crossing the span start (5): visible = [5, 7]
		expect(clipSegmentToSpan(seg(3, 7), SPAN)).toEqual({ start: 5, end: 7 });
	});

	it('clips the end when the segment ends after the span', () => {
		// Segment [8, 12] crossing the span end (10): visible = [8, 10]
		expect(clipSegmentToSpan(seg(8, 12), SPAN)).toEqual({ start: 8, end: 10 });
	});

	it('clips both ends when the segment surrounds the span', () => {
		// Segment [2, 14] fully surrounding the span: visible = [5, 10]
		expect(clipSegmentToSpan(seg(2, 14), SPAN)).toEqual({ start: 5, end: 10 });
	});

	it('returns null when the segment is degenerate after clipping', () => {
		// Segment [4, 5] touching the span start exactly: visible = [5, 5] → null
		expect(clipSegmentToSpan(seg(4, 5), SPAN)).toBeNull();
	});
});

describe('clipWordsToSpan', () => {
	it('keeps words fully inside the span unchanged', () => {
		const words = [word(5.5, 6, 'a'), word(6.5, 7, 'b'), word(8, 9, 'c')];
		expect(clipWordsToSpan(words, SPAN)).toEqual(words);
	});

	it('drops words entirely before the span', () => {
		const words = [word(1, 2, 'a'), word(2.5, 3, 'b'), word(6, 7, 'c')];
		const out = clipWordsToSpan(words, SPAN);
		expect(out).toHaveLength(1);
		expect(out[0]?.text).toBe('c');
	});

	it('drops words entirely after the span', () => {
		const words = [word(6, 7, 'a'), word(11, 12, 'b')];
		const out = clipWordsToSpan(words, SPAN);
		expect(out).toHaveLength(1);
		expect(out[0]?.text).toBe('a');
	});

	it('clips words that span the span boundary', () => {
		// Word [3, 7] crosses the span start: clip start to 5, keep end at 7
		const words = [word(3, 7, 'a')];
		const out = clipWordsToSpan(words, SPAN);
		expect(out).toEqual([{ start: 5, end: 7, text: 'a' }]);
	});

	it('returns empty when no words survive', () => {
		const words = [word(1, 4, 'a'), word(11, 14, 'b')];
		expect(clipWordsToSpan(words, SPAN)).toEqual([]);
	});
});

describe('activeClippedSegment', () => {
	const segments: TranscriptSegment[] = [
		seg(1, 3, [word(1, 2, 'first')]),
		seg(4, 8, [word(4, 5, 'middle-a'), word(5.5, 7, 'middle-b'), word(7, 8, 'middle-c')]),
		seg(8, 12, [word(8, 10, 'last-a'), word(10, 12, 'last-b')]),
	];

	it('returns the segment clipped to the kept span when nowOrig is inside', () => {
		// nowOrig = 6.0 is inside segment [4, 8] which spans [5, 10] → visible = [5, 8]
		const result = activeClippedSegment(segments, SPAN, 6.0);
		expect(result).not.toBeNull();
		expect(result?.visible).toEqual({ start: 5, end: 8 });
		expect(result?.segment.start).toBe(4);
	});

	it('returns null when nowOrig is inside a cut (between kept spans)', () => {
		// SPAN starts at 5; nowOrig = 4 is before it
		expect(activeClippedSegment(segments, SPAN, 4.0)).toBeNull();
	});

	it('returns null when no segment overlaps the visible window', () => {
		// nowOrig = 5.5 is inside the visible window but the only segment
		// overlapping it would need to be the [4, 8] one. Verify with a
		// span that excludes all segments.
		const tightSpan: KeptSpan = { origStart: 3.5, origEnd: 3.8 };
		expect(activeClippedSegment(segments, tightSpan, 3.6)).toBeNull();
	});

	it('returns the segment clipped to a single boundary crossing the cut', () => {
		// Segment [4, 8] crosses the cut at 5. Inside the kept span [5, 10]
		// only [5, 8] is visible. nowOrig = 5.5 is in the visible window.
		const result = activeClippedSegment(segments, SPAN, 5.5);
		expect(result?.visible).toEqual({ start: 5, end: 8 });
	});

	it('returns the segment clipped at the other boundary too', () => {
		// Segment [8, 12] crosses the cut end at 10. Visible = [8, 10].
		// nowOrig = 9.0 is in the visible window.
		const result = activeClippedSegment(segments, SPAN, 9.0);
		expect(result?.visible).toEqual({ start: 8, end: 10 });
	});
});
