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

// Target window around each trigger. Shrinks if a neighbour forces it, but
// never shorter than MIN_REGION_DURATION — a 200 ms zoom reads as a flicker.
export const IDEAL_HALF_WIDTH = 0.5; // seconds — full 1 s window when unobstructed
export const MIN_REGION_DURATION = 0.4;
export const MIN_GAP = 0.05; // guardband so adjacent regions don't visually touch

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
 * intervals. Returns null if there's no room for a meaningful zoom.
 */
export function planPlacement(
	occupied: Interval[],
	clipStart: number,
	clipEnd: number,
	centerSec: number,
): Interval | null {
	const slot = findFreeSlot(occupied, clipStart, clipEnd, centerSec);
	if (!slot) return null;
	const start = Math.max(slot.start, centerSec - IDEAL_HALF_WIDTH);
	const end = Math.min(slot.end, centerSec + IDEAL_HALF_WIDTH);
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
	scale = 1.8,
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
