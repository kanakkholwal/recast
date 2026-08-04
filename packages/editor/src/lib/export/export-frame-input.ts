/**
 * Export frame mapping (Phase 4a): turns the static export scene + a per-output
 * frame index into the `FrameInput` the offline renderer composites and the
 * ORIGINAL source time to sample the decoded frame at. Everything is static
 * across the export except `playbackTime`, which is the original time for the
 * output frame (mapped through cuts/speed via the time-map) — so cursor, zoom,
 * scene-anim and the sampled video frame all resolve at the same instant, the
 * same way the preview's picture clock does.
 */

import type { FrameInput } from "../../components/frame-params";
import { outputToOriginal, type TimeMap } from "../timeline/time-map";
import { exportFrameTime } from "./browser-export-plan";

/** Scene at output frame `index`: what to composite + the source time to sample. */
export type ExportFrameAt = (
	index: number,
	outputSec: number,
) => {
	input: FrameInput;
	originalSec: number;
};

/**
 * Build the per-output-frame accessor. `base` is every FrameInput field except
 * `playbackTime` (all static for one export); `timeMap` maps output→original so
 * cuts are gapless and per-segment speed is applied. Pure and deterministic.
 */
export function makeExportFrameAt(
	base: Omit<FrameInput, "playbackTime">,
	timeMap: TimeMap,
): ExportFrameAt {
	return (_index, outputSec) => {
		const originalSec = outputToOriginal(timeMap, outputSec);
		return { input: { ...base, playbackTime: originalSec }, originalSec };
	};
}

/** Convenience: frame accessor that derives `outputSec` from the index + fps,
 *  matching the encoder's frame clock (so callers can't drift the two apart). */
export function makeIndexedExportFrameAt(
	base: Omit<FrameInput, "playbackTime">,
	timeMap: TimeMap,
	fps: number,
): (index: number) => { input: FrameInput; originalSec: number } {
	const at = makeExportFrameAt(base, timeMap);
	return (index) => at(index, exportFrameTime(index, fps));
}
