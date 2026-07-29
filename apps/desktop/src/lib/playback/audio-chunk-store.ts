/**
 * Streaming audio chunk store: decodes a source's audio in windows via
 * MediaBunny's `AudioBufferSink` and keeps only the chunks near the playhead
 * resident, instead of decoding the whole file to one `AudioBuffer` (hundreds of
 * MB of PCM for a long recording — the CON-3 memory/crash risk). The audio
 * engine decodes a window ahead of the playhead and evicts behind it.
 *
 * Chunk indices in `chunks()` line up with `buffer(i)` only until the next
 * `ensureRange`/`evictBefore`; the engine slices + reads buffers synchronously
 * after each `await ensureRange`, so the order never shifts mid-use.
 */

import { ALL_FORMATS, AudioBufferSink, Input, UrlSource } from "@recast/media/mediabunny";
import { missingRanges, type AudioChunk } from "@recast/media";

interface Resident {
	startSec: number;
	durationSec: number;
	buffer: AudioBuffer;
}

const SAME_START_EPS = 1e-4;

export class AudioChunkStore {
	#input: Input;
	#sink: AudioBufferSink;
	/** Resident decoded chunks, sorted by `startSec` (may have gaps). */
	#resident: Resident[] = [];

	private constructor(input: Input, sink: AudioBufferSink) {
		this.#input = input;
		this.#sink = sink;
	}

	/** Open the source and its primary audio track. Null when there's no audio
	 *  track (caller skips this source). */
	static async create(url: string): Promise<AudioChunkStore | null> {
		const input = new Input({ source: new UrlSource(url), formats: ALL_FORMATS });
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
			for await (const w of this.#sink.buffers(gap.start, gap.end)) {
				if (signal?.aborted) return;
				this.#insert(w.timestamp, w.duration, w.buffer);
			}
		}
	}

	#insert(startSec: number, durationSec: number, buffer: AudioBuffer): void {
		let i = this.#resident.length;
		while (i > 0 && this.#resident[i - 1].startSec > startSec) i--;
		// A neighbouring window already yielded this chunk — don't double-insert.
		if (i > 0 && Math.abs(this.#resident[i - 1].startSec - startSec) < SAME_START_EPS) return;
		if (i < this.#resident.length && Math.abs(this.#resident[i].startSec - startSec) < SAME_START_EPS) return;
		this.#resident.splice(i, 0, { startSec, durationSec, buffer });
	}

	/** Drop chunks that end at or before `beforeSec` (played, out of window). A
	 *  `BufferSourceNode` already started keeps its own buffer alive, so this only
	 *  releases our reference. */
	evictBefore(beforeSec: number): void {
		this.#resident = this.#resident.filter((r) => r.startSec + r.durationSec > beforeSec);
	}

	dispose(): void {
		this.#resident = [];
		this.#input.dispose();
	}
}
