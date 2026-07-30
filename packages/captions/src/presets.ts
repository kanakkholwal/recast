/**
 * Shipped caption themes and the default style.
 *
 * Presets are applied by COPYING their style (never stored by id), so this list
 * can evolve freely. Ordered so the picker reads as a spectrum: the compact
 * Loom default first, then quieter looks, then the one display face.
 *
 * Colours here are DATA (hex), not UI chrome, because they must serialize
 * identically to the ASS export. That is why they are literal hex and not
 * shadcn tokens.
 *
 * `presets.test.ts` holds the bar these have to clear: short entrances,
 * restrained accents, unspoken words legible on the pill, and a size band that
 * keeps a caption from becoming a title card.
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

/** Every preset animates the same way unless it says otherwise: a short fade,
 *  spoken words filling in as they are said. */
const calm = {
	chunk: "phrase",
	chunkSize: 6,
	emphasis: "none",
	emphasisColor: "#ffffff",
	highlight: "progressive",
	entrance: "fade",
	entranceMs: 140,
	holdGaps: true,
} as const;

export const CAPTION_PRESETS: CaptionPreset[] = [
	{
		id: "loom",
		label: "Loom",
		description: "Compact pill, words brighten as spoken",
		style: {
			...base,
			fontFamily: stack("Inter"),
			fontWeight: 600,
			fontSizePct: 3.8,
			background: "box",
			backgroundColor: "#0b0b12",
			backgroundOpacity: 76,
			animation: { ...calm, entrance: "slide", entranceMs: 120 },
		},
	},
	{
		id: "minimal",
		label: "Minimal",
		description: "No backing, just a soft shadow",
		style: {
			...base,
			fontFamily: stack("Inter"),
			fontWeight: 500,
			fontSizePct: 4.2,
			mutedColor: "#9ca3af",
			background: "soft",
			backgroundColor: "#000000",
			backgroundOpacity: 0,
			animation: { ...calm },
		},
	},
	{
		id: "subtitle",
		label: "Subtitle",
		description: "Plain broadcast subtitles, whole line",
		style: {
			...base,
			fontFamily: stack("Inter"),
			fontWeight: 500,
			fontSizePct: 3.6,
			offsetPct: 6,
			background: "none",
			backgroundColor: "#000000",
			backgroundOpacity: 0,
			// Nothing behind the text, so the stroke does the legibility work.
			outlineWidth: 4,
			animation: { ...calm, chunk: "line", highlight: "none", entranceMs: 120 },
		},
	},
	{
		id: "accent",
		label: "Accent",
		description: "Spoken word takes a soft tint",
		style: {
			...base,
			fontFamily: stack("Inter"),
			fontWeight: 600,
			fontSizePct: 4.2,
			mutedColor: "#9ca3af",
			background: "soft",
			backgroundColor: "#000000",
			backgroundOpacity: 0,
			animation: {
				...calm,
				emphasis: "color",
				// Indigo-300: reads as a tint on white text, not a highlighter.
				emphasisColor: "#a5b4fc",
				chunkSize: 5,
			},
		},
	},
	{
		id: "editorial",
		label: "Editorial",
		description: "Serif, whole line, no word highlight",
		style: {
			...base,
			fontFamily: stack("Source Serif 4", "serif"),
			fontWeight: 600,
			fontSizePct: 4.2,
			letterSpacing: -0.005,
			lineHeight: 1.4,
			background: "none",
			backgroundColor: "#000000",
			backgroundOpacity: 0,
			// No pill to sit on, so the stroke is what keeps it readable over
			// bright footage.
			outlineWidth: 4,
			animation: { ...calm, chunk: "line", highlight: "none", entranceMs: 160 },
		},
	},
	{
		id: "bold",
		label: "Bold",
		description: "Uppercase display face for vertical clips",
		style: {
			...base,
			fontFamily: stack("Archivo Black"),
			// Archivo Black ships one weight; asking for 700 would miss the fetch
			// and drop the burn-in to a libass fallback.
			fontWeight: 400,
			fontSizePct: 5.4,
			uppercase: true,
			letterSpacing: 0.005,
			lineHeight: 1.25,
			maxLines: 1,
			maxCharsPerLine: 24,
			mutedColor: "#8b8b93",
			background: "none",
			backgroundColor: "#000000",
			backgroundOpacity: 0,
			outlineWidth: 5,
			animation: { ...calm, chunkSize: 3, entrance: "pop", entranceMs: 150 },
		},
	},
];

/** The editor's starting style: the Loom look, enabled. */
export const DEFAULT_CAPTION_STYLE: CaptionStyle = {
	enabled: true,
	...CAPTION_PRESETS[0].style,
};
