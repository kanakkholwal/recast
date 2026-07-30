// The clip surface, in one place. Every lane paints a block the same way and
// differs only by hue, so the classes live here rather than being retyped (and
// drifting) in five components.
//
// Anatomy, following Premiere/Resolve: a SOLID body in the lane's fill colour,
// the label inside it in near-white, and the bright lane accent reserved for
// the waveform and the state outlines. Blocks used to be a 20-35% wash of the
// accent behind a 2px left spine, which read as a tint of the lane background
// rather than an object you can pick up.
//
// Class strings must stay literal: Tailwind scans source text, so a composed
// name like `bg-lane-${tone}-fill` would never be generated.

export type LaneTone = "zoom" | "markup" | "music" | "audio" | "cut";

export interface ClipSurface {
	/** Solid body. */
	fill: string;
	/** Waveform / sparkline drawn on top of the body. */
	wave: string;
	/** Resize grip. */
	grip: string;
	/** Lane accent, for an icon on the body. */
	accent: string;
}

const SURFACES: Record<LaneTone, ClipSurface> = {
	zoom: {
		fill: "bg-lane-zoom-fill",
		wave: "fill-lane-zoom/70",
		grip: "bg-lane-on/70",
		accent: "text-lane-on",
	},
	markup: {
		fill: "bg-lane-markup-fill",
		wave: "fill-lane-markup/70",
		grip: "bg-lane-on/70",
		accent: "text-lane-on",
	},
	music: {
		fill: "bg-lane-music-fill",
		wave: "fill-lane-music/70",
		grip: "bg-lane-on/70",
		accent: "text-lane-on",
	},
	audio: {
		fill: "bg-lane-audio-fill",
		wave: "fill-lane-audio/70",
		grip: "bg-lane-on/70",
		accent: "text-lane-on",
	},
	cut: {
		fill: "bg-lane-cut-fill",
		wave: "fill-lane-cut/70",
		grip: "bg-lane-on/70",
		accent: "text-lane-on",
	},
};

export function clipSurface(tone: LaneTone): ClipSurface {
	return SURFACES[tone];
}

/**
 * Shape + label treatment shared by every block, whatever the lane.
 *
 * Carries NO position utility. Every consumer positions itself (`absolute
 * inset-0`, or absolute with an inline left/top/height), and Tailwind emits
 * `relative` AFTER `absolute` in the stylesheet — so a `relative` in here won
 * the cascade regardless of class order, dropped `inset-0`, and every clip
 * collapsed to the width and height of its own label.
 */
export const CLIP_BASE =
	"overflow-hidden rounded-[4px] select-none transition-[filter,box-shadow] duration-150";

/** Hover lift. Tone-independent, so it can't fall out of step with a fill. */
export const CLIP_HOVER = "hover:brightness-110";

/**
 * Selected outline. Near-white and inset, the NLE convention — and it keeps
 * `--primary` out of a decorative role (see the 60/30/10 rule).
 */
export const CLIP_SELECTED = "ring-2 ring-inset ring-lane-on";

/** Focus ring for keyboard users; distinct from selection, which can be silent. */
export const CLIP_FOCUS =
	"focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-ring";

/** Label sitting on a filled body. */
export const CLIP_LABEL =
	"pointer-events-none truncate text-[11px] font-semibold leading-none text-lane-on";

/** Secondary text on a filled body (durations, counts). */
export const CLIP_META =
	"pointer-events-none shrink-0 font-mono text-[10px] leading-none tabular-nums text-lane-on/70";
