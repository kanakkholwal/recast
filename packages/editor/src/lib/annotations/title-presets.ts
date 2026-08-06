/**
 * One-click title / lower-third / callout presets. Each inserts a pre-styled,
 * pre-positioned TEXT annotation (which already rasterizes to a PNG at export),
 * plus a dark legibility glow so light text stays readable over any recording.
 * No new schema — presets are just good defaults over the existing text kind.
 */

import type { AnnotationGlow, AnnotationKind } from "../../stores/editor-store.svelte";

export type TextKind = Extract<AnnotationKind, { kind: "text" }>;

export interface TitlePreset {
	id: string;
	label: string;
	/** Editable placeholder inserted as the text content. */
	placeholder: string;
	/** Builds a fresh text kind (factory so presets never share a mutable ref). */
	build: () => TextKind;
	/** Legibility halo applied to the created annotation. */
	glow: AnnotationGlow;
}

const FONT = "'Geist Variable', system-ui, sans-serif";
// Soft dark halo (~7px at 1080p) so white text reads over any background.
const LEGIBILITY_GLOW: AnnotationGlow = { color: "#000000", blur: 0.014, opacity: 0.7 };

function text(
	over: Partial<TextKind> & Pick<TextKind, "x" | "y" | "w" | "h" | "content">,
): TextKind {
	return {
		kind: "text",
		fontFamily: FONT,
		fontSize: 0.06,
		fontWeight: 600,
		color: "#ffffff",
		align: "center",
		lineHeight: 1.2,
		...over,
	};
}

export const TITLE_PRESETS: TitlePreset[] = [
	{
		id: "title",
		label: "Title",
		placeholder: "Title",
		glow: LEGIBILITY_GLOW,
		build: () =>
			text({
				x: 0.12,
				y: 0.12,
				w: 0.76,
				h: 0.16,
				content: "Title",
				fontSize: 0.09,
				fontWeight: 700,
				lineHeight: 1.15,
			}),
	},
	{
		id: "subtitle",
		label: "Subtitle",
		placeholder: "Subtitle",
		glow: LEGIBILITY_GLOW,
		build: () =>
			text({
				x: 0.18,
				y: 0.3,
				w: 0.64,
				h: 0.1,
				content: "Subtitle",
				fontSize: 0.045,
				fontWeight: 500,
			}),
	},
	{
		id: "lower-third",
		label: "Lower third",
		placeholder: "Name",
		glow: LEGIBILITY_GLOW,
		build: () =>
			text({
				x: 0.06,
				y: 0.8,
				w: 0.6,
				h: 0.1,
				content: "Name",
				fontSize: 0.05,
				fontWeight: 600,
				align: "left",
			}),
	},
	{
		id: "callout",
		label: "Callout",
		placeholder: "Callout",
		glow: LEGIBILITY_GLOW,
		build: () => text({ x: 0.3, y: 0.45, w: 0.4, h: 0.1, content: "Callout", fontSize: 0.05 }),
	},
];
