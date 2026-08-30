// Pure adaptive tick density for the canvas timeline ruler (Stage A). Ticks are
// worked out in FRAMES — a frame is what the timeline is made of — so they never
// jitter by a rounding error at fractional zoom.

import { frameToX, type TimelineView, xToFrame } from "./canvas-view";

export const TARGET_MAJOR_TICK_DISTANCE = 160;
const MIN_MINOR_PX = 8;
const MAX_TICKS = 10000;

const SECONDS_LADDER = [1, 2, 5, 10, 15, 20, 30, 60, 120, 300, 600, 900, 1800, 3600, 7200];

/** Candidate tick intervals in frames: sub-second frame steps plus the second
 *  ladder scaled by fps. Sorted ascending, deduped. */
export function niceIntervals(fps: number): number[] {
	const f = fps > 0 ? fps : 60;
	const sub = [1, 2, 5, 10, Math.round(f / 4), Math.round(f / 2)];
	const secs = SECONDS_LADDER.map((s) => Math.round(s * f));
	return Array.from(new Set([...sub, ...secs].filter((n) => n >= 1))).sort((a, b) => a - b);
}

export interface TickInterval {
	/** Labelled tick spacing, frames. */
	major: number;
	/** Unlabelled subdivision spacing, frames. */
	minor: number;
}

export function chooseTickInterval(
	view: TimelineView,
	fps: number,
	targetPx = TARGET_MAJOR_TICK_DISTANCE,
): TickInterval {
	const targetFrames = view.resolution > 0 ? targetPx / view.resolution : Number.POSITIVE_INFINITY;
	const ladder = niceIntervals(fps);
	const major = ladder.find((n) => n >= targetFrames) ?? ladder[ladder.length - 1];
	// The finest subdivision that still keeps minor ticks readable apart.
	let minor = major;
	for (const k of [10, 5, 4, 2]) {
		if (major % k === 0 && (major / k) * view.resolution >= MIN_MINOR_PX) {
			minor = major / k;
			break;
		}
	}
	return { major, minor };
}

export interface Tick {
	frame: number;
	x: number;
	major: boolean;
}

/** Ticks intersecting the viewport, clamped to `[0, totalFrames]`. */
export function visibleTicks(
	view: TimelineView,
	viewportPx: number,
	fps: number,
	totalFrames: number,
): { ticks: Tick[]; interval: TickInterval } {
	const interval = chooseTickInterval(view, fps);
	const { major, minor } = interval;
	const first = Math.max(0, Math.floor(xToFrame(0, view) / minor) * minor);
	const last = Math.min(totalFrames, xToFrame(viewportPx, view));
	const ticks: Tick[] = [];
	for (let f = first; f <= last + 1e-6 && ticks.length < MAX_TICKS; f += minor) {
		ticks.push({ frame: f, x: frameToX(f, view), major: f % major === 0 });
	}
	return { ticks, interval };
}

/** Time label for a labelled tick. `M:SS` at second-scale, seconds with decimals
 *  when major ticks are sub-second. */
export function formatTickLabel(frame: number, fps: number, majorFrames: number): string {
	const f = fps > 0 ? fps : 60;
	const secs = frame / f;
	if (majorFrames >= f) {
		const m = Math.floor(secs / 60);
		const s = Math.floor(secs % 60);
		return `${m}:${s.toString().padStart(2, "0")}`;
	}
	return `${secs.toFixed(2)}s`;
}
