import { REF_ASPECTS, REF_GRADIENTS, REF_MAGIC, REF_MESH, REF_SOLIDS } from "./backgrounds-data";
import type {
	AspectPreset,
	BackgroundPreset,
	PerspectivePreset,
	Template,
	Transform3D,
} from "./types";

export const DEFAULT_TRANSFORM: Transform3D = {
	perspective: 1000,
	rotateX: 0,
	rotateY: 0,
	rotateZ: 0,
	scale: 1,
	translateX: 0,
	translateY: 0,
};

/** Build a full transform from a partial, defaulting the rest (incl. the new
 * translate fields) so preset/template literals stay terse. */
export function t3d(p: Partial<Transform3D>): Transform3D {
	return { ...DEFAULT_TRANSFORM, ...p };
}

/** One-click 3D tilts, ported from the reference app's perspective presets. */
export const PERSPECTIVE_PRESETS: PerspectivePreset[] = [
	tilt("flat", "Flat", {}),
	tilt("left", "Left", { rotateX: 3, rotateY: -12 }),
	tilt("right", "Right", { rotateX: 3, rotateY: 12 }),
	tilt("up", "Up", { rotateX: 12 }),
	tilt("dynamic", "Dynamic", { perspective: 800, rotateX: 10, rotateY: -22 }),
	tilt("dramatic", "Dramatic", { perspective: 900, rotateX: 28, rotateZ: -18, scale: 0.95 }),
];

/** Full classic-gradient set (102) transcribed from the reference app. */
export const GRADIENT_PRESETS: BackgroundPreset[] = REF_GRADIENTS;

/** Dark "magic" gradients (100): radial/conic/pattern glows on near-black. */
export const MAGIC_PRESETS: BackgroundPreset[] = REF_MAGIC;

/** Mesh gradients (12): soft layered radial blobs. */
export const MESH_PRESETS: BackgroundPreset[] = REF_MESH;

/** Tiling patterns built from repeating gradients plus a base color, sized per
 * layer via the shorthand `/ <size>`, so they also fit the `gradient` kind. */
export const PATTERN_PRESETS: BackgroundPreset[] = [
	patternBg(
		"dots",
		"Dots",
		"radial-gradient(#334155 1.5px, transparent 1.6px) 0 0 / 20px 20px, #0f172a",
	),
	patternBg(
		"grid",
		"Grid",
		"linear-gradient(#1e293b 1px, transparent 1px) 0 0 / 24px 24px, linear-gradient(90deg, #1e293b 1px, transparent 1px) 0 0 / 24px 24px, #0f172a",
	),
	patternBg(
		"graph",
		"Graph",
		"linear-gradient(#e2e8f0 1px, transparent 1px) 0 0 / 20px 20px, linear-gradient(90deg, #e2e8f0 1px, transparent 1px) 0 0 / 20px 20px, #f8fafc",
	),
	patternBg(
		"diagonal",
		"Lines",
		"repeating-linear-gradient(45deg, #1f2937 0 1px, transparent 1px 14px) 0 0 / auto, #111827",
	),
];

/** Solid backdrops (33) transcribed from the reference app. */
export const SOLID_PRESETS: BackgroundPreset[] = REF_SOLIDS;

export const DEFAULT_BACKGROUND = GRADIENT_PRESETS[0];

/** Output aspect ratios (25 + Auto), transcribed from the reference so the
 * social/app-store share targets match one-for-one. `Auto` keeps the
 * screenshot's own ratio. */
export const ASPECT_PRESETS: AspectPreset[] = REF_ASPECTS;

/** Reference opens at 4:3 ("Traditional"); fall back to the first entry. */
export const DEFAULT_ASPECT = ASPECT_PRESETS.find((a) => a.id === "4_3") ?? ASPECT_PRESETS[0];

function tilt(id: string, label: string, transform: Partial<Transform3D>): PerspectivePreset {
	return { id, label, transform: t3d(transform) };
}

const NO_BORDER = { width: 0, color: "#ffffff" };
const FLAT: Transform3D = { ...DEFAULT_TRANSFORM };
const NO_MOCKUP = { kind: "none" as const, theme: "light" as const, url: "example.com" };

/** One-click coordinated looks (background + frame + shadow + mockup + 3D). */
export const TEMPLATE_PRESETS: Template[] = [
	{
		id: "clean",
		label: "Clean",
		backgroundId: "solid-white",
		background: { kind: "solid", color: "#ffffff" },
		padding: 8,
		radius: 12,
		shadow: { x: 0, y: 20, blur: 45, spread: 0, opacity: 0.18, color: "#000000" },
		mockup: NO_MOCKUP,
		transform: FLAT,
		swatch: "#ffffff",
	},
	{
		id: "vivid",
		label: "Vivid",
		backgroundId: MESH_PRESETS[0].id,
		background: { kind: "gradient", css: MESH_PRESETS[0].swatch },
		padding: 11,
		radius: 16,
		shadow: { x: 0, y: 30, blur: 70, spread: 0, opacity: 0.4, color: "#000000" },
		mockup: NO_MOCKUP,
		transform: FLAT,
		swatch: MESH_PRESETS[0].swatch,
	},
	{
		id: "browser",
		label: "Browser",
		backgroundId: "grad-azure",
		background: { kind: "gradient", css: "linear-gradient(135deg, #57c0e6 0%, #2b7ec9 100%)" },
		padding: 9,
		radius: 12,
		shadow: { x: 0, y: 26, blur: 60, spread: 0, opacity: 0.32, color: "#000000" },
		mockup: { kind: "safari", theme: "light", url: "example.com" },
		transform: t3d({ perspective: 1000, rotateX: 3, rotateY: -8 }),
		swatch: "linear-gradient(135deg, #57c0e6 0%, #2b7ec9 100%)",
	},
	{
		id: "tilted",
		label: "Tilted",
		backgroundId: "grad-dusk",
		background: {
			kind: "gradient",
			css: "linear-gradient(135deg, #595883 0%, #263455 100%)",
		},
		padding: 12,
		radius: 14,
		shadow: { x: 0, y: 34, blur: 72, spread: 0, opacity: 0.42, color: "#000000" },
		mockup: NO_MOCKUP,
		transform: t3d({ perspective: 800, rotateX: 10, rotateY: -22 }),
		swatch: "linear-gradient(135deg, #595883 0%, #263455 100%)",
	},
	{
		id: "mono",
		label: "Mono",
		backgroundId: "solid-ink",
		background: { kind: "solid", color: "#252422" },
		padding: 8,
		radius: 10,
		shadow: { x: 0, y: 18, blur: 50, spread: 0, opacity: 0.5, color: "#000000" },
		mockup: { kind: "window", theme: "dark", url: "example.com" },
		transform: FLAT,
		swatch: "#252422",
	},
	{
		id: "bold",
		label: "Bold",
		backgroundId: MESH_PRESETS[3].id,
		background: { kind: "gradient", css: MESH_PRESETS[3].swatch },
		padding: 13,
		radius: 20,
		shadow: { x: 0, y: 40, blur: 90, spread: 0, opacity: 0.45, color: "#000000" },
		mockup: NO_MOCKUP,
		transform: t3d({ perspective: 900, rotateX: 24, rotateZ: -14, scale: 0.96 }),
		swatch: MESH_PRESETS[3].swatch,
	},
];

// Pattern backdrops are `gradient`-kind (a `background` shorthand); the swatch
// reuses the same CSS so the picker preview matches the stage.
function patternBg(id: string, label: string, css: string): BackgroundPreset {
	return { id, label, background: { kind: "gradient", css }, swatch: css };
}
