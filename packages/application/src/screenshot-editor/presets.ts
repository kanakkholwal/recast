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
  tilt("dramatic", "Dramatic", { perspective: 900, rotateX: 28, rotateY: 0, rotateZ: -18, scale: 0.95 }),
];

/** Curated gradient backdrops. `css` is a ready-to-use `background` value and
 * the swatch mirrors it, so the picker preview always matches the stage. */
export const GRADIENT_PRESETS: BackgroundPreset[] = [
  grad("dusk", "Dusk", "linear-gradient(135deg, #6366f1 0%, #a855f7 50%, #ec4899 100%)"),
  grad("ocean", "Ocean", "linear-gradient(135deg, #0ea5e9 0%, #2563eb 100%)"),
  grad("sunset", "Sunset", "linear-gradient(135deg, #f97316 0%, #ec4899 100%)"),
  grad("forest", "Forest", "linear-gradient(135deg, #22c55e 0%, #0d9488 100%)"),
  grad("gold", "Gold", "linear-gradient(135deg, #f59e0b 0%, #ef4444 100%)"),
  grad("slate", "Slate", "linear-gradient(135deg, #334155 0%, #0f172a 100%)"),
  grad("candy", "Candy", "linear-gradient(135deg, #a855f7 0%, #ec4899 50%, #f97316 100%)"),
  grad("mint", "Mint", "linear-gradient(135deg, #34d399 0%, #22d3ee 100%)"),
];

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

/** Neutral solid backdrops for a cleaner, flatter look. */
export const SOLID_PRESETS: BackgroundPreset[] = [
  solid("white", "White", "#ffffff"),
  solid("light", "Light", "#f4f4f5"),
  solid("dark", "Dark", "#18181b"),
  solid("black", "Black", "#000000"),
];

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
    backgroundId: "light",
    background: { kind: "solid", color: "#f4f4f5" },
    padding: 8,
    radius: 12,
    shadow: { x: 0, y: 20, blur: 45, spread: 0, opacity: 0.18, color: "#000000" },
    mockup: NO_MOCKUP,
    transform: FLAT,
    swatch: "#f4f4f5",
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
    backgroundId: "ocean",
    background: { kind: "gradient", css: "linear-gradient(135deg, #0ea5e9 0%, #2563eb 100%)" },
    padding: 9,
    radius: 12,
    shadow: { x: 0, y: 26, blur: 60, spread: 0, opacity: 0.32, color: "#000000" },
    mockup: { kind: "safari", theme: "light", url: "example.com" },
    transform: { perspective: 1000, rotateX: 3, rotateY: -8, rotateZ: 0, scale: 1 },
    swatch: "linear-gradient(135deg, #0ea5e9 0%, #2563eb 100%)",
  },
  {
    id: "tilted",
    label: "Tilted",
    backgroundId: "dusk",
    background: { kind: "gradient", css: "linear-gradient(135deg, #6366f1 0%, #a855f7 50%, #ec4899 100%)" },
    padding: 12,
    radius: 14,
    shadow: { x: 0, y: 34, blur: 72, spread: 0, opacity: 0.42, color: "#000000" },
    mockup: NO_MOCKUP,
    transform: { perspective: 800, rotateX: 10, rotateY: -22, rotateZ: 0, scale: 1 },
    swatch: "linear-gradient(135deg, #6366f1 0%, #a855f7 50%, #ec4899 100%)",
  },
  {
    id: "mono",
    label: "Mono",
    backgroundId: "dark",
    background: { kind: "solid", color: "#18181b" },
    padding: 8,
    radius: 10,
    shadow: { x: 0, y: 18, blur: 50, spread: 0, opacity: 0.5, color: "#000000" },
    mockup: { kind: "window", theme: "dark", url: "example.com" },
    transform: FLAT,
    swatch: "#18181b",
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
