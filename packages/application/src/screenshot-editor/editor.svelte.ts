import { DEFAULT_ASPECT, DEFAULT_BACKGROUND } from "./presets";
import type {
  AspectPreset,
  Background,
  EditorImage,
  ExportFormat,
  Frame,
  Mockup,
} from "./types";

const DEFAULT_FRAME: Frame = { padding: 8, radius: 12, shadow: 45 };
const DEFAULT_MOCKUP: Mockup = { kind: "none", theme: "light", url: "example.com" };

/** Reactive state for one screenshot-editing session. One-way flow: the UI
 * calls these setters, the stage + controls read the getters. Nothing here
 * touches the DOM, so it stays unit-testable and SSR-safe. */
export class ScreenshotEditorState {
  image = $state<EditorImage | null>(null);
  background = $state<Background>(DEFAULT_BACKGROUND.background);
  backgroundId = $state<string>(DEFAULT_BACKGROUND.id);
  frame = $state<Frame>({ ...DEFAULT_FRAME });
  mockup = $state<Mockup>({ ...DEFAULT_MOCKUP });
  aspect = $state<AspectPreset>(DEFAULT_ASPECT);

  // Export options live with the session so they persist across edits.
  exportFormat = $state<ExportFormat>("png");
  exportScale = $state<number>(2);

  /** True once an image is loaded and the stage should render. */
  readonly hasImage = $derived(this.image !== null);

  setImage(image: EditorImage) {
    this.image = image;
  }

  clear() {
    this.image = null;
  }

  setBackground(id: string, background: Background) {
    this.backgroundId = id;
    this.background = background;
  }

  /** Apply a custom solid color, clearing any preset selection. */
  setCustomColor(color: string) {
    this.backgroundId = "custom";
    this.background = { kind: "solid", color };
  }

  patchFrame(patch: Partial<Frame>) {
    this.frame = { ...this.frame, ...patch };
  }

  patchMockup(patch: Partial<Mockup>) {
    this.mockup = { ...this.mockup, ...patch };
  }

  setAspect(aspect: AspectPreset) {
    this.aspect = aspect;
  }

  reset() {
    this.background = DEFAULT_BACKGROUND.background;
    this.backgroundId = DEFAULT_BACKGROUND.id;
    this.frame = { ...DEFAULT_FRAME };
    this.mockup = { ...DEFAULT_MOCKUP };
    this.aspect = DEFAULT_ASPECT;
  }
}
