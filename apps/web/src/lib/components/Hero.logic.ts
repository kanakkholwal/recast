import { type IconComponent, Share2, Video, Wand2 } from "@recast/icons";
import { cubicOut } from "svelte/easing";

export const words = [
	"launch video.",
	"demo.",
	"changelog clip.",
	"screen recording.",
	"screencast.",
	"tutorial.",
	"presentation.",
];

export const platforms = ["macOS", "Windows", "Linux"];

export type HeroStep = {
	id: string;
	label: string;
	icon: IconComponent;
	/** One accent per tag — never two hues on one component. */
	accent: "tangerine" | "lavender" | "green";
	/** Per-step clip. Falls back to the shared hero take until each is shot. */
	src?: string;
	caption: string;
};

// Order is the spine of the whole page, so it must not be re-sorted here without re-sorting the sections below.
export const steps: HeroStep[] = [
	{
		id: "record",
		label: "Record",
		icon: Video,
		accent: "tangerine",
		caption: "Region, window, or full screen. One shortcut, no project setup.",
	},
	{
		id: "polish",
		label: "Polish",
		icon: Wand2,
		accent: "lavender",
		caption: "Smart zoom, cursor smoothing, and silence cuts applied as you record.",
	},
	{
		id: "share",
		label: "Share",
		icon: Share2,
		accent: "green",
		caption: "Export straight to your Drive or Recast Cloud and copy the link.",
	},
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
