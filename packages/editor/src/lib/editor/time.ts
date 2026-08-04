/**
 * The editor's ONE clock.
 *
 * Everything a user reads is on the OUTPUT (post-cut) axis, because output is
 * what gets exported. Original-recording time is an internal coordinate and must
 * never reach a label: the transport used to show output seconds while the
 * timeline playhead sitting under it showed original seconds, so after a single
 * cut the two disagreed with no way to tell which one the export would match.
 *
 * Every readout in the editor formats through `formatTimeByMode` so the Time
 * display setting governs all of them at once.
 */

import { clockCentis } from "../format/time";
import { originalToOutput, outputToOriginal, type TimeMap } from "../timeline/time-map";

/** How every timecode in the editor is rendered. Owned by the store. */
export type TimeMode = "smpte" | "seconds" | "frames";

/** SMPTE `HH:MM:SS:FF`, dropping the hours for clips under an hour. */
export function formatSmpte(seconds: number, fps: number): string {
	const t = Math.max(0, seconds);
	const rate = Math.round(fps) || 1;
	const totalFrames = Math.round(t * fps);
	const frames = totalFrames % rate;
	const totalSecs = Math.floor(totalFrames / rate);
	const ff = String(frames).padStart(2, "0");
	const ss = String(totalSecs % 60).padStart(2, "0");
	const mm = String(Math.floor(totalSecs / 60) % 60).padStart(2, "0");
	const hours = Math.floor(totalSecs / 3600);
	return hours > 0 ? `${String(hours).padStart(2, "0")}:${mm}:${ss}:${ff}` : `${mm}:${ss}:${ff}`;
}

/** `M:SS.cc`. The plain wall-clock reading. */
export function formatClock(seconds: number): string {
	return clockCentis(seconds);
}

/** Absolute frame index, e.g. `412f`. */
export function formatFrames(seconds: number, fps: number): string {
	return `${Math.max(0, Math.round(seconds * fps))}f`;
}

export function formatTimeByMode(seconds: number, mode: TimeMode, fps: number): string {
	switch (mode) {
		case "smpte":
			return formatSmpte(seconds, fps);
		case "seconds":
			return formatClock(seconds);
		case "frames":
			return formatFrames(seconds, fps);
	}
}

/**
 * Compact ruler tick label. Kept separate from `formatTimeByMode` because ruler
 * ticks sit ~50px apart and a full SMPTE stamp does not fit.
 *
 * `interval` is the spacing between ticks: below one second the label MUST carry
 * a decimal, or every tick floors to the same whole second and the ruler prints
 * `0:00, 0:00, 0:01, 0:01`.
 */
export function formatRulerTick(
	seconds: number,
	mode: TimeMode,
	fps: number,
	interval: number,
): string {
	if (mode === "frames") return formatFrames(seconds, fps);
	const mins = Math.floor(seconds / 60);
	const secs = seconds % 60;
	return interval < 1
		? `${mins}:${secs.toFixed(1).padStart(4, "0")}`
		: `${mins}:${String(Math.floor(secs)).padStart(2, "0")}`;
}

/**
 * Step one frame on the OUTPUT (post-cut) axis, returning the new ORIGINAL time.
 * Stepping across a cut boundary lands on the next kept frame instead of inside
 * a removed range. The caller drives the transport (video + audio) with the
 * returned original time.
 */
export function frameStepOutput(
	map: TimeMap,
	meta: { fps: number; duration: number },
	currentTime: number,
	dir: number,
): number {
	const frameDur = 1 / (meta.fps || 30);
	const outDur = originalToOutput(map, meta.duration);
	const nextOut = Math.max(
		0,
		Math.min(originalToOutput(map, currentTime) + frameDur * dir, outDur),
	);
	return outputToOriginal(map, nextOut);
}
