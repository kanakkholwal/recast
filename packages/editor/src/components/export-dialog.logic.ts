/**
 * Pure export-option maths for ExportDialog: cut/duration derivations, the fps
 * option list, and the GIF loop-count cycle. `computeRemovedDuration` mirrors
 * the Rust export's `collect_export_cuts`. Keep the two in lockstep.
 */

// Runtime import via relative path (not `$lib`): this module is unit-tested and
// the standalone vitest config has no `$lib` alias.
import { totalCutDuration } from "../lib/timeline/cuts";
import type { TimelineCut } from "../lib/timeline/cuts";

export interface FpsOption {
	value: number | null;
	label: string;
	desc: string;
}

/** Source fps, clamped to a sane minimum and rounded (default 60 when unknown). */
export function clampSourceFps(fps: number | undefined | null): number {
	return Math.max(1, Math.round(fps ?? 60));
}

/**
 * Source rate plus standard lower rates only, never higher (duplicating frames
 * adds size without smoothness). A stored choice above the current source falls
 * back to Original via the Rust-side clamp.
 */
export function buildFpsOptions(sourceFps: number): FpsOption[] {
	return [
		{ value: null, label: "Original", desc: `${sourceFps} fps` },
		...[60, 30, 24]
			.filter((f) => f < sourceFps)
			.map((f) => ({
				value: f as number | null,
				label: `${f} fps`,
				desc: f === 24 ? "Cinematic" : "Smaller file",
			})),
	];
}

/**
 * Seconds removed by the (already opt-filtered) cuts once clamped to the trim
 * window and merged. Mirrors the Rust export's collect_export_cuts.
 */
export function computeRemovedDuration(
	cuts: TimelineCut[],
	trimStart: number,
	clipEnd: number,
): number {
	if (cuts.length === 0) return 0;
	const clamped = cuts
		.map((c) => ({
			...c,
			start: Math.max(c.start, trimStart),
			end: Math.min(c.end, clipEnd),
		}))
		.filter((c) => c.end > c.start);
	return totalCutDuration(clamped);
}

/** Trimmed-clip length and post-cut output length. */
export function computeExportDurations(
	clipEnd: number,
	trimStart: number,
	removedDuration: number,
): { clipDuration: number; outputDuration: number } {
	const clipDuration = Math.max(0, clipEnd - trimStart);
	const outputDuration = Math.max(0, clipDuration - removedDuration);
	return { clipDuration, outputDuration };
}
