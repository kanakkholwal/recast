/**
 * Default geometry for a shape placed by a CLICK rather than a drag, so picking
 * a tool and clicking once produces a usable shape instead of nothing.
 *
 * Sizes are derived in visual pixels (UV x frame dimensions) and mapped back,
 * the same convention `resize-constraints.ts` uses, so a default box reads the
 * same on a 16:9 capture and a portrait one.
 */

/** Width of a click-placed shape, as a fraction of frame width. */
const CLICK_WIDTH = 0.24;
/** Screen aspect a click-placed box aims for; 3:2 reads as a deliberate box. */
const CLICK_SCREEN_RATIO = 1.5;
/** Diagonal length of a click-placed arrow, as a fraction of frame width. */
const CLICK_ARROW_LENGTH = 0.3;

function clamp01(v: number): number {
	return Math.min(1, Math.max(0, v));
}

/** Box centred on the click, nudged to stay inside the frame. */
export function clickPlacedBox(
	ux: number,
	uy: number,
	frameW: number,
	frameH: number,
): { x: number; y: number; w: number; h: number } {
	const w = Math.min(CLICK_WIDTH, 1);
	const screenW = w * frameW;
	const h = Math.min(screenW / CLICK_SCREEN_RATIO / Math.max(frameH, 1), 1);
	return {
		x: Math.min(Math.max(clamp01(ux) - w / 2, 0), 1 - w),
		y: Math.min(Math.max(clamp01(uy) - h / 2, 0), 1 - h),
		w,
		h,
	};
}

/**
 * Arrow starting at the click and running down-right at a visual 45 degrees,
 * flipped on either axis when there is no room, so the head never lands
 * off-frame in a corner.
 */
export function clickPlacedArrow(
	ux: number,
	uy: number,
	frameW: number,
	frameH: number,
): { x1: number; y1: number; x2: number; y2: number } {
	const leg = (CLICK_ARROW_LENGTH * frameW) / Math.SQRT2;
	const dx = leg / Math.max(frameW, 1);
	const dy = leg / Math.max(frameH, 1);
	const x1 = clamp01(ux);
	const y1 = clamp01(uy);
	return {
		x1,
		y1,
		x2: clamp01(x1 + dx > 1 ? x1 - dx : x1 + dx),
		y2: clamp01(y1 + dy > 1 ? y1 - dy : y1 + dy),
	};
}

/** How long a newly placed annotation lasts, in seconds. */
const PLACED_SECONDS = 2;

/**
 * Time range for an annotation placed at the playhead. The fade-in runs BEFORE
 * `now`, so the annotation is at full opacity exactly where it was placed; ending
 * the fade at the playhead instead would leave the thing you just added invisible
 * until you seeked past it.
 */
export function placedTimeRange(
	now: number,
	trimStart: number,
	clipEnd: number,
	rampIn: number,
): { start: number; end: number } {
	const at = Math.min(Math.max(now, trimStart), clipEnd);
	const wanted = Math.max(trimStart, at - Math.max(0, rampIn));
	// Slid back off the tail rather than truncated, or placing at the very end gives an annotation made entirely of fade.
	const start = Math.max(trimStart, Math.min(wanted, clipEnd - PLACED_SECONDS));
	return { start, end: Math.min(clipEnd, start + PLACED_SECONDS) };
}
