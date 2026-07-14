/**
 * Shipped caption themes and the default style.
 *
 * Presets are applied by COPYING their style (never stored by id), so this list
 * can evolve freely. Ordered loom -> impact so the picker reads as a spectrum:
 * the compact Loom look first as the default, the heavier looks after.
 *
 * Colours here are DATA (hex), not UI chrome, because they must serialize
 * identically to the ASS export. That is why they are literal hex and not
 * shadcn tokens.
 */

import type { CaptionPreset, CaptionStyle } from "./types";

/** A CSS font-family stack for a webfont, e.g. `'Inter', sans-serif`. */
const stack = (family: string, fallback = "sans-serif") => `'${family}', ${fallback}`;

/** Shared modern base: the pill/typography defaults every preset starts from,
 *  so a new preset only states what makes it distinct. */
const base = {
	position: "bottom",
	align: "center",
	offsetPct: 8,
	color: "#ffffff",
	mutedColor: "#a1a1aa",
	uppercase: false,
	letterSpacing: 0,
	boxPaddingXEm: 0.7,
	boxPaddingYEm: 0.32,
	boxRadiusEm: 0.6,
	lineHeight: 1.35,
	outlineWidth: 0,
	outlineColor: "#0a0a0a",
	maxLines: 2,
	maxCharsPerLine: 42,
} satisfies Partial<Omit<CaptionStyle, "enabled">>;

export const CAPTION_PRESETS: CaptionPreset[] = [
	{
		id: "loom",
		label: "Loom",
		description: "Compact, spoken words brighten",
		style: {
			...base,
			fontFamily: stack("Inter"),
			fontWeight: 600,
			fontSizePct: 3.8,
			background: "box",
			backgroundColor: "#0b0b12",
			backgroundOpacity: 78,
			animation: {
				chunk: "phrase",
				chunkSize: 6,
				emphasis: "none",
				emphasisColor: "#ffffff",
				highlight: "progressive",
				entrance: "slide",
				entranceMs: 125,
				holdGaps: true,
			},
		},
	},
	{
		id: "clean",
		label: "Clean",
		description: "Minimal, no box",
		style: {
			...base,
			fontFamily: stack("Inter"),
			fontWeight: 600,
			fontSizePct: 4.4,
			background: "soft",
			backgroundColor: "#000000",
			backgroundOpacity: 0,
			animation: {
				chunk: "phrase",
				chunkSize: 6,
				emphasis: "none",
				emphasisColor: "#ffffff",
				highlight: "progressive",
				entrance: "fade",
				entranceMs: 125,
				holdGaps: true,
			},
		},
	},
	{
		id: "pill",
		label: "Pill",
		description: "Rounded bar",
		style: {
			...base,
			fontFamily: stack("Plus Jakarta Sans"),
			fontWeight: 700,
			fontSizePct: 4.2,
			background: "box",
			backgroundColor: "#111827",
			backgroundOpacity: 85,
			boxRadiusEm: 1.2,
			animation: {
				chunk: "phrase",
				chunkSize: 5,
				emphasis: "none",
				emphasisColor: "#ffffff",
				highlight: "progressive",
				entrance: "slide",
				entranceMs: 125,
				holdGaps: true,
			},
		},
	},
	{
		id: "spotlight",
		label: "Spotlight",
		description: "Accent on the spoken word",
		style: {
			...base,
			fontFamily: stack("Inter"),
			fontWeight: 700,
			fontSizePct: 4.6,
			background: "box",
			backgroundColor: "#0b0b12",
			backgroundOpacity: 72,
			animation: {
				chunk: "phrase",
				chunkSize: 5,
				emphasis: "color",
				emphasisColor: "#4ade80",
				highlight: "progressive",
				entrance: "slide",
				entranceMs: 125,
				holdGaps: true,
			},
		},
	},
	{
		id: "wave",
		label: "Wave",
		description: "Cyan reveal",
		style: {
			...base,
			fontFamily: stack("Outfit"),
			fontWeight: 700,
			fontSizePct: 4.6,
			mutedColor: "#7dd3fc",
			background: "box",
			backgroundColor: "#082f49",
			backgroundOpacity: 70,
			animation: {
				chunk: "phrase",
				chunkSize: 4,
				emphasis: "color",
				emphasisColor: "#38bdf8",
				highlight: "progressive",
				entrance: "fade",
				entranceMs: 125,
				holdGaps: true,
			},
		},
	},
	{
		id: "punch",
		label: "Punch",
		description: "Word pop",
		style: {
			...base,
			fontFamily: stack("Anton"),
			fontWeight: 700,
			fontSizePct: 6.5,
			position: "center",
			offsetPct: 0,
			uppercase: true,
			letterSpacing: 0.01,
			background: "box",
			backgroundColor: "#0a0a0a",
			backgroundOpacity: 55,
			boxRadiusEm: 0.4,
			maxLines: 1,
			animation: {
				chunk: "word",
				chunkSize: 1,
				emphasis: "scale",
				emphasisColor: "#4ade80",
				highlight: "none",
				entrance: "pop",
				entranceMs: 150,
				holdGaps: true,
			},
		},
	},
	{
		id: "hype",
		label: "Hype",
		description: "Big impact",
		style: {
			...base,
			fontFamily: stack("Anton"),
			fontWeight: 700,
			fontSizePct: 7,
			position: "center",
			offsetPct: 0,
			uppercase: true,
			letterSpacing: 0.01,
			mutedColor: "#a3a3a3",
			background: "none",
			backgroundColor: "#000000",
			backgroundOpacity: 0,
			outlineWidth: 6,
			animation: {
				chunk: "phrase",
				chunkSize: 3,
				emphasis: "color",
				emphasisColor: "#fde047",
				highlight: "progressive",
				entrance: "pop",
				entranceMs: 150,
				holdGaps: true,
			},
		},
	},
];

/** The editor's starting style: the Loom look, enabled. */
export const DEFAULT_CAPTION_STYLE: CaptionStyle = {
	enabled: true,
	...CAPTION_PRESETS[0].style,
};
