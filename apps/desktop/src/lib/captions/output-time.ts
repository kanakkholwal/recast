/**
 * Transcript → OUTPUT timeline mapping, for anything that ships caption timings
 * alongside the edited video (SRT/VTT sidecars, the Cloud caption track).
 *
 * The transcript is in SOURCE time; the exported video is on the output axis
 * (trim + cuts + per-segment speed). Lives here rather than in the export
 * service because it's pure caption math over ./clip-with-cuts, and the service
 * layer pulls in Tauri + analytics that unit tests can't load.
 */

import type { Transcript } from '$lib/ipc';
import { originalToOutput, type TimeMap } from '$lib/timeline/time-map';
import { splitSegmentAcrossSpans } from './clip-with-cuts';

/** Map a transcript onto the OUTPUT timeline (trim + cuts + per-segment speed)
 *  so sidecar timings line up with the exported video, not the raw recording. */
export function toOutputTimeTranscript(map: TimeMap, src: Transcript): Transcript {
	const at = (t: number) => originalToOutput(map, t);
	// Split against the kept spans FIRST, then map. Remapping a segment's two
	// endpoints alone can't express a cue that a cut broke in half: both
	// endpoints collapse onto their nearest seam and the cue silently stretches
	// across removed time, carrying words the export dropped. Splitting first
	// means every emitted cue lies wholly inside one kept span, so the mapping
	// is linear over it and cues can't overlap or reorder.
	const segments: Transcript['segments'] = [];
	for (const seg of src.segments) {
		for (const piece of splitSegmentAcrossSpans(seg, map.spans)) {
			const words = piece.words
				.map((w) => ({ ...w, start: at(w.start), end: at(w.end) }))
				.filter((w) => w.end - w.start > 0);
			const start = at(piece.start);
			const end = at(piece.end);
			if (end - start <= 0.01) continue;
			segments.push({
				...seg,
				// A split piece is its own cue and needs its own id (consumers key
				// on it) and its own text — the half of the line actually spoken
				// here, not the whole original line repeated on both sides.
				id: piece.split ? `${seg.id}:${piece.spanIndex}` : seg.id,
				text: piece.split && words.length > 0 ? words.map((w) => w.text).join(' ').trim() : seg.text,
				start,
				end,
				words,
			});
		}
	}
	return { ...src, segments };
}
