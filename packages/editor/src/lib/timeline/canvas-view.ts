// Pure view geometry (Stage A): timeline drawn against OUTPUT frames; view = (scrollFrames, px/frame); per-scene, never in the undo journal or project file.

export interface TimelineView {
	/** Left edge of the viewport in OUTPUT frames. */
	scrollFrames: number;
	/** Pixels per output frame. */
	resolution: number;
}

// Reference constants from .notes/frontend-timeline-plan.md §3.
export const RESOLUTION_MIN = 0.03;
export const RESOLUTION_MAX = 120;
export const DEFAULT_RESOLUTION = 1 / 0.7;

export function clampResolution(resolution: number): number {
	if (!Number.isFinite(resolution)) return DEFAULT_RESOLUTION;
	return Math.min(Math.max(resolution, RESOLUTION_MIN), RESOLUTION_MAX);
}

/** Pixel x of a frame in the viewport. */
export function frameToX(frame: number, view: TimelineView): number {
	return (frame - view.scrollFrames) * view.resolution;
}

/** Frame at a viewport pixel x. */
export function xToFrame(x: number, view: TimelineView): number {
	return view.scrollFrames + x / view.resolution;
}

/** The furthest the view can scroll so content still fills (or under-fills) it. */
export function maxScrollFrames(
	totalFrames: number,
	viewportPx: number,
	resolution: number,
): number {
	const viewportFrames = resolution > 0 ? viewportPx / resolution : 0;
	return Math.max(0, totalFrames - viewportFrames);
}

/** Keep the view within [0, maxScroll] so it never drifts off the content. */
export function clampScroll(
	view: TimelineView,
	totalFrames: number,
	viewportPx: number,
): TimelineView {
	const max = maxScrollFrames(totalFrames, viewportPx, view.resolution);
	const scrollFrames = Math.min(Math.max(view.scrollFrames, 0), max);
	return scrollFrames === view.scrollFrames ? view : { ...view, scrollFrames };
}

/**
 * Zoom by `factor` while pinning the frame under `cursorX` to that same pixel,
 * so zooming does not move the view out from under the pointer. Resolution is
 * clamped; scroll is re-derived from the pinned frame.
 */
export function zoomAt(view: TimelineView, cursorX: number, factor: number): TimelineView {
	const resolution = clampResolution(view.resolution * factor);
	if (resolution === view.resolution) return view;
	const frameAtCursor = xToFrame(cursorX, view);
	const scrollFrames = frameAtCursor - cursorX / resolution;
	return { scrollFrames, resolution };
}

/** Scroll horizontally by a pixel delta, expressed in frames. */
export function scrollByPixels(view: TimelineView, deltaPx: number): TimelineView {
	if (view.resolution <= 0) return view;
	return { ...view, scrollFrames: view.scrollFrames + deltaPx / view.resolution };
}
