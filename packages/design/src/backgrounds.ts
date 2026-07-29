/**
 * Backdrop presets shared by the video editor and the screenshot editor, so a
 * name like "Azure" means one colour across the product.
 *
 * Authored in OKLCH for even lightness/chroma steps, emitted as hex because
 * both renderers parse hex stops only (see `parseGradient`). Three tiers:
 * neutral (C<=0.01), tinted (C<=0.07), vivid (C<=0.15) — a backdrop fills the
 * frame behind the recording, so it stays below `--primary` (C 0.20) rather
 * than competing with the content it frames.
 */

export type BackgroundPresetTier = "neutral" | "tinted" | "vivid";

export interface BackgroundPreset {
	/** Stable slug. Never derive this from `value`, or re-tuning orphans saved projects. */
	id: string;
	label: string;
	/** Hex for colours, a full `linear-gradient(...)` string for gradients. */
	value: string;
	tier: BackgroundPresetTier;
}

/** Reference content luminances: a light app window and a dark IDE. */
const LIGHT_CONTENT_LUMINANCE = 1;
const DARK_CONTENT_LUMINANCE = 0.0129;
/** Below this contrast against the recording, the frame edge stops reading. */
const EDGE_SEPARATION_MIN = 1.3;

function channelToLinear(c: number): number {
	return c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
}

function hexLuminance(hex: string): number | null {
	const s = hex.replace("#", "");
	if (s.length !== 6 && s.length !== 8) return null;
	const [r, g, b] = [0, 2, 4].map((i) =>
		channelToLinear(Number.parseInt(s.slice(i, i + 2), 16) / 255),
	);
	if ([r, g, b].some(Number.isNaN)) return null;
	return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

function contrast(a: number, b: number): number {
	const [hi, lo] = a > b ? [a, b] : [b, a];
	return (hi + 0.05) / (lo + 0.05);
}

/**
 * Whether a backdrop sits too close in luminance to common recording content
 * for the recording's edge to read unaided — the cue that a drop shadow is
 * doing real work rather than decoration. Unknown values (images, wallpapers)
 * return `false`: we can't measure them, and guessing would surprise the user.
 */
export function backgroundNeedsShadow(value: string): boolean {
	const hexes = value.match(/#(?:[0-9a-fA-F]{8}|[0-9a-fA-F]{6})/g);
	if (!hexes?.length) return false;
	const luminances = hexes.map(hexLuminance).filter((l): l is number => l !== null);
	if (!luminances.length) return false;
	const worstAgainstLight = Math.min(
		...luminances.map((l) => contrast(l, LIGHT_CONTENT_LUMINANCE)),
	);
	const worstAgainstDark = Math.min(...luminances.map((l) => contrast(l, DARK_CONTENT_LUMINANCE)));
	return worstAgainstLight < EDGE_SEPARATION_MIN || worstAgainstDark < EDGE_SEPARATION_MIN;
}

/** Solid backdrops, light to dark within each tier. */
export const BACKGROUND_COLORS: BackgroundPreset[] = [
	{ id: "white", label: "White", value: "#ffffff", tier: "neutral" },
	{ id: "ash", label: "Ash", value: "#d8d7d4", tier: "neutral" },
	{ id: "gray", label: "Gray", value: "#a5a4a2", tier: "neutral" },
	{ id: "graphite", label: "Graphite", value: "#565553", tier: "neutral" },
	{ id: "ink", label: "Ink", value: "#252422", tier: "neutral" },
	{ id: "black", label: "Black", value: "#000000", tier: "neutral" },

	{ id: "linen", label: "Linen", value: "#f6e9d5", tier: "tinted" },
	{ id: "blush", label: "Blush", value: "#f7d5d4", tier: "tinted" },
	{ id: "sage", label: "Sage", value: "#cbe2cf", tier: "tinted" },
	{ id: "mist", label: "Mist", value: "#c2dcef", tier: "tinted" },
	{ id: "lilac", label: "Lilac", value: "#d8cfeb", tier: "tinted" },
	{ id: "storm", label: "Storm", value: "#324457", tier: "tinted" },

	{ id: "sand", label: "Sand", value: "#f4c582", tier: "vivid" },
	{ id: "coral", label: "Coral", value: "#e97871", tier: "vivid" },
	{ id: "jade", label: "Jade", value: "#49af7e", tier: "vivid" },
	{ id: "azure", label: "Azure", value: "#0a8fd1", tier: "vivid" },
	{ id: "violet", label: "Violet", value: "#7153b2", tier: "vivid" },
	{ id: "midnight", label: "Midnight", value: "#172a5a", tier: "vivid" },
];

/** Two-stop gradients: hue span <=40deg and lightness delta <=0.25, so they read
 *  as one surface rather than the wide rainbow sweeps they replaced. */
export const BACKGROUND_GRADIENTS: BackgroundPreset[] = [
	{
		id: "slate",
		label: "Slate",
		value: "linear-gradient(135deg, #44484d 0%, #1b2025 100%)",
		tier: "neutral",
	},
	{
		id: "carbon",
		label: "Carbon",
		value: "linear-gradient(135deg, #393836 0%, #171614 100%)",
		tier: "neutral",
	},
	{
		id: "fog",
		label: "Fog",
		value: "linear-gradient(135deg, #d5e0e8 0%, #afc0d2 100%)",
		tier: "tinted",
	},
	{
		id: "linen",
		label: "Linen",
		value: "linear-gradient(135deg, #ece0cc 0%, #e1bbaa 100%)",
		tier: "tinted",
	},
	{
		id: "sage",
		label: "Sage",
		value: "linear-gradient(135deg, #d0e4d3 0%, #9dc9b5 100%)",
		tier: "tinted",
	},
	{
		id: "blush",
		label: "Blush",
		value: "linear-gradient(135deg, #f7d5d4 0%, #e2b0c7 100%)",
		tier: "tinted",
	},
	{
		id: "dusk",
		label: "Dusk",
		value: "linear-gradient(135deg, #595883 0%, #263455 100%)",
		tier: "tinted",
	},
	{
		id: "harbor",
		label: "Harbor",
		value: "linear-gradient(135deg, #3f6f8b 0%, #1f3959 100%)",
		tier: "tinted",
	},
	{
		id: "coral",
		label: "Coral",
		value: "linear-gradient(135deg, #ff987e 0%, #d7606e 100%)",
		tier: "vivid",
	},
	{
		id: "meadow",
		label: "Meadow",
		value: "linear-gradient(135deg, #83d494 0%, #03aa8e 100%)",
		tier: "vivid",
	},
	{
		id: "azure",
		label: "Azure",
		value: "linear-gradient(135deg, #57c0e6 0%, #2b7ec9 100%)",
		tier: "vivid",
	},
	{
		id: "orchid",
		label: "Orchid",
		value: "linear-gradient(135deg, #c889d7 0%, #6e65c5 100%)",
		tier: "vivid",
	},
];

/**
 * Retired stock presets (uiGradients / Color Hunt) mapped to their nearest
 * replacement. Applied when a saved project loads so an old project keeps a
 * highlighted swatch instead of silently reading as "custom".
 */
export const LEGACY_BACKGROUND_VALUES: Record<string, string> = {
	// Colours
	"#eaffd0": "#cbe2cf",
	"#95e1d3": "#cbe2cf",
	"#f5f5f5": "#ffffff",
	"#533483": "#7153b2",
	"#e94560": "#e97871",
	"#f38181": "#e97871",
	"#fce38a": "#f4c582",
	"#0f3460": "#172a5a",
	"#16213e": "#172a5a",
	"#1a1a2e": "#252422",
	// Gradients
	"linear-gradient(135deg, #6366f1 0%, #8b5cf6 50%, #d946ef 100%)":
		"linear-gradient(135deg, #c889d7 0%, #6e65c5 100%)",
	"linear-gradient(120deg, #ff6a00 0%, #ee0979 100%)":
		"linear-gradient(135deg, #ff987e 0%, #d7606e 100%)",
	"linear-gradient(135deg, #2193b0 0%, #6dd5ed 100%)":
		"linear-gradient(135deg, #57c0e6 0%, #2b7ec9 100%)",
	"linear-gradient(135deg, #00c9ff 0%, #92fe9d 100%)":
		"linear-gradient(135deg, #83d494 0%, #03aa8e 100%)",
	"linear-gradient(135deg, #f12711 0%, #f5af19 100%)":
		"linear-gradient(135deg, #ff987e 0%, #d7606e 100%)",
	"linear-gradient(135deg, #11998e 0%, #38ef7d 100%)":
		"linear-gradient(135deg, #83d494 0%, #03aa8e 100%)",
	"linear-gradient(135deg, #7028e4 0%, #e5b2ca 100%)":
		"linear-gradient(135deg, #c889d7 0%, #6e65c5 100%)",
	"linear-gradient(135deg, #c31432 0%, #240b36 100%)":
		"linear-gradient(135deg, #595883 0%, #263455 100%)",
	"linear-gradient(160deg, #141e30 0%, #243b55 100%)":
		"linear-gradient(135deg, #3f6f8b 0%, #1f3959 100%)",
	"linear-gradient(135deg, #232526 0%, #414345 100%)":
		"linear-gradient(135deg, #44484d 0%, #1b2025 100%)",
	"linear-gradient(135deg, #ed4264 0%, #ffedbc 100%)":
		"linear-gradient(135deg, #f7d5d4 0%, #e2b0c7 100%)",
	"linear-gradient(160deg, #43c6ac 0%, #191654 100%)":
		"linear-gradient(135deg, #3f6f8b 0%, #1f3959 100%)",
};

/** Rewrite a retired preset value to its replacement; anything else passes through. */
export function migrateBackgroundValue(value: string): string {
	return LEGACY_BACKGROUND_VALUES[value] ?? value;
}
