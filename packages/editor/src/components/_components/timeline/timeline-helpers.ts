// Pure helpers extracted from Timeline.svelte so subviews share them and they stay unit-testable.
//
// Timecode formatting is NOT defined here. It lives in `$lib/editor/time`, which
// the transport also reads, so the timeline and the player can't drift onto
// different clocks (they did: one showed output time, the other original time).
// Re-exported so the timeline subviews keep a single local import.

import { formatRulerTick, type TimeMode } from "../../../lib/editor/time";

export {
	formatClock,
	formatFrames,
	formatRulerTick,
	formatSmpte,
	formatTimeByMode,
	type TimeMode,
} from "../../../lib/editor/time";

export function effectiveFps(metadataFps: number | undefined): number {
	const f = metadataFps ?? 0;
	return f > 0 ? f : 60;
}

export function quantizeToFrame(time: number, fps: number): number {
	return Math.round(time * fps) / fps;
}

export function frameStep(fps: number): number {
	return 1 / fps;
}

// At least 2 frames so a trimmed range is never sub-frame.
export function minClipDuration(fps: number): number {
	return 2 * frameStep(fps);
}

export function greatestCommonDivisor(a: number, b: number): number {
	let left = Math.abs(a);
	let right = Math.abs(b);
	while (right !== 0) {
		const next = left % right;
		left = right;
		right = next;
	}
	return left || 1;
}

export interface TimeMarker {
	time: number;
	label: string;
	emphasis: boolean;
}

// ---- Zoom ------------------------------------------------------------------
//
// Zoom is expressed as a multiple of "the whole clip fits the viewport", so the
// ceiling has to be derived from the clip's length, not fixed. It used to be a
// flat 5x, which made maximum magnification a function of how long you recorded:
// a 30-minute screencast bottomed out at 2.5 px/sec, i.e. 0.04px per frame, and
// could not be trimmed precisely at all.

/** Ceiling in pixels per second: ~6px per frame at 60fps, enough to aim at one. */
export const MAX_PIXELS_PER_SECOND = 400;

/** Zoom that fits the whole clip. Below this the viewport just grows dead space. */
export const MIN_TIMELINE_ZOOM = 1;

export function maxTimelineZoom(outputDuration: number, viewportWidth: number): number {
	if (outputDuration <= 0 || viewportWidth <= 0) return MIN_TIMELINE_ZOOM;
	const zoomAtCeiling = (MAX_PIXELS_PER_SECOND * outputDuration) / viewportWidth;
	return Math.max(MIN_TIMELINE_ZOOM, zoomAtCeiling);
}

export function clampTimelineZoom(
	zoom: number,
	outputDuration: number,
	viewportWidth: number,
): number {
	const max = maxTimelineZoom(outputDuration, viewportWidth);
	return Math.min(Math.max(zoom, MIN_TIMELINE_ZOOM), max);
}

/**
 * One zoom step. Multiplicative, not additive: the old +/-0.25 steps would need
 * thousands of clicks to cross the range a long recording now spans.
 */
export const ZOOM_STEP_FACTOR = 1.5;

export function steppedZoom(
	zoom: number,
	direction: number,
	outputDuration: number,
	viewportWidth: number,
): number {
	const next = direction > 0 ? zoom * ZOOM_STEP_FACTOR : zoom / ZOOM_STEP_FACTOR;
	return clampTimelineZoom(next, outputDuration, viewportWidth);
}

/** Spacing between ruler labels, chosen to keep them roughly 50px apart. */
export function rulerInterval(pixelsPerSecond: number): number {
	if (pixelsPerSecond < 26) return 10;
	if (pixelsPerSecond < 52) return 5;
	if (pixelsPerSecond < 120) return 2;
	if (pixelsPerSecond > 260) return 0.5;
	return 1;
}

// Major ruler labels. Formatted through the shared clock so the ruler agrees
// with the playhead standing on it, including in Frames mode.
export function buildTimeMarkers(
	duration: number,
	pixelsPerSecond: number,
	mode: TimeMode,
	fps: number,
	window?: TickWindow,
): TimeMarker[] {
	if (duration <= 0) return [];
	const markers: TimeMarker[] = [];
	const interval = rulerInterval(pixelsPerSecond);

	for (const t of tickTimes(duration, interval, window)) {
		markers.push({
			time: t,
			label: formatRulerTick(t, mode, fps, interval),
			emphasis: Math.round(t) % (interval >= 2 ? interval * 2 : 2) === 0,
		});
	}
	return markers;
}

// Filled SVG envelope path for an audio waveform, built in output-pixel space
// (each bucket at `xOf(bucketTime)`) so buckets inside a removed cut collapse
// onto the seam. `range` clips to a kept window (in/out); null keeps everything.
// `amp` is the peak half-height; `mid` is height/2.
export function buildWaveformPath(p: {
	waveform: ReadonlyArray<number>;
	duration: number;
	xOf: (t: number) => number;
	height: number;
	amp: number;
	range?: { start: number; end: number } | null;
}): string {
	const w = p.waveform;
	const n = w.length;
	if (n < 2 || p.duration <= 0) return "";
	const mid = p.height / 2;
	const kept: number[] = [];
	for (let i = 0; i < n; i++) {
		if (p.range) {
			const t = (i / n) * p.duration;
			if (t < p.range.start - 0.001 || t > p.range.end + 0.001) continue;
		}
		kept.push(i);
	}
	if (kept.length < 2) return "";
	const xAt = (i: number) => p.xOf((i / n) * p.duration).toFixed(2);
	let d = `M ${xAt(kept[0])} ${mid}`;
	for (const i of kept) d += ` L ${xAt(i)} ${(mid - w[i] * p.amp).toFixed(2)}`;
	for (let k = kept.length - 1; k >= 0; k--) {
		const i = kept[k];
		d += ` L ${xAt(i)} ${(mid + w[i] * p.amp).toFixed(2)}`;
	}
	return `${d} Z`;
}

// Minor tick marks between labels.
export function buildMinorTicks(
	duration: number,
	pixelsPerSecond: number,
	window?: TickWindow,
): number[] {
	if (duration <= 0) return [];
	const interval = pixelsPerSecond > 180 ? 0.25 : pixelsPerSecond > 80 ? 0.5 : 1;
	return tickTimes(duration, interval, window);
}

/** Visible slice of the timeline, in OUTPUT seconds. Omit for the whole thing. */
export interface TickWindow {
	startSec: number;
	endSec: number;
}

/**
 * Tick times on the `interval` grid, clipped to `window`. Generated from the
 * grid INDEX rather than by accumulating `t += interval`, so a tick's value —
 * and therefore its `{#each}` key — is identical no matter where the window
 * starts. Accumulating would shift every value as the user scrolls and
 * re-create the whole row each frame.
 */
function tickTimes(duration: number, interval: number, window?: TickWindow): number[] {
	const lastIdx = Math.floor((duration + interval * 0.5) / interval);
	const firstIdx = window ? Math.max(0, Math.floor(window.startSec / interval)) : 0;
	const endIdx = window ? Math.min(lastIdx, Math.ceil(window.endSec / interval)) : lastIdx;
	const times: number[] = [];
	for (let i = firstIdx; i <= endIdx; i++) times.push(i * interval);
	return times;
}
