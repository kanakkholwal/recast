/** Types shared across the screenshot editor. Kept framework-free so the state
 * store, presets, and export code stay testable without a component. */

/** A loaded source image plus its intrinsic pixel size (needed for aspect and
 * export math). `src` is a data/blob/object URL usable directly in `<img>`. */
export interface EditorImage {
  src: string;
  width: number;
  height: number;
}

/** How the stage backdrop is painted behind the screenshot. */
export type Background =
  | { kind: "solid"; color: string }
  | { kind: "gradient"; css: string }
  | { kind: "transparent" };

/** A named backdrop the user can pick from the swatch grid. */
export interface BackgroundPreset {
  id: string;
  label: string;
  background: Background;
  /** Small CSS value used to paint the swatch (mirrors the backdrop). */
  swatch: string;
}

/** Frame treatment applied to the screenshot itself (not the backdrop). */
export interface Frame {
  /** Space between the screenshot and the stage edge, as a % of the stage's
   * shorter side, so padding looks consistent across aspect ratios. */
  padding: number;
  /** Corner radius in CSS pixels (stage space). */
  radius: number;
  /** Drop-shadow strength, 0..100; 0 hides the shadow. */
  shadow: number;
}

/** Chrome wrapped around the screenshot to frame it as an app/browser window.
 * All variants are pure CSS on the DOM stage, so they export identically. */
export type MockupKind = "none" | "window" | "safari" | "chrome";

/** Light/dark browser chrome, independent of the app's own theme. */
export type MockupTheme = "light" | "dark";

export interface Mockup {
  kind: MockupKind;
  theme: MockupTheme;
  /** Address-bar text for the browser variants (safari/chrome). */
  url: string;
}

/** A selectable output aspect ratio. `ratio` is width/height; `null` means
 * "match the screenshot" (no letterboxing). */
export interface AspectPreset {
  id: string;
  label: string;
  ratio: number | null;
}

export type ExportFormat = "png" | "jpeg";

export interface ExportSpec {
  format: ExportFormat;
  /** Device-pixel multiplier for the snapshot (1..4). */
  scale: number;
}
