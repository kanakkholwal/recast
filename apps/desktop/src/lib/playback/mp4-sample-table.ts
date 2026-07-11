/**
 * Pure MP4 sample-table arithmetic for the WebCodecs progressive (HTTP-range)
 * path, split out so it's unit-testable without a real decoder/MP4/mp4box.
 *
 * Progressive mode fetches only the `moov` index first, then each GOP's bytes on
 * demand, which needs the full per-sample map (time, duration, sync flag, and
 * the byte offset+size) reconstructed from the ISO-BMFF `stbl` tables:
 *
 *   - stsz  → sample sizes (or one constant size)
 *   - stsc  → sample-to-chunk runs (how many samples each chunk holds)
 *   - stco / co64 → the file byte offset of each chunk
 *   - stts  → decode-time deltas (run-length)
 *   - ctts  → composition-time offsets (B-frame reordering; run-length, signed)
 *   - stss  → which samples are sync samples (keyframes); absent ⇒ all are
 *
 * Output times are microseconds; samples are in DECODE order.
 */

/** The raw `stbl` arrays we read off the parsed mp4box boxes. */
export interface RawSampleTables {
	/** Per-sample sizes (stsz.sample_sizes). Empty when a constant size is used. */
	sampleSizes: ReadonlyArray<number>;
	/** Constant sample size (stsz.sample_size); used only when sampleSizes is empty. */
	sampleSizeConstant: number;
	/** Total sample count (stsz.sample_count). */
	sampleCount: number;
	/** Byte offset of each chunk in the file (stco/co64.chunk_offsets). */
	chunkOffsets: ReadonlyArray<number>;
	/** stsc first_chunk (1-based) per run. */
	stscFirstChunk: ReadonlyArray<number>;
	/** stsc samples_per_chunk per run (parallel to stscFirstChunk). */
	stscSamplesPerChunk: ReadonlyArray<number>;
	/** stts sample_counts per run. */
	sttsCounts: ReadonlyArray<number>;
	/** stts sample_deltas per run (parallel to sttsCounts). */
	sttsDeltas: ReadonlyArray<number>;
	/** ctts sample_counts per run (optional). */
	cttsCounts?: ReadonlyArray<number>;
	/** ctts sample_offsets per run, signed (optional, parallel to cttsCounts). */
	cttsOffsets?: ReadonlyArray<number>;
	/** stss 1-based sync-sample numbers. Absent/empty ⇒ every sample is sync. */
	stssSampleNumbers?: ReadonlyArray<number>;
	/** Media timescale (mdhd timescale), units per second. */
	timescale: number;
}

/** One sample in DECODE order, with its byte location and timing in µs. */
export interface SampleEntry {
	/** Presentation timestamp (µs). */
	ctsUs: number;
	/** Duration (µs). */
	durUs: number;
	/** Sync sample (valid decode entry point / keyframe). */
	key: boolean;
	/** Byte offset of the sample's media bytes in the file. */
	offset: number;
	/** Size of the sample's media bytes. */
	size: number;
}

/** Expand a run-length list (counts[k] copies of values[k]) into a flat array. */
function expandRuns(
	counts: ReadonlyArray<number>,
	values: ReadonlyArray<number>,
	total: number,
): number[] {
	const out: number[] = [];
	for (let k = 0; k < counts.length && out.length < total; k++) {
		const c = counts[k];
		const v = values[k];
		for (let i = 0; i < c && out.length < total; i++) out.push(v);
	}
	// Pad with the last value if the table under-counts (defensive; valid files
	// cover every sample).
	const last = values.length ? values[values.length - 1] : 0;
	while (out.length < total) out.push(last);
	return out;
}

/**
 * Samples-per-chunk for each chunk (1-based chunk index → count), expanded from
 * the stsc run table. Run k applies to chunks [firstChunk[k], firstChunk[k+1]).
 */
function samplesPerChunk(
	firstChunk: ReadonlyArray<number>,
	perChunk: ReadonlyArray<number>,
	chunkCount: number,
): number[] {
	const out = new Array<number>(chunkCount + 1).fill(0); // 1-based; [0] unused
	if (firstChunk.length === 0) {
		for (let c = 1; c <= chunkCount; c++) out[c] = 1;
		return out;
	}
	for (let k = 0; k < firstChunk.length; k++) {
		const start = firstChunk[k];
		const end = k + 1 < firstChunk.length ? firstChunk[k + 1] : chunkCount + 1;
		for (let c = start; c < end && c <= chunkCount; c++) out[c] = perChunk[k];
	}
	return out;
}

/**
 * Reconstruct the full sample map (decode order) from the parsed `stbl` arrays.
 * Returns `[]` when the table is empty or inconsistent (caller falls back).
 */
export function buildSampleTable(t: RawSampleTables): SampleEntry[] {
	const n = t.sampleCount;
	if (n <= 0 || t.chunkOffsets.length === 0) return [];
	const ts = t.timescale > 0 ? t.timescale : 1;

	// Sizes.
	const sizeOf = (i: number): number =>
		t.sampleSizes.length > 0 ? t.sampleSizes[i] : t.sampleSizeConstant;

	// Per-sample byte offset, walking chunk by chunk.
	const spc = samplesPerChunk(t.stscFirstChunk, t.stscSamplesPerChunk, t.chunkOffsets.length);
	const offsets = new Array<number>(n);
	let sample = 0;
	for (let c = 1; c <= t.chunkOffsets.length && sample < n; c++) {
		let pos = t.chunkOffsets[c - 1];
		const count = spc[c];
		for (let s = 0; s < count && sample < n; s++) {
			offsets[sample] = pos;
			pos += sizeOf(sample);
			sample++;
		}
	}
	// If the chunk table under-covers the samples (malformed), bail.
	if (sample < n) return [];

	// Decode-time deltas → dts, and per-sample duration.
	const deltas = expandRuns(t.sttsCounts, t.sttsDeltas, n);
	// Composition offsets (B-frame reordering). Absent ⇒ cts == dts.
	const ctsOff =
		t.cttsCounts && t.cttsOffsets && t.cttsCounts.length > 0
			? expandRuns(t.cttsCounts, t.cttsOffsets, n)
			: null;

	const sync = t.stssSampleNumbers;
	const syncSet = sync && sync.length > 0 ? new Set(sync) : null;

	const out = new Array<SampleEntry>(n);
	let dts = 0;
	for (let i = 0; i < n; i++) {
		const cts = dts + (ctsOff ? ctsOff[i] : 0);
		out[i] = {
			ctsUs: Math.round((cts / ts) * 1e6),
			durUs: Math.round((deltas[i] / ts) * 1e6),
			// No stss table means every sample is a sync sample (all-keyframe).
			key: syncSet ? syncSet.has(i + 1) : true,
			offset: offsets[i],
			size: sizeOf(i),
		};
		dts += deltas[i];
	}
	return out;
}

/**
 * The byte range [startByte, endByte] (inclusive) covering the GOP that begins
 * at decode-index `kfIndex` and runs up to (but not including) the next
 * keyframe. Used to fetch exactly one GOP's media bytes in a single Range
 * request. Samples within a GOP are stored contiguously in decode order, but we
 * take min/max defensively so an unusual layout still yields a covering range.
 */
export function gopByteRange(
	samples: ReadonlyArray<Pick<SampleEntry, "offset" | "size">>,
	keyframes: ReadonlyArray<number>,
	kfIndex: number,
): { startByte: number; endByte: number } {
	// The decode-index where this GOP ends (next keyframe, or end of stream).
	let next = samples.length;
	for (let k = 0; k < keyframes.length; k++) {
		if (keyframes[k] === kfIndex) {
			next = k + 1 < keyframes.length ? keyframes[k + 1] : samples.length;
			break;
		}
	}
	let start = Infinity;
	let end = -Infinity;
	for (let i = kfIndex; i < next; i++) {
		const s = samples[i];
		if (s.offset < start) start = s.offset;
		if (s.offset + s.size > end) end = s.offset + s.size;
	}
	if (start === Infinity) return { startByte: 0, endByte: -1 }; // empty
	return { startByte: start, endByte: end - 1 };
}

/**
 * Size threshold (bytes) above which the source switches from whole-file load to
 * progressive HTTP-range ingestion; progressive trades whole-file's simplicity
 * for a flat memory footprint on multi-GB recordings that would blow the heap.
 */
export const PROGRESSIVE_THRESHOLD_BYTES = 500 * 1024 * 1024;

/** Pick the ingestion strategy for a file of `sizeBytes`. */
export function chooseIngestion(
	sizeBytes: number | undefined,
	threshold = PROGRESSIVE_THRESHOLD_BYTES,
): "whole" | "progressive" {
	// Unknown size ⇒ play it safe with the proven whole-file path.
	if (!sizeBytes || !Number.isFinite(sizeBytes) || sizeBytes <= 0) return "whole";
	return sizeBytes >= threshold ? "progressive" : "whole";
}
