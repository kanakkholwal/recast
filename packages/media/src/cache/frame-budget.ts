/**
 * Resolution-adaptive decoded-frame budget for the preview decoder.
 *
 * Each decoded `VideoFrame` checks out one of the hardware decoder's limited
 * output surfaces; holding too many starves the pool and the decoder stalls
 * (accepts input, emits nothing → ~8fps). A fixed count is wrong across
 * resolutions: 7 frames is fine at 1080p but holds 4–7× the surface memory at
 * 4K/5K (macOS records full native Retina), re-triggering the stall. So we scale
 * all three holders (`cacheMax`, `holdoutMax`, `decodeAhead`) down together with
 * pixel count against a fixed surface-memory budget. At ≤1440p this returns the
 * historical, known-good 7 / 4 / 6; at 4K/5K it tightens.
 */

/**
 * Bytes assumed per decoded frame per pixel. Raw yuv420p is 1.5 B/px, but HW
 * decode surfaces are padded/tiled and often NV12/RGBA-backed, so budget a
 * conservative 4 to stay under the real output pool.
 */
const BYTES_PER_PX = 4;

/** Total decoded-surface memory we're willing to hold at once (~192 MB). */
const SURFACE_BUDGET_BYTES = 192 * 1024 * 1024;

/** Clamp range for the combined frame count (primary cache + scout holdout). */
const MIN_TOTAL_FRAMES = 6;
const MAX_TOTAL_FRAMES = 11;

export interface FrameBudget {
	/** Cap for the primary decoded-frame cache. */
	cacheMax: number;
	/** Cap for the scout (cross-cut prefetch) holdout. */
	holdoutMax: number;
	/** How many samples the worker decodes ahead of the playhead. */
	decodeAhead: number;
}

/**
 * Compute the decoded-frame budget for a `width`×`height` source. Falls back to
 * the generous (low-res) budget when dimensions are unknown/invalid.
 */
export function frameBudget(width: number, height: number): FrameBudget {
	const pixels = width > 0 && height > 0 ? width * height : 0;
	const perFrame = pixels > 0 ? pixels * BYTES_PER_PX : 0;

	const total =
		perFrame > 0
			? Math.min(
					MAX_TOTAL_FRAMES,
					Math.max(MIN_TOTAL_FRAMES, Math.floor(SURFACE_BUDGET_BYTES / perFrame)),
				)
			: MAX_TOTAL_FRAMES;

	// Reserve up to 4 for the scout holdout, shrinking it first as the budget tightens; the primary cache matters more.
	const holdoutMax = Math.min(4, Math.max(2, total - 5));
	const cacheMax = Math.max(4, total - holdoutMax);
	// Don't decode further ahead than the cache holds, or those frames are evicted on arrival.
	const decodeAhead = Math.max(3, Math.min(6, cacheMax - 1));

	return { cacheMax, holdoutMax, decodeAhead };
}

/** GPU memory we'll spend on our OWN frame textures (~256 MB). */
const TEXTURE_BUDGET_BYTES = 256 * 1024 * 1024;
const MIN_RING_FRAMES = 4;
const MAX_RING_FRAMES = 16;

/**
 * How many frames to keep as GPU textures we own.
 *
 * Distinct from {@link frameBudget}, and the distinction is the whole point: a
 * decoded `VideoFrame` occupies one of the decoder's few output surfaces, so
 * holding them starves the decoder and it stops emitting. A texture is ordinary
 * GPU memory. Upload, close the frame, and buffer depth becomes a memory
 * decision instead of a decoder-pool one.
 */
export function textureRingFrames(width: number, height: number): number {
	const pixels = width > 0 && height > 0 ? width * height : 0;
	if (pixels === 0) return MAX_RING_FRAMES;
	const perFrame = pixels * BYTES_PER_PX;
	const fits = Math.floor(TEXTURE_BUDGET_BYTES / perFrame);
	return Math.min(MAX_RING_FRAMES, Math.max(MIN_RING_FRAMES, fits));
}

/**
 * Byte cap for the in-memory frame cache at this resolution. Scaling by pixel
 * count is the point: a flat cap that is safe at 1080p holds several times the
 * decoder's surface pool at 4K and stalls it.
 */
export function frameCacheCapBytes(width: number, height: number): number {
	const px = Math.max(1, width * height);
	return frameBudget(width, height).cacheMax * px * BYTES_PER_PX;
}
