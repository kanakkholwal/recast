import { MousePointer2, Share2, Video } from "@lucide/svelte";
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

/** Svelte native transition params: snappy in, lands gently. Duration kept in
 *  the 400-600ms band so the staggered ladder reads brisk, not sluggish. */
export const rise = (delay: number) => ({
	y: 16,
	duration: 560,
	delay,
	easing: cubicOut,
});
