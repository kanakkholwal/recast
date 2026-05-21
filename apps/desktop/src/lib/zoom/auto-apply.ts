/**
 * Smart Auto-Zoom — shared placement helpers used by both the manual
 * "Auto-focus" popover and the on-load auto-apply hook.
 *
 * Detection (`suggestZoomRegions`) lives in Rust. This module owns:
 *   - placement geometry (slotting a 1 s window around each trigger without
 *     overlapping existing focus regions or the clip's trim bounds)
 *   - resolving the *focus point* (UV centre) from cursor data so each zoom
 *     lands on what the user was actually pointing at
 */

import type { CursorSampleLike } from "$lib/cursor/smoothing";
import type { ZoomSuggestion } from "$lib/ipc";
import type { EditorStore } from "$lib/stores/editor-store.svelte";

// A zoom reads well when it is *already* at full scale by the time the
// action happens and then holds long enough for the viewer to register it.
// So the window around a trigger is asymmetric: a short lead-in before the
// trigger, a long hold after. With the default 0.35 s ramps that leaves a
// comfortable ~2 s plateau at full zoom — a 1 s symmetric window only held
// ~0.3 s and read as a nervous flicker.
export const ZOOM_LEAD_IN = 0.6; // seconds of window before the trigger
export const ZOOM_HOLD = 2.4; // seconds of window after the trigger
// Shrinks if a neighbour forces it, but never below this — anything shorter
// barely clears the ramps and reads as a flicker.
export const MIN_REGION_DURATION = 1.2;
export const MIN_GAP = 0.08; // guardband so adjacent regions don't visually touch
// Default scale for auto-placed zooms. 1.8× crops aggressively and feels
// claustrophobic; ~1.6× keeps surrounding context visible.
export const AUTO_ZOOM_SCALE = 1.6;

export interface Interval {
	start: number;
	end: number;
}

/**
 * Given a sorted list of occupied intervals within [clipStart, clipEnd],
 * find the free slot that contains `t`. Returns null if `t` is inside an
 * occupied interval (with `MIN_GAP` padding).
 */
export function findFreeSlot(
	occupied: Interval[],
	clipStart: number,
	clipEnd: number,
	t: number,
): Interval | null {
	if (t < clipStart || t > clipEnd) return null;
	let a = clipStart;
	for (const iv of occupied) {
		if (t >= iv.start - MIN_GAP && t <= iv.end + MIN_GAP) return null;
		if (iv.end <= t) {
			a = iv.end + MIN_GAP;
		} else {
			return { start: a, end: iv.start - MIN_GAP };
		}
	}
	return { start: a, end: clipEnd };
}

/**
 * Compute the placement window for a suggestion given current occupied
 * intervals. The window is anchored asymmetrically around `centerSec` (the
 * trigger moment): `ZOOM_LEAD_IN` before, `ZOOM_HOLD` after. Returns null if
 * there's no room for a meaningful zoom.
 */
export function planPlacement(
	occupied: Interval[],
	clipStart: number,
	clipEnd: number,
	centerSec: number,
): Interval | null {
	const slot = findFreeSlot(occupied, clipStart, clipEnd, centerSec);
	if (!slot) return null;
	const start = Math.max(slot.start, centerSec - ZOOM_LEAD_IN);
	const end = Math.min(slot.end, centerSec + ZOOM_HOLD);
	if (end - start < MIN_REGION_DURATION) return null;
	return { start, end };
}

/**
 * Resolve a focus point in UV coordinates from the captured cursor track at
 * a given playback time. Falls back to the canvas centre when there's no
 * usable sample (e.g. cursor data isn't loaded yet, or screen-only capture).
 *
 * `samples` x/y are in source-video pixel space — the same as `metadata.width/
 * height` — so we just normalise. Binary-search nearest by timestamp.
 */
export function resolveZoomCenter(
	samples: CursorSampleLike[] | null | undefined,
	atTimeSec: number,
	canvasW: number,
	canvasH: number,
): { x: number; y: number } {
	if (!samples || samples.length === 0 || canvasW <= 0 || canvasH <= 0) {
		return { x: 0.5, y: 0.5 };
	}
	const targetUs = atTimeSec * 1_000_000;
	// Binary search for the nearest sample.
	let lo = 0;
	let hi = samples.length - 1;
	while (lo < hi) {
		const mid = (lo + hi) >>> 1;
		if (samples[mid].timestampUs < targetUs) lo = mid + 1;
		else hi = mid;
	}
	const cand = samples[lo];
	const prev = lo > 0 ? samples[lo - 1] : cand;
	const nearest =
		Math.abs(cand.timestampUs - targetUs) <= Math.abs(prev.timestampUs - targetUs)
			? cand
			: prev;
	const x = Math.min(1, Math.max(0, nearest.x / canvasW));
	const y = Math.min(1, Math.max(0, nearest.y / canvasH));
	return { x, y };
}

/** UV centre derived directly from a suggestion's pixel coordinates. */
function suggestionCenter(
	sug: ZoomSuggestion,
	canvasW: number,
	canvasH: number,
): { x: number; y: number } {
	if (canvasW <= 0 || canvasH <= 0) return { x: 0.5, y: 0.5 };
	return {
		x: Math.min(1, Math.max(0, sug.x / canvasW)),
		y: Math.min(1, Math.max(0, sug.y / canvasH)),
	};
}

export interface AutoZoomResult {
	applied: number;
	skipped: number;
}

/**
 * Place each suggestion as an auto-sourced zoom region in `store`. Earlier
 * timestamps win when two triggers compete for the same slot. The caller is
 * responsible for pushing a single coalesced undo entry so all auto-zooms
 * collapse into one Cmd-Z.
 */
export function applyAutoZooms(
	store: EditorStore,
	suggestions: ZoomSuggestion[],
	clipBounds: Interval,
	canvasW: number,
	canvasH: number,
	scale = AUTO_ZOOM_SCALE,
): AutoZoomResult {
	const occupied: Interval[] = store.zoomRegions
		.map((z) => ({ start: z.start, end: z.end }))
		.sort((a, b) => a.start - b.start);
	const sorted = [...suggestions].sort((a, b) => a.timestampUs - b.timestampUs);
	let applied = 0;
	let skipped = 0;
	for (const sug of sorted) {
		const centerSec = sug.timestampUs / 1_000_000;
		const plan = planPlacement(occupied, clipBounds.start, clipBounds.end, centerSec);
		if (!plan) {
			skipped++;
			continue;
		}
		const c = suggestionCenter(sug, canvasW, canvasH);
		store.addAutoZoomRegion(plan.start, plan.end, scale, c.x, c.y);
		occupied.push(plan);
		occupied.sort((a, b) => a.start - b.start);
		applied++;
	}
	return { applied, skipped };
}
