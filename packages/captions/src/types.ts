/**
 * Caption data model shared by every renderer: the desktop editor preview
 * (DOM), the export burn-in (Rust/libass, which mirrors these shapes against
 * `src/__fixtures__/caption-parity.json`), and the web player overlay.
 *
 * Everything here is plain data. No store, no registry, no DOM, so it imports
 * cleanly from either app and the Rust side can mirror it field for field.
 */

/** A single transcribed word with its own source-time span (seconds). */
export interface TranscriptWord {
	start: number;
	end: number;
	text: string;
}

/** How many words are shown on screen at once. */
export type CaptionChunk = "line" | "phrase" | "word";
/** Treatment of the word currently being spoken. */
export type CaptionEmphasis = "none" | "color" | "scale";
/** Per-chunk entrance animation. */
export type CaptionEntrance = "none" | "fade" | "pop" | "slide";
/**
 * How spoken vs unspoken words are coloured.
 * - `none`        every word uses the base colour (a plain caption line).
 * - `active`      only the currently-spoken word takes the accent, then reverts
 *                 (the legacy behaviour; absent animations resolve to this).
 * - `progressive` unspoken words are muted; a word turns to the base colour when
 *                 spoken and STAYS there (the Loom karaoke fill).
 */
export type CaptionHighlight = "none" | "active" | "progressive";

export interface CaptionAnimation {
	/** Words shown at once: a full line, a fixed-size phrase, or one word. */
	chunk: CaptionChunk;
	/** Words per chunk when `chunk === 'phrase'`. */
	chunkSize: number;
	/** What happens to the word being spoken. */
	emphasis: CaptionEmphasis;
	/** Accent colour for `emphasis === 'color'` (hex). */
	emphasisColor: string;
	/**
	 * Spoken/unspoken colouring. Optional so a project saved before this field
	 * existed resolves to `active`, preserving how it looked. New defaults and
	 * presets set this explicitly.
	 */
	highlight?: CaptionHighlight;
	/** Entrance animation applied to each chunk as it appears. */
	entrance: CaptionEntrance;
	/** Entrance duration (ms). */
	entranceMs: number;
	/** Keep the active-word emphasis through short silences instead of clearing. */
	holdGaps: boolean;
}

/** How generated captions render over the preview / player / export. */
export interface CaptionStyle {
	enabled: boolean;
	/** CSS font-family stack. */
	fontFamily: string;
	/** Font weight (400-800). */
	fontWeight: number;
	/** Font size as a percent of the preview/video height. */
	fontSizePct: number;
	position: "bottom" | "center" | "top";
	/** Horizontal alignment of the caption block. */
	align: "left" | "center" | "right";
	/** Top/bottom nudge, percent of frame height: + pushes the caption outward
	 *  into the padding, - pulls it back onto the video. Ignored for `center`. */
	offsetPct: number;
	/** Text colour of a spoken word (hex). */
	color: string;
	/** Text colour of an unspoken word (hex), used when highlight is progressive.
	 *  Must clear 4.5:1 against the pill fill, not just against the base colour. */
	mutedColor: string;
	/** Render text in uppercase. */
	uppercase: boolean;
	/** Letter spacing, in em (can be negative). */
	letterSpacing: number;
	/** Backing behind the text: none, soft shadow, or a solid box/pill. */
	background: "none" | "soft" | "box";
	/** Box/pill backing colour (hex), used when `background` is `box`. */
	backgroundColor: string;
	/** Box/pill backing opacity (0-100), used when `background` is `box`. */
	backgroundOpacity: number;
	/** Horizontal pill padding, in em of the caption font size. */
	boxPaddingXEm: number;
	/** Vertical pill padding, in em of the caption font size. */
	boxPaddingYEm: number;
	/** Pill corner radius, in em of the caption font size. Clamped to half the
	 *  pill height at render, so a large value yields a full stadium. */
	boxRadiusEm: number;
	/** Line height as a multiple of font size. Pinned so DOM and ASS agree on
	 *  the pill height. */
	lineHeight: number;
	/** Outline / stroke thickness as a percent of font size (0 = none). */
	outlineWidth: number;
	/** Outline / stroke colour (hex). */
	outlineColor: string;
	/** Max lines shown at once before clamping. */
	maxLines: number;
	/** Greedy line-break width, in characters. Both renderers break at the same
	 *  index off this so the DOM and the ASS `\N` agree without measurement. */
	maxCharsPerLine: number;
	/** Word-by-word animation. Absent = static (a plain, un-highlighted line). */
	animation?: CaptionAnimation;
}

/** A named caption look: the visual half of {@link CaptionStyle}. Applied
 *  wholesale; users then tweak. Built-ins ship a set; extension packs add more
 *  via the asset registry (`captionPreset` kind). */
export interface CaptionPreset {
	id: string;
	label: string;
	description?: string;
	style: Omit<CaptionStyle, "enabled">;
}
