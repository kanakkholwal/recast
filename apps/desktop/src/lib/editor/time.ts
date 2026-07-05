/**
 * Shared editor transport-time helpers. Both the player controls and the editor
 * route consume these so frame-stepping and the timecode readout can't drift
 * apart.
 */

import { clockCentis } from "$lib/format/time";
import {
	originalToOutput,
	outputToOriginal,
	type TimeMap,
} from "$lib/timeline/time-map";

/** `M:SS.cc` timecode, seconds in. Delegates to the canonical formatter. */
export function formatTimecode(seconds: number): string {
	return clockCentis(seconds);
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
