/**
 * Align a transcript's time base to the video/timeMap axis. The transcript is
 * timed against the AUDIO (real-time wall clock), but the editor's playhead maps
 * to VIDEO SOURCE time (the recording is count-based CFR, so its frame-count
 * duration is slightly shorter than wall clock — see recording-fps notes). Left
 * uncorrected, captions drift up to that whole-clip gap, growing toward the end.
 * Scaling every timestamp by videoDuration/audioDuration removes the linear drift.
 */

import type { Transcript } from "$lib/ipc";

/** The correction factor, or 1 (identity) when it can't be trusted. Clamped to a
 *  small window so a bad/absent probe never mistimes captions — a real CFR gap is
 *  well under a few percent. */
export function transcriptTimeScale(
	videoDurationSec: number | null | undefined,
	audioDurationSec: number | null | undefined,
): number {
	if (!videoDurationSec || !audioDurationSec || audioDurationSec <= 0) return 1;
	const scale = videoDurationSec / audioDurationSec;
	if (!(scale > 0) || Math.abs(scale - 1) > 0.05) return 1;
	return scale;
}

/** Return a transcript with every segment/word time multiplied by `scale`. The
 *  SAME object is returned for an identity scale, so callers keep referential
 *  stability (no needless re-render / re-alloc). */
export function scaleTranscript(transcript: Transcript | null, scale: number): Transcript | null {
	if (!transcript || !(scale > 0) || Math.abs(scale - 1) < 1e-6) return transcript;
	return {
		...transcript,
		segments: transcript.segments.map((s) => ({
			...s,
			start: s.start * scale,
			end: s.end * scale,
			words: s.words?.map((w) => ({ ...w, start: w.start * scale, end: w.end * scale })) ?? s.words,
		})),
	};
}
