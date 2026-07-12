/**
 * @recast/application/screenshot-editor — beautify a screenshot (native capture
 * on desktop, or upload/paste/drop anywhere) with a backdrop, padding, rounded
 * corners, shadow, and aspect-ratio presets, then export to PNG/JPG or copy to
 * the clipboard. Shared verbatim by apps/web and apps/desktop.
 */

export { default as ScreenshotEditor } from "./ScreenshotEditor.svelte";
export type { ScreenshotEditorProps } from "./ScreenshotEditor.svelte";
export { ScreenshotEditorState } from "./editor.svelte";
export { canExportVideo, exportVideo } from "./video";
export {
  ANIMATION_PRESETS,
  CATEGORY_LABELS,
  presetsByCategory,
  propsAtTime,
  propsToTransform,
} from "./animation";
export type {
  AnimatableProperties,
  AnimationCategory,
  AnimationPreset,
  Easing,
} from "./animation";
export { captureWebsite } from "./website";
export {
  ASPECT_PRESETS,
  GRADIENT_PRESETS,
  MESH_PRESETS,
  PATTERN_PRESETS,
  PERSPECTIVE_PRESETS,
  SOLID_PRESETS,
  TEMPLATE_PRESETS,
} from "./presets";
export type {
  AspectPreset,
  Background,
  BackgroundPreset,
  EditorImage,
  ExportFormat,
  ExportSpec,
  Frame,
  Mockup,
  MockupKind,
  MockupTheme,
  Overlay,
  PerspectivePreset,
  Shadow,
  ShapeKind,
  ShapeOverlay,
  Template,
  TextAlign,
  TextOverlay,
  Transform3D,
} from "./types";
