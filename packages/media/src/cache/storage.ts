/**
 * Value types for the decoded-frame cache, keyed by original-recording
 * timestamp (µs). Memory only — the IndexedDB backend behind this was removed
 * once the preview switched to hand-off-and-close frame ownership, which never
 * produced a structured-cloneable value to persist.
 */

/** Cached frame. `ImageBitmap` for uploaded stills, `VideoFrame` from the decoder. */
export type CachedFrame = ImageBitmap | VideoFrame;

/** Nominal RGBA bytes for a frame whose real dimensions are unreadable. Large
 *  on purpose (4K) so an un-sizeable frame is evicted EARLY. Returning 0 here
 *  let the cap check pass forever, disabling eviction and growing the Map (and
 *  its retained decoder surfaces) without bound. */
const UNKNOWN_FRAME_BYTES = 3840 * 2160 * 4;

/** Per-entry byte estimate for budget accounting. */
export function estimateFrameBytes(frame: CachedFrame): number {
	// `VideoFrame` has codedWidth/codedHeight and no width/height; reading the
	// wrong pair yields NaN and silently disables every cap.
	const w = "codedWidth" in frame ? frame.codedWidth : frame.width;
	const h = "codedHeight" in frame ? frame.codedHeight : frame.height;
	if (!Number.isFinite(w) || !Number.isFinite(h)) return UNKNOWN_FRAME_BYTES;
	// No public byteLength; nominal RGBA size, over-estimating on purpose.
	return w * h * 4;
}
