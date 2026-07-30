import { BACKGROUND_COLORS, BACKGROUND_GRADIENTS } from "@recast/design/backgrounds";
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
};

/** One-click 3D tilts, ported from the reference app's perspective presets. */
export const PERSPECTIVE_PRESETS: PerspectivePreset[] = [
	tilt("flat", "Flat", { ...DEFAULT_TRANSFORM }),
	tilt("left", "Left", { perspective: 1000, rotateX: 3, rotateY: -12, rotateZ: 0, scale: 1 }),
	tilt("right", "Right", { perspective: 1000, rotateX: 3, rotateY: 12, rotateZ: 0, scale: 1 }),
	tilt("up", "Up", { perspective: 1000, rotateX: 12, rotateY: 0, rotateZ: 0, scale: 1 }),
	tilt("dynamic", "Dynamic", { perspective: 800, rotateX: 10, rotateY: -22, rotateZ: 0, scale: 1 }),
	tilt("dramatic", "Dramatic", {
		perspective: 900,
		rotateX: 28,
		rotateY: 0,
		rotateZ: -18,
		scale: 0.95,
	}),
];

/** Curated gradient backdrops, shared with the video editor via
 * `@recast/design/backgrounds` so a name means one colour across the product. */
// Ids are namespaced per list: `backgroundId` is matched across gradients,
// patterns and solids, and the shared ramp reuses names like "coral" in both
// the gradient and solid sets.
export const GRADIENT_PRESETS: BackgroundPreset[] = BACKGROUND_GRADIENTS.map((p) =>
	grad(`grad-${p.id}`, p.label, p.value),
);

/** Mesh gradients: layered radial blobs plus a base color, as one `background`
 * shorthand. Trendier, softer backdrops than a two-stop linear. */
export const MESH_PRESETS: BackgroundPreset[] = [
	mesh(
		"aurora",
		"Aurora",
		"radial-gradient(at 15% 20%, #6366f1 0px, transparent 55%), radial-gradient(at 85% 10%, #ec4899 0px, transparent 50%), radial-gradient(at 75% 85%, #f59e0b 0px, transparent 45%), radial-gradient(at 10% 90%, #22d3ee 0px, transparent 50%), #4f46e5",
	),
	mesh(
		"bloom",
		"Bloom",
		"radial-gradient(at 20% 25%, #f472b6 0px, transparent 50%), radial-gradient(at 80% 20%, #a855f7 0px, transparent 50%), radial-gradient(at 50% 90%, #60a5fa 0px, transparent 50%), #7c3aed",
	),
	mesh(
		"reef",
		"Reef",
		"radial-gradient(at 10% 10%, #2dd4bf 0px, transparent 50%), radial-gradient(at 90% 30%, #0ea5e9 0px, transparent 50%), radial-gradient(at 50% 100%, #6366f1 0px, transparent 55%), #0891b2",
	),
	mesh(
		"ember",
		"Ember",
		"radial-gradient(at 25% 15%, #fb7185 0px, transparent 50%), radial-gradient(at 85% 40%, #f59e0b 0px, transparent 50%), radial-gradient(at 40% 95%, #ef4444 0px, transparent 50%), #b91c1c",
	),
];

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

/** Solid backdrops, shared with the video editor. */
export const SOLID_PRESETS: BackgroundPreset[] = BACKGROUND_COLORS.map((p) =>
	solid(`solid-${p.id}`, p.label, p.value),
);

export const DEFAULT_BACKGROUND = GRADIENT_PRESETS[0];

/** Output aspect ratios. `Auto` keeps the screenshot's own ratio. Social sizes
 * cover the common share targets without the user hunting for pixel dimensions. */
export const ASPECT_PRESETS: AspectPreset[] = [
	{ id: "auto", label: "Auto", ratio: null },
	{ id: "16-9", label: "16:9", ratio: 16 / 9 },
	{ id: "4-3", label: "4:3", ratio: 4 / 3 },
	{ id: "1-1", label: "1:1", ratio: 1 },
	{ id: "4-5", label: "4:5", ratio: 4 / 5 },
	{ id: "191-1", label: "1.91:1", ratio: 1.91 },
];

export const DEFAULT_ASPECT = ASPECT_PRESETS[0];

function tilt(id: string, label: string, transform: Transform3D): PerspectivePreset {
	return { id, label, transform };
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
		backgroundId: "aurora",
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
		transform: { perspective: 1000, rotateX: 3, rotateY: -8, rotateZ: 0, scale: 1 },
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
		transform: { perspective: 800, rotateX: 10, rotateY: -22, rotateZ: 0, scale: 1 },
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
		backgroundId: "ember",
		background: { kind: "gradient", css: MESH_PRESETS[3].swatch },
		padding: 13,
		radius: 20,
		shadow: { x: 0, y: 40, blur: 90, spread: 0, opacity: 0.45, color: "#000000" },
		mockup: NO_MOCKUP,
		transform: { perspective: 900, rotateX: 24, rotateY: 0, rotateZ: -14, scale: 0.96 },
		swatch: MESH_PRESETS[3].swatch,
	},
];

function grad(id: string, label: string, css: string): BackgroundPreset {
	return { id, label, background: { kind: "gradient", css }, swatch: css };
}

// Mesh and pattern backdrops are also `gradient`-kind (a `background` shorthand);
// the swatch reuses the same CSS so the picker preview matches the stage.
function mesh(id: string, label: string, css: string): BackgroundPreset {
	return { id, label, background: { kind: "gradient", css }, swatch: css };
}

function patternBg(id: string, label: string, css: string): BackgroundPreset {
	return { id, label, background: { kind: "gradient", css }, swatch: css };
}

function solid(id: string, label: string, color: string): BackgroundPreset {
	return { id, label, background: { kind: "solid", color }, swatch: color };
}
