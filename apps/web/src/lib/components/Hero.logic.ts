import { MousePointer2, Share2, Video } from "@recast/icons";
import { cubicOut } from "svelte/easing";

// Concrete artifacts the committed audience (solo founders + dev teams /
// DevRel) actually ships. Narrowed from five to three: opens on the broadest
// noun (demo), then the two on-message outputs both segments make (launch
// video, changelog clip). Dropped "investor walkthrough" (pure founder) and
// "onboarding tour" (support) so the loop stops chasing every market at once.
export const words = ["demo.", "launch video.", "changelog clip."];

export const platforms = ["macOS", "Windows", "Linux"];

// Editorial hero backdrop. Swap for a local `/hero-backdrop.webp` or any
// Unsplash/Pexels landscape. If it fails to load, the base gradient behind it
// shows through, so the hero never renders broken. Kept here (not inline) so
// the one place to change the art is obvious.
export const backdropUrl =
	"/background-hero.webp";

export const steps = [
	{ icon: Video, label: "Record" },
	{ icon: MousePointer2, label: "Auto-polish" },
	{ icon: Share2, label: "Share" },
];

/**
 * Hero entrance choreography.
 *
 * Each element rises 12px and lands in 460ms on a `cubicOut` curve — under the
 * 500ms UI ceiling so the first paint reads as decisive, not decorative. The
 * per-element delays follow a tight 80ms ladder (was 120ms; the wider gap
 * left a visible beat between the CTA and the preview figure). Total ladder
 * ends ~960ms after first paint — slow enough to feel premium, short enough
 * not to gate the visitor's scroll.
 */
export const rise = (delay: number) => ({
	y: 12,
	duration: 460,
	delay,
	easing: cubicOut,
});

/** Stagger between consecutive hero elements. Tighter than the previous 120ms
 *  reads as a single confident breath rather than a stage-by-stage march. */
export const heroStagger = 80;
