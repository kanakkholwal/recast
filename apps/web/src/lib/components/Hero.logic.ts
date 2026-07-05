import { MousePointer2, Share2, Video } from "@lucide/svelte";
import { cubicOut } from "svelte/easing";

// Concrete artifacts the target audience actually makes, ordered so the
// loop opens with the broadest noun (demo) and rotates through the
// segment-specific outputs (investor walkthrough = founders, launch video
// = indie hackers, changelog clip = devrels, onboarding tour = product
// engineers / solopreneurs). Naming outputs instead of style adjectives
// (the old "cinematic / hand-edited" loop) plants category + audience in
// the same beat and makes the TextLoop animation land on real value.
export const words = [
	"demo.",
	"launch video.",
	"changelog clip.",
	"investor walkthrough.",
	"onboarding tour.",
];

export const platforms = ["macOS", "Windows", "Linux"];

export const steps = [
	{ icon: Video, label: "Record" },
	{ icon: MousePointer2, label: "Auto-polish" },
	{ icon: Share2, label: "Share" },
];

/** Svelte native transition params — snappy in, lands gently. */
export const rise = (delay: number) => ({
	y: 16,
	duration: 720,
	delay,
	easing: cubicOut,
});
