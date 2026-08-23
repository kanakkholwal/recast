/** Editor defaults and preset->state maps, transcribed from the screenshot-studio
 * reference app so both sides stay in lockstep. Kept framework-free (no runes) so
 * the parity suite can assert these tables without a Svelte runtime.
 *
 * Clone sources: `lib/store/index.ts:717-760` (defaults) and `:1220-1247`
 * (setImageStylePreset / setShadowPreset maps). */
import type {
	Frame,
	ImageFilters,
	ImageStyle,
	ImageStylePreset,
	Mockup,
	Shadow,
	ShadowPreset,
} from "./types";

export const DEFAULT_FRAME: Frame = {
	padding: 8,
	radius: 10, // clone borderRadius default
	border: { width: 0, color: "#ffffff" },
};

/** The clone's INITIAL imageShadow (which is NOT its 'soft' preset). */
export const DEFAULT_SHADOW: Shadow = {
	x: 5,
	y: 8,
	blur: 15,
	spread: 3,
	opacity: 0.5,
	color: "#000000",
};

export const DEFAULT_STYLE: ImageStyle = { preset: "default", padding: 1, opacity: 0.3 };

// Browser chrome defaults to dark (clone's editorMode->browser applies windows-dark).
export const DEFAULT_MOCKUP: Mockup = { kind: "none", theme: "dark", url: "example.com" };

/** Neutral color adjustments (mirrors the clone's `imageFilters` defaults). */
export const DEFAULT_FILTERS: ImageFilters = {
	brightness: 100,
	contrast: 100,
	saturate: 100,
	grayscale: 0,
	sepia: 0,
	hueRotate: 0,
	invert: 0,
	blur: 0,
};

/** shadowPreset -> our structured Shadow (clone `shadowMap`, single-layer). */
export const SHADOW_PRESETS: Record<ShadowPreset, Shadow> = {
	none: { x: 0, y: 0, blur: 0, spread: 0, opacity: 0, color: "#000000" },
	hug: { x: 0, y: 2, blur: 10, spread: 0, opacity: 0.25, color: "#000000" },
	soft: { x: 0, y: 12, blur: 30, spread: 5, opacity: 0.5, color: "#000000" },
	strong: { x: 0, y: 24, blur: 60, spread: 10, opacity: 0.8, color: "#000000" },
};

/** imageStylePreset -> the style-frame's padding/opacity (clone `borderMap`). */
export const STYLE_PRESETS: Record<ImageStylePreset, { padding: number; opacity: number }> = {
	default: { padding: 1, opacity: 0.3 },
	"glass-light": { padding: 1, opacity: 0.25 },
	"glass-dark": { padding: 1, opacity: 0.7 },
	outline: { padding: 0.5, opacity: 0.35 },
	"border-light": { padding: 1, opacity: 0.3 },
	"border-dark": { padding: 1, opacity: 0.3 },
};

/** Map a timeline playhead into preset-local time for a clip placed at
 * `clipStart` and stretched to `clipLength`. Held at the clip's first frame
 * before it starts and its last frame after it ends. Pure so the timeline math
 * is unit-testable independently of the state class. */
export function clipTime(
	playhead: number,
	clipStart: number,
	clipLength: number,
	presetDuration: number,
): number {
	if (clipLength <= 0 || presetDuration <= 0) return 0;
	const local = Math.max(0, Math.min(clipLength, playhead - clipStart));
	return (local / clipLength) * presetDuration;
}
