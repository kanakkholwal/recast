/**
 * Pure recording-settings maths: which fps tiers a display can present, the
 * clamp back to what it actually supports, resolving the gating refresh rate
 * from probed displays, and the 60→null persistence sentinel.
 */

import type { DisplayInfo, LastSource } from "$lib/recorder-types";

/**
 * 60 is always offered; 120/144/240 appear only when a display can present them
 * (the -2 tolerance covers 119.88/143.86-style reported rates).
 */
export function computeFpsOptions(maxRefreshHz: number): number[] {
	return [60, 120, 144, 240].filter((rate) => rate === 60 || maxRefreshHz >= rate - 2);
}

/** Desired rate capped to the highest option this display supports. */
export function clampFps(desiredFps: number, options: number[]): number {
	return Math.min(desiredFps, options[options.length - 1] ?? 60);
}

/**
 * Gating refresh: the selected monitor's rate when a monitor is the active
 * source, else the highest attached display (windows/regions don't pin one).
 * Falls back to 60 when nothing usable is reported.
 */
export function resolveMaxRefresh(displays: DisplayInfo[], last: LastSource | null): number {
	const globalMax = displays.reduce((m, d) => Math.max(m, d.refreshHz || 0), 0);
	let selected = 0;
	if (last?.kind === "monitor") {
		selected = displays.find((d) => d.id === last.id)?.refreshHz ?? 0;
	}
	const resolved = selected || globalMax;
	return resolved >= 1 ? resolved : 60;
}

/**
 * 60 persists as null (the unset/default sentinel) so a fresh install and an
 * explicit 60 behave identically downstream.
 */
export function fpsToStored(fps: number): number | null {
	return fps === 60 ? null : fps;
}
