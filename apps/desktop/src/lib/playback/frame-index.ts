/**
 * Pure, decoder-free frame-index logic for the WebCodecs video source.
 *
 * The index is the demuxed sample table in two orders:
 *   - `chunks`     : encoded samples in DECODE order (the order mp4box emits).
 *   - `presOrder`  : indices into `chunks` sorted by presentation time (cts),
 *                    to answer "which frame is on screen at time T".
 * Keyframes are decode-order indices of sync samples. All times are microseconds.
 */

/** One encoded frame, in DECODE order, with its presentation time. */
export interface ChunkMeta {
	/** Presentation timestamp (µs): the cache key and frameAt lookup key. */
	ctsUs: number;
	/** Duration (µs). */
	durUs: number;
	/** IDR / sync sample: a valid decode entry point. */
	key: boolean;
	/**
	 * Encoded bytes (avcC length-prefixed NAL units), or `null` when not yet
	 * fetched. Whole-file ingestion always has the bytes; the progressive path
	 * leaves them `null` until the GOP is range-fetched (using the parallel
	 * sample byte-map), then fills them in and nulls them again on Tier-1
	 * eviction.
	 */
	data: Uint8Array | null;
}

/** Decode-order indices of keyframes, ascending. */
export function buildKeyframes(chunks: ReadonlyArray<ChunkMeta>): number[] {
	const out: number[] = [];
	for (let i = 0; i < chunks.length; i++) if (chunks[i].key) out.push(i);
	return out;
}

/** Indices into `chunks` sorted by presentation time (cts ascending). */
export function buildPresOrder(chunks: ReadonlyArray<ChunkMeta>): number[] {
	return chunks
		.map((_, i) => i)
		.sort((a, b) => chunks[a].ctsUs - chunks[b].ctsUs);
}

/**
 * Decode-index of the last sample whose presentation time is ≤ `tUs`. Returns
 * the earliest sample when `tUs` precedes everything (screen is never blank).
 * Assumes `presOrder` is non-empty.
 */
export function sampleAtOrBefore(
	chunks: ReadonlyArray<ChunkMeta>,
	presOrder: ReadonlyArray<number>,
	tUs: number,
): number {
	let lo = 0;
	let hi = presOrder.length - 1;
	let ans = presOrder[0];
	while (lo <= hi) {
		const mid = (lo + hi) >> 1;
		if (chunks[presOrder[mid]].ctsUs <= tUs) {
			ans = presOrder[mid];
			lo = mid + 1;
		} else {
			hi = mid - 1;
		}
	}
	return ans;
}

/** Largest keyframe decode-index ≤ `decodeIndex`. Assumes a keyframe at 0. */
export function keyframeAtOrBefore(
	keyframes: ReadonlyArray<number>,
	decodeIndex: number,
): number {
	let lo = 0;
	let hi = keyframes.length - 1;
	let ans = keyframes[0] ?? 0;
	while (lo <= hi) {
		const mid = (lo + hi) >> 1;
		if (keyframes[mid] <= decodeIndex) {
			ans = keyframes[mid];
			lo = mid + 1;
		} else {
			hi = mid - 1;
		}
	}
	return ans;
}

/**
 * Whether the decoder must be reset+reconfigured before feeding the GOP at
 * keyframe `kf`, given the keyframe it's primed from (`anchorKey`, -1 if never
 * primed) and the next decode-index it would feed (`feedCursor`).
 *
 * Resets on: never primed; backward jump to an earlier keyframe; or a forward
 * GOP gap, but only when that gap is a genuine discontinuity (`forwardIsJump`).
 *
 * The `forwardIsJump` guard is essential: the playback clock free-runs at
 * realtime, so if decode falls behind the playhead outruns the feed cursor and
 * crosses keyframes the decoder hasn't reached. Resetting there would seek
 * forward and SKIP the frames in between, freezing the picture on edit-heavy
 * content. The caller flags a jump via a large time delta. Defaults to true so
 * existing callers keep the old "any forward gap resets" behaviour.
 */
export function needsReset(
	anchorKey: number,
	feedCursor: number,
	kf: number,
	forwardIsJump = true,
): boolean {
	return anchorKey === -1 || kf < anchorKey || (kf > feedCursor && forwardIsJump);
}
