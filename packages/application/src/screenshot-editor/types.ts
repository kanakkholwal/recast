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
  /** Space between the screenshot and the stage edge, as a % of the stage
   * width, so padding is symmetric across aspect ratios. */
  padding: number;
  /** Corner radius in CSS pixels (stage space). */
  radius: number;
  /** Border around the screenshot; width 0 hides it. */
  border: { width: number; color: string };
}

/** Drop shadow beneath the framed screenshot. Composed into a CSS box-shadow. */
export interface Shadow {
  x: number;
  y: number;
  blur: number;
  spread: number;
  /** 0..1 shadow alpha; 0 hides the shadow. */
  opacity: number;
  color: string;
}

/** Chrome wrapped around the screenshot to frame it as an app/browser window
 * or a device. All variants are pure CSS on the DOM stage, so they export
 * identically. Browser kinds fill the frame; device kinds impose their own
 * aspect ratio. */
export type MockupKind = "none" | "window" | "safari" | "chrome" | "phone" | "tablet";

/** Light/dark browser chrome, independent of the app's own theme. */
export type MockupTheme = "light" | "dark";

export interface Mockup {
  kind: MockupKind;
  theme: MockupTheme;
  /** Address-bar text for the browser variants (safari/chrome). */
  url: string;
}

/** 3D transform applied to the framed screenshot (pure CSS perspective). Lower
 * `perspective` = stronger depth; angles in degrees; `scale` 0.5..1.5. */
export interface Transform3D {
  perspective: number;
  rotateX: number;
  rotateY: number;
  rotateZ: number;
  scale: number;
}

/** A named 3D preset for one-click tilts. */
export interface PerspectivePreset {
  id: string;
  label: string;
  transform: Transform3D;
}

/** A selectable output aspect ratio. `ratio` is width/height; `null` means
 * "match the screenshot" (no letterboxing). */
export interface AspectPreset {
  id: string;
  label: string;
  ratio: number | null;
}

/** Overlays sit in an absolute layer over the stage. Position is in percent of
 * the stage so overlays stay put as the preview resizes and on export. */
export interface BaseOverlay {
  id: string;
  /** Center position, 0..100 percent of stage width/height. */
  x: number;
  y: number;
  rotation: number;
}

export type TextAlign = "left" | "center" | "right";

export interface TextOverlay extends BaseOverlay {
  type: "text";
  text: string;
  fontSize: number;
  fontFamily: string;
  fontWeight: number;
  color: string;
  align: TextAlign;
}

/** Shape/arrow annotations drawn over the stage (Phase 6). */
export type ShapeKind = "rectangle" | "ellipse" | "arrow";

export interface ShapeOverlay extends BaseOverlay {
  type: "shape";
  shape: ShapeKind;
  /** Size in percent of the stage (arrows use it as the vector to x2,y2). */
  w: number;
  h: number;
  color: string;
  strokeWidth: number;
  filled: boolean;
}

export type Overlay = TextOverlay | ShapeOverlay;

/** A one-click design: applies a coordinated background + frame + shadow +
 * mockup + 3D, leaving the image and overlays untouched. */
export interface Template {
  id: string;
  label: string;
  backgroundId: string;
  background: Background;
  padding: number;
  radius: number;
  shadow: Shadow;
  mockup: Mockup;
  transform: Transform3D;
  /** A CSS `background` value for the template's swatch. */
  swatch: string;
}

export type ExportFormat = "png" | "jpeg";

export interface ExportSpec {
  format: ExportFormat;
  /** Device-pixel multiplier for the snapshot (1..4). */
  scale: number;
}
