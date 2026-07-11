import type { AspectPreset, BackgroundPreset } from "./types";

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

function grad(id: string, label: string, css: string): BackgroundPreset {
  return { id, label, background: { kind: "gradient", css }, swatch: css };
}

function solid(id: string, label: string, color: string): BackgroundPreset {
  return { id, label, background: { kind: "solid", color }, swatch: color };
}
