/**
 * Pure decision logic + presets for the silence-review popover: detection
 * sensitivity presets, confidence classification, and the keep-margin cut-bounds
 * maths. Extracted so they're testable; the store-coupled state machine
 * (status/pending + detect/cut/dismiss against the editor store) stays in the
 * component.
 */

import type { SilenceDetectOptions } from "../lib/wire-types";

export type Sensitivity = "relaxed" | "balanced" | "aggressive";

/** A small margin kept at each end of a cut so speech onsets/tails beside the
 *  silence are never clipped. */
export const CUT_PADDING = 0.12;
/** Bulk "Cut all" only takes confident suggestions; uncertain ones are left
 *  for the user to judge individually. */
export const BULK_MIN_CONFIDENCE = 0.5;

/** "Balanced" uses the Rust-side defaults; the others trade recall vs false
 *  positives. A lower `threshold` is stricter (a frame counts as speech more
 *  readily, so less is called silence); a higher one is more aggressive. */
export const SENSITIVITY_PRESETS: Record<Sensitivity, SilenceDetectOptions> = {
	relaxed: {
		threshold: 0.35,
		minAudioSilence: 1,
		minSegment: 1.5,
	},
	balanced: {},
	aggressive: {
		threshold: 0.6,
		minAudioSilence: 0.4,
		minSegment: 0.6,
	},
};

export const SENSITIVITY_OPTIONS: Array<{ id: Sensitivity; label: string }> = [
	{ id: "relaxed", label: "Relaxed" },
	{ id: "balanced", label: "Balanced" },
	{ id: "aggressive", label: "Aggressive" },
];

/** Coerce a persisted string into a valid Sensitivity (default "balanced"). */
export function parseSensitivity(v: string): Sensitivity {
	return v === "relaxed" || v === "aggressive" ? v : "balanced";
}

export function confidenceLabel(c: number): string {
	if (c >= 0.66) return "Strong";
	if (c >= 0.4) return "Likely";
	return "Uncertain";
}

export function confidenceTextClass(c: number): string {
	if (c >= 0.66) return "text-success";
	if (c >= 0.4) return "text-warning";
	return "text-muted-foreground";
}

export function confidenceBarClass(c: number): string {
	if (c >= 0.66) return "bg-success";
	if (c >= 0.4) return "bg-warning";
	return "bg-muted-foreground/60";
}

/**
 * Apply the keep-margin to a silence segment and return the cut bounds, or null
 * if the padded region would be too short (< 0.2s) to be worth cutting. The pad
 * is capped at a third of the segment so short silences still cut.
 */
export function cutBounds(
	start: number,
	end: number,
	padding = CUT_PADDING,
): { start: number; end: number } | null {
	const pad = Math.min(padding, (end - start) / 3);
	const s = start + pad;
	const e = end - pad;
	if (e - s < 0.2) return null;
	return { start: s, end: e };
}
