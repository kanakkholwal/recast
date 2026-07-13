/**
 * Caption placement relative to the VIDEO rect inside the output frame.
 *
 * The output frame (canvas) is the video plus padding plus any letterbox bars.
 * Anchoring captions to the frame edge means a top/bottom caption lands ON the
 * video once padding is added. Instead we anchor to the video's edge and push
 * the caption OUTWARD into the padding, so it never covers the video, with a
 * clamp so a no-padding video still shows captions (they fall back to the frame
 * edge, over the video, as before).
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
	// Signed: positive pushes the caption outward into the padding, negative
	// pulls it back toward (onto) the video. The result is clamped to the frame.
	const offset = offsetPct / 100;
	const cap = Math.max(0, Math.min(MAX_CAP_FRAC, capFrac));
	const maxTop = Math.max(0, 1 - cap);
	if (position === "bottom") {
		// Top edge sits just below the video's bottom edge (in the padding),
		// clamped so the block never runs off the frame.
		return Math.min(video.bottom + offset, maxTop);
	}
	// Top: the block's BOTTOM rests just above the video's top edge, so its top
	// is that minus the block height.
	return Math.max(0, Math.min(video.top - offset - cap, maxTop));
}
