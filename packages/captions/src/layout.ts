/**
 * Caption placement relative to the VIDEO rect inside the output frame.
 *
 * The output frame (canvas) is the video plus padding plus any letterbox bars.
 * At Offset 0 a top/bottom caption sits at the video's edge (in the padding, not
 * covering the video); positive Offset moves it INWARD over the video and negative
 * tucks it OUTWARD into the padding. The baseline is clamped to the frame edge so a
 * full-bleed video keeps the whole Offset range live instead of dead-clamping.
 *
 * Both renderers derive from this: the CSS preview overlay and the Rust ASS
 * generator (which mirrors `captionHeightFrac` / `captionTopFrac`). All values
 * are fractions of the canvas (0..1). Keep the two in sync.
 */

export interface VideoRect {
	/** Video rect within the canvas, as fractions of canvas width/height. */
	left: number;
	right: number;
	top: number;
	bottom: number;
}

/** Largest caption block height we ever reserve, as a fraction of the frame.
 *  Keeps the clamp from pushing captions past the frame centre. */
const MAX_CAP_FRAC = 0.7;
/** Line-height + breathing room factor for the height estimate. */
const LINE_FACTOR = 1.35;

/** Estimated caption block height as a fraction of frame height. Uses `maxLines`
 *  as an upper bound so the clamp reserves enough room for the tallest case. */
export function captionHeightFrac(fontSizePct: number, maxLines: number): number {
	const lines = Math.max(1, maxLines);
	return Math.min(MAX_CAP_FRAC, (fontSizePct / 100) * lines * LINE_FACTOR);
}

/**
 * Fraction-from-top of the caption block's TOP edge (the block grows downward).
 * `null` means centre: vertically centred on the video (which is itself centred
 * in the canvas), handled by the caller. `capFrac` comes from
 * {@link captionHeightFrac}.
 */
export function captionTopFrac(
	position: "top" | "center" | "bottom",
	offsetPct: number,
	capFrac: number,
	video: Pick<VideoRect, "top" | "bottom">,
): number | null {
	if (position === "center") return null;
	// Signed against the on-frame edge, so every slider value is live; anchoring on the raw video edge dead-clamped the positive range at full bleed.
	const offset = offsetPct / 100;
	const cap = Math.max(0, Math.min(MAX_CAP_FRAC, capFrac));
	const maxTop = Math.max(0, 1 - cap);
	if (position === "bottom") {
		// Baseline just below the video (clamped on-frame); positive lifts up.
		const base = Math.min(video.bottom, maxTop);
		return Math.max(0, Math.min(base - offset, maxTop));
	}
	// Top: baseline just above the video (clamped on-frame); positive pushes down.
	const base = Math.max(0, video.top - cap);
	return Math.max(0, Math.min(base + offset, maxTop));
}
