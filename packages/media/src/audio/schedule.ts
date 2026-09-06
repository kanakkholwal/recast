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

/**
 * Like `planAudioSchedule`, but plans only the slice of output time
 * `[windowStart, windowEnd]` and anchors `whenDelay` to a fixed `anchorOutput`
 * (the play-from time), not the window start. This lets a streaming engine
 * schedule the timeline in disjoint slices — decoding + firing sources a window
 * ahead of the playhead and evicting behind — instead of materialising the whole
 * recording's PCM upfront. `planAudioScheduleWindow(r, from, from, Infinity)`
 * equals `planAudioSchedule(r, from)`.
 */
export function planAudioScheduleWindow(
	regions: ReadonlyArray<Region>,
	anchorOutput: number,
	windowStart: number,
	windowEnd: number,
): ScheduledChunk[] {
	const out: ScheduledChunk[] = [];
	if (windowEnd - windowStart <= EPS) return out;
	let outCursor = 0;
	for (const region of regions) {
		const sourceDur = region.end - region.start;
		if (sourceDur <= EPS) continue;
		const rate = region.speed && region.speed > 0 ? region.speed : 1;
		const outStart = outCursor;
		const outEnd = outCursor + sourceDur / rate;
		outCursor = outEnd;
		const clipStart = Math.max(outStart, windowStart);
		const clipEnd = Math.min(outEnd, windowEnd);
		if (clipEnd - clipStart <= EPS) continue;
		const sourceInto = (clipStart - outStart) * rate;
		out.push({
			whenDelay: Math.max(0, clipStart - anchorOutput),
			bufferOffset: region.start + sourceInto,
			duration: (clipEnd - clipStart) * rate,
			rate,
			outStart: clipStart,
			outEnd: clipEnd,
		});
	}
	return out;
}

/** A decoded audio chunk resident in the store, covering source time
 *  `[startSec, startSec + durationSec)`. Chunks tile the source contiguously
 *  and are sorted by `startSec` — the streaming decode fills them in order. */
export interface AudioChunk {
	/** Source-time (original-recording seconds) of the chunk's first sample. */
	startSec: number;
	/** Source-time length of the chunk. */
	durationSec: number;
}

/** One `AudioBufferSourceNode` to fire so a scheduled chunk plays from the
 *  chunk store: play `playDuration` SOURCE seconds of chunk `chunkIndex`'s
 *  buffer from `offsetInChunk`, starting `whenDelay` OUTPUT-seconds from now,
 *  at `rate`. */
export interface SubPlay {
	chunkIndex: number;
	offsetInChunk: number;
	playDuration: number;
	whenDelay: number;
	rate: number;
}

/** Sub-ranges of `[from, to]` NOT covered by the resident (sorted, possibly
 *  gapped) chunks — the source ranges a streaming store still needs to decode.
 *  Lets the store skip re-decoding audio it already holds. */
export function missingRanges(
	chunks: ReadonlyArray<AudioChunk>,
	from: number,
	to: number,
): Array<{ start: number; end: number }> {
	const out: Array<{ start: number; end: number }> = [];
	if (to - from <= EPS) return out;
	let cursor = from;
	for (const c of chunks) {
		const cEnd = c.startSec + c.durationSec;
		if (cEnd <= cursor + EPS) continue;
		if (c.startSec >= to - EPS) break;
		if (c.startSec > cursor + EPS) out.push({ start: cursor, end: Math.min(c.startSec, to) });
		cursor = Math.max(cursor, cEnd);
		if (cursor >= to - EPS) break;
	}
	if (cursor < to - EPS) out.push({ start: cursor, end: to });
	return out;
}

/** SOURCE-time position at gapless OUTPUT time `outputSec`, walking the kept
 *  regions (speed-warped). Used to pick the eviction boundary: source audio well
 *  behind the playhead can be dropped. Clamps to the last region's end. */
export function outputToSource(regions: ReadonlyArray<Region>, outputSec: number): number {
	let outCursor = 0;
	for (const r of regions) {
		const rate = r.speed && r.speed > 0 ? r.speed : 1;
		const outDur = (r.end - r.start) / rate;
		if (outputSec <= outCursor + outDur + EPS) {
			return r.start + Math.max(0, outputSec - outCursor) * rate;
		}
		outCursor += outDur;
	}
	const last = regions[regions.length - 1];
	return last ? last.end : 0;
}

/**
 * Split one `ScheduledChunk` across the decoded chunk store. The scheduled chunk
 * plays source range `[bufferOffset, bufferOffset+duration]` at `rate` starting
 * at output time `whenDelay`; this cuts it at chunk boundaries so each piece
 * plays from its own resident buffer. When the chunks tile the source
 * contiguously the sub-plays reproduce the old single-big-buffer playback
 * sample-for-sample; a hole in the store simply drops that slice (silent gap)
 * rather than mis-indexing.
 */
export function sliceChunksForPlayback(
	scheduled: ScheduledChunk,
	chunks: ReadonlyArray<AudioChunk>,
): SubPlay[] {
	const out: SubPlay[] = [];
	const playStart = scheduled.bufferOffset;
	const playEnd = scheduled.bufferOffset + scheduled.duration;
	const rate = scheduled.rate > 0 ? scheduled.rate : 1;
	for (let i = 0; i < chunks.length; i++) {
		const chunk = chunks[i];
		const overlapStart = Math.max(playStart, chunk.startSec);
		const overlapEnd = Math.min(playEnd, chunk.startSec + chunk.durationSec);
		if (overlapEnd - overlapStart <= EPS) continue;
		out.push({
			chunkIndex: i,
			offsetInChunk: overlapStart - chunk.startSec,
			playDuration: overlapEnd - overlapStart,
			// The earlier source part has already played by the time this sub-play starts, which at `rate` is this much output time.
			whenDelay: scheduled.whenDelay + (overlapStart - playStart) / rate,
			rate,
		});
	}
	return out;
}
