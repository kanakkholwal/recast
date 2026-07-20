/**
 * Pure scheduling math for the audio timeline (no `AudioContext`, no DOM).
 * Lives in `@recast/media` so both the worklet-backed scheduler and the
 * legacy fallback can share the same `keptRegions → planAudioSchedule` math
 * the editor's preview already uses.
 *
 * The recording's audio covers the FULL original timeline. To play the
 * EDITED timeline we don't seek a media element around the cuts (fragile,
 * drifts, stalls); instead we schedule each KEPT region as its own audio
 * source on the audio clock, so the cuts become gaps in the schedule:
 * sample-accurate and silent by construction.
 */

const EPS = 1e-4;

/** An interval on some timeline. */
export interface Region {
	/** Start (seconds). */
	start: number;
	/** End (seconds). */
	end: number;
	/** Playback speed (>0); 1 = normal. A 2× region plays its audio twice as
	 *  fast (and occupies half the output time), matching the per-segment clip
	 *  speed. Optional; absent/≤0 means 1×. */
	speed?: number;
}

/**
 * Kept original-time audio regions = `[inPoint, outPoint]` minus `cuts`.
 * Cuts are clipped to the trim range, merged, and removed; the surviving
 * gaps are the regions.
 */
export function keptRegions(
	inPoint: number,
	outPoint: number,
	cuts: ReadonlyArray<Region>,
): Region[] {
	if (outPoint - inPoint <= EPS) return [];

	const clipped = cuts
		.map((c) => ({
			start: Math.max(inPoint, Math.min(c.start, c.end)),
			end: Math.min(outPoint, Math.max(c.start, c.end)),
		}))
		.filter((c) => c.end - c.start > EPS)
		.sort((a, b) => a.start - b.start);

	const merged: Region[] = [];
	for (const c of clipped) {
		const last = merged[merged.length - 1];
		if (last && c.start <= last.end + EPS) last.end = Math.max(last.end, c.end);
		else merged.push({ ...c });
	}

	const regions: Region[] = [];
	let cursor = inPoint;
	for (const c of merged) {
		if (c.start > cursor + EPS) regions.push({ start: cursor, end: c.start });
		cursor = Math.max(cursor, c.end);
	}
	if (outPoint > cursor + EPS) regions.push({ start: cursor, end: outPoint });
	return regions;
}

/** One scheduled audio chunk: play `duration` SOURCE seconds of the buffer from
 *  `bufferOffset` at `rate`, starting `whenDelay` output-seconds from "now". The
 *  audio clock runs in output (== wall) time, so a 2× chunk consumes twice the
 *  source per output-second and is positioned on the warped output axis. */
export interface ScheduledChunk {
	/** Output-seconds from now to begin this chunk (0 = immediately, mid-region). */
	whenDelay: number;
	/** Offset into the (original-time) audio buffer to start playing from. */
	bufferOffset: number;
	/** SOURCE seconds of buffer to play (wall time = duration / rate). */
	duration: number;
	/** Playback rate (= region speed). */
	rate: number;
	/** Output-time span this chunk occupies (for resync/debugging). */
	outStart: number;
	outEnd: number;
}

/**
 * Plan the chunks to schedule so playback continues from OUTPUT time
 * `fromOutputTime`. Output time is gapless and SPEED-WARPED: a region of source
 * length L at speed s occupies L/s on the output axis. Region N starts where
 * region N-1 ended. Regions fully behind the playhead are skipped; the region
 * the playhead is inside starts immediately (`whenDelay` 0) at the right offset.
 */
export function planAudioSchedule(
	regions: ReadonlyArray<Region>,
	fromOutputTime: number,
): ScheduledChunk[] {
	const out: ScheduledChunk[] = [];
	let outCursor = 0;
	for (const region of regions) {
		const sourceDur = region.end - region.start;
		if (sourceDur <= EPS) continue;
		const rate = region.speed && region.speed > 0 ? region.speed : 1;
		const outDur = sourceDur / rate;
		const outStart = outCursor;
		const outEnd = outCursor + outDur;
		outCursor = outEnd;
		if (outEnd <= fromOutputTime + EPS) continue; // already played

		const intoOutput = Math.max(0, fromOutputTime - outStart);
		const whenDelay = Math.max(0, outStart - fromOutputTime);
		const sourceInto = intoOutput * rate;
		const duration = sourceDur - sourceInto;
		if (duration <= EPS) continue;
		out.push({
			whenDelay,
			bufferOffset: region.start + sourceInto,
			duration,
			rate,
			outStart,
			outEnd,
		});
	}
	return out;
}