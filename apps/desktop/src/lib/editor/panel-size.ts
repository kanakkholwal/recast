/**
 * Bounds for the resizable timeline panel.
 *
 * The panel is docked bottom and shares the editor column with the preview, so
 * it is bounded at BOTH ends. The floor keeps the ruler, the clip bar and one
 * lane on screen, since a panel shorter than that is a scrollbar with nothing
 * useful in it. The ceiling is a SHARE of the column rather than a fixed number:
 * a height that leaves a sensible preview on a 27" display leaves none at all on
 * a laptop, and the preview is the thing being edited.
 */

/** Ruler + clip bar + one lane + the toolbar, near enough. */
export const TIMELINE_MIN_HEIGHT_PX = 180;
/** Absolute ceiling. Past this the extra height is just empty lane space. */
export const TIMELINE_MAX_HEIGHT_PX = 560;
/**
 * Ceiling as a fraction of the editor column. Under half deliberately: the
 * preview is the thing being edited, so it keeps the larger share at every
 * window size. (0.55 was the first guess and it let the timeline take the
 * majority, which is the complaint this whole panel exists to fix.)
 */
export const TIMELINE_MAX_SHARE = 0.45;
export const TIMELINE_DEFAULT_HEIGHT_PX = 260;

/**
 * Tallest the panel may be inside a column of `columnHeightPx`.
 *
 * A non-positive column means "not measured yet" (first paint, or a hidden
 * editor), and falls back to the absolute ceiling. Returning the share of zero
 * would collapse the panel to its floor for a frame and make the timeline jump
 * as soon as layout settled.
 */
export function timelineMaxHeight(columnHeightPx: number): number {
	if (!(columnHeightPx > 0)) return TIMELINE_MAX_HEIGHT_PX;
	const share = Math.round(columnHeightPx * TIMELINE_MAX_SHARE);
	// The floor wins on a very short window: better to overflow the column than
	// to render a panel with no usable rows in it.
	return Math.max(TIMELINE_MIN_HEIGHT_PX, Math.min(TIMELINE_MAX_HEIGHT_PX, share));
}

/** `height` brought inside the bounds for the current column. */
export function clampTimelineHeight(height: number, columnHeightPx: number): number {
	const max = timelineMaxHeight(columnHeightPx);
	if (!Number.isFinite(height)) return TIMELINE_DEFAULT_HEIGHT_PX;
	return Math.min(max, Math.max(TIMELINE_MIN_HEIGHT_PX, Math.round(height)));
}
