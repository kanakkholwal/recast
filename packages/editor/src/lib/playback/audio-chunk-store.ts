/**
 * Streaming audio chunk store: decodes a source's audio in windows via
 * MediaBunny's `AudioBufferSink` and keeps only the chunks near the playhead
 * resident, instead of decoding the whole file to one `AudioBuffer` (hundreds of
 * MB of PCM for a long recording — the CON-3 memory/crash risk). The audio
 * engine decodes a window ahead of the playhead and evicts behind it.
 *
 * Chunk indices in `chunks()` line up with `buffer(i)` only until the next
 * `ensureRange`/`evictOutside`; the engine slices + reads buffers synchronously
 * after each `await ensureRange`, so the order never shifts mid-use.
 */

import { type AudioChunk, type MediaRef, missingRanges, toMediaRef } from "@recast/media";
import { ALL_FORMATS, AudioBufferSink, Input, mediaRefSource } from "@recast/media/mediabunny";

interface Resident {
	startSec: number;
	durationSec: number;
	buffer: AudioBuffer;
}

const SAME_START_EPS = 1e-4;

/** Chunks overlapping `[startSec, endSec]`. An inverted range keeps everything,
 *  so a bad bound degrades to the old retain-all rather than silencing audio. */
export function keepInWindow<T extends { startSec: number; durationSec: number }>(
	resident: readonly T[],
	startSec: number,
	endSec: number,
): T[] {
	if (endSec < startSec) return [...resident];
	return resident.filter((r) => r.startSec + r.durationSec > startSec && r.startSec < endSec);
}

/**
 * File-time range to decode for a timeline-time range. `offsetSec` is how far
 * this track's first sample sits after video frame 0, so file time is timeline
 * time minus the offset, clamped at the start of the file.
 */
export function fileRangeFor(
	fromSec: number,
	toSec: number,
	offsetSec: number,
): { start: number; end: number } {
	const shift = Number.isFinite(offsetSec) ? offsetSec : 0;
	const start = Math.max(0, fromSec - shift);
	return { start, end: Math.max(start, toSec - shift) };
}

export class AudioChunkStore {
	#input: Input;
	#sink: AudioBufferSink;
	/** Resident decoded chunks in TIMELINE time, sorted by `startSec`. */
	#resident: Resident[] = [];
	/** Seconds this track's first sample lands after video frame 0. */
	#offsetSec = 0;

	private constructor(input: Input, sink: AudioBufferSink) {
		this.#input = input;
		this.#sink = sink;
	}

	/** Place this track on the timeline. Changing it invalidates what is
	 *  resident, since chunks are stored already translated. */
	setOffsetSec(offsetSec: number): void {
		const next = Number.isFinite(offsetSec) ? offsetSec : 0;
		if (next === this.#offsetSec) return;
		this.#offsetSec = next;
		this.#resident = [];
	}

	/** Open the source and its primary audio track. Null when there's no audio
	 *  track (caller skips this source). */
	static async create(src: MediaRef | Blob | string): Promise<AudioChunkStore | null> {
		const input = new Input({ source: mediaRefSource(toMediaRef(src)), formats: ALL_FORMATS });
		try {
			const track = await input.getPrimaryAudioTrack();
			if (!track) {
				input.dispose();
				return null;
			}
			return new AudioChunkStore(input, new AudioBufferSink(track));
		} catch (err) {
			input.dispose();
			throw err;
		}
	}

	/** Resident chunks as `AudioChunk`s for the pure scheduler. Index i maps to
	 *  `buffer(i)`. */
	chunks(): AudioChunk[] {
		return this.#resident.map((r) => ({ startSec: r.startSec, durationSec: r.durationSec }));
	}

	buffer(index: number): AudioBuffer | null {
		return this.#resident[index]?.buffer ?? null;
	}

	/** Decode any part of source `[from, to]` not already resident. */
	async ensureRange(from: number, to: number, signal?: AbortSignal): Promise<void> {
		for (const gap of missingRanges(this.chunks(), from, to)) {
			const file = fileRangeFor(gap.start, gap.end, this.#offsetSec);
			if (file.end <= file.start) continue;
			for await (const w of this.#sink.buffers(file.start, file.end)) {
				if (signal?.aborted) return;
				this.#insert(w.timestamp + this.#offsetSec, w.duration, w.buffer);
			}
		}
	}

	#insert(startSec: number, durationSec: number, buffer: AudioBuffer): void {
		let i = this.#resident.length;
		while (i > 0 && this.#resident[i - 1].startSec > startSec) i--;
		// A neighbouring window already yielded this chunk — don't double-insert.
		if (i > 0 && Math.abs(this.#resident[i - 1].startSec - startSec) < SAME_START_EPS) return;
		if (
			i < this.#resident.length &&
			Math.abs(this.#resident[i].startSec - startSec) < SAME_START_EPS
		)
			return;
		this.#resident.splice(i, 0, { startSec, durationSec, buffer });
	}

	/** Drop chunks lying entirely outside `[startSec, endSec]`. A
	 *  `BufferSourceNode` already started keeps its own buffer alive, so this only
	 *  releases our reference.
	 *
	 *  Two-sided on purpose: the old `evictBefore` kept everything AHEAD of the
	 *  playhead, so every backward seek stranded the window decoded at the old
	 *  position — ~16 s per jump, for the rest of the session. Evicting too
	 *  eagerly only costs a re-decode, since `ensureRange` is awaited before
	 *  anything is scheduled. */
	evictOutside(startSec: number, endSec: number): void {
		this.#resident = keepInWindow(this.#resident, startSec, endSec);
	}

	dispose(): void {
		this.#resident = [];
		this.#input.dispose();
	}
}
