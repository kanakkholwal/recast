import { ASPECT_PRESETS, DEFAULT_ASPECT, DEFAULT_BACKGROUND, DEFAULT_TRANSFORM } from "./presets";
import { presetById } from "./animation";

/** Coalesce rapid edits (a slider drag) into one undo step. */
const HISTORY_COALESCE_MS = 500;
const HISTORY_LIMIT = 40;
import type {
  AspectPreset,
  Background,
  EditorImage,
  ExportFormat,
  Frame,
  Mockup,
  Overlay,
  Shadow,
  ShapeKind,
  ShapeOverlay,
  Template,
  TextOverlay,
  Transform3D,
} from "./types";

function newId(): string {
  return typeof crypto !== "undefined" && crypto.randomUUID
    ? crypto.randomUUID()
    : `ov-${Math.round(performance.now())}-${Math.round(performance.now() % 1000)}`;
}

const DEFAULT_FRAME: Frame = {
  padding: 8,
  radius: 12,
  border: { width: 0, color: "#ffffff" },
};
const DEFAULT_SHADOW: Shadow = {
  x: 0,
  y: 24,
  blur: 60,
  spread: 0,
  opacity: 0.35,
  color: "#000000",
};
const DEFAULT_MOCKUP: Mockup = { kind: "none", theme: "light", url: "example.com" };

/** Reactive state for one screenshot-editing session. One-way flow: the UI
 * calls these setters, the stage + controls read the getters. Nothing here
 * touches the DOM, so it stays unit-testable and SSR-safe. */
export class ScreenshotEditorState {
  image = $state<EditorImage | null>(null);
  background = $state<Background>(DEFAULT_BACKGROUND.background);
  backgroundId = $state<string>(DEFAULT_BACKGROUND.id);
  frame = $state<Frame>(structuredClone(DEFAULT_FRAME));
  shadow = $state<Shadow>({ ...DEFAULT_SHADOW });
  mockup = $state<Mockup>({ ...DEFAULT_MOCKUP });
  transform = $state<Transform3D>({ ...DEFAULT_TRANSFORM });
  aspect = $state<AspectPreset>(DEFAULT_ASPECT);
  overlays = $state<Overlay[]>([]);
  selectedId = $state<string | null>(null);

  // Live animation (playback state is transient — not in history).
  animationId = $state<string | null>(null);
  playing = $state(false);
  playhead = $state(0); // ms into the animation

  // Export options live with the session so they persist across edits.
  exportFormat = $state<ExportFormat>("png");
  exportScale = $state<number>(2);

  // Undo/redo over the design (not the image); coalesced so a slider drag is
  // one step. Driven by a single $effect in the component (see `record`).
  _undo = $state<string[]>([]);
  _redo = $state<string[]>([]);
  _lastSnap = "";
  _lastAt = 0;
  _restoring = false;

  /** True once an image is loaded and the stage should render. */
  readonly hasImage = $derived(this.image !== null);
  readonly canUndo = $derived(this._undo.length > 0);
  readonly canRedo = $derived(this._redo.length > 0);
  readonly selectedOverlay = $derived(
    this.overlays.find((o: Overlay) => o.id === this.selectedId) ?? null,
  );
  readonly animationPreset = $derived(presetById(this.animationId));
  readonly animationDuration = $derived(this.animationPreset?.duration ?? 0);

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

  patchBorder(patch: Partial<Frame["border"]>) {
    this.frame = { ...this.frame, border: { ...this.frame.border, ...patch } };
  }

  patchShadow(patch: Partial<Shadow>) {
    this.shadow = { ...this.shadow, ...patch };
  }

  patchMockup(patch: Partial<Mockup>) {
    this.mockup = { ...this.mockup, ...patch };
  }

  patchTransform(patch: Partial<Transform3D>) {
    this.transform = { ...this.transform, ...patch };
  }

  setTransform(transform: Transform3D) {
    this.transform = { ...transform };
  }

  setAspect(aspect: AspectPreset) {
    this.aspect = aspect;
  }

  /** Apply a one-click template: background + frame + shadow + mockup + 3D,
   * leaving the image, overlays, and aspect ratio as they are. */
  applyTemplate(t: Template) {
    this.backgroundId = t.backgroundId;
    this.background = t.background;
    this.frame = { padding: t.padding, radius: t.radius, border: { width: 0, color: "#ffffff" } };
    this.shadow = { ...t.shadow };
    this.mockup = { ...t.mockup };
    this.transform = { ...t.transform };
  }

  // --- Overlays ----------------------------------------------------------

  /** Add a text overlay at the stage center and select it. Returns its id so
   * the caller can drop straight into edit mode. */
  addText(): string {
    const overlay: TextOverlay = {
      id: newId(),
      type: "text",
      x: 50,
      y: 50,
      rotation: 0,
      text: "Double-click to edit",
      fontSize: 32,
      fontFamily: "Inter, system-ui, sans-serif",
      fontWeight: 600,
      color: "#ffffff",
      align: "center",
    };
    this.overlays = [...this.overlays, overlay];
    this.selectedId = overlay.id;
    return overlay.id;
  }

  addShape(shape: ShapeKind): string {
    const overlay: ShapeOverlay = {
      id: newId(),
      type: "shape",
      shape,
      x: 35,
      y: 35,
      w: 30,
      h: 20,
      rotation: 0,
      color: "#ef4444",
      strokeWidth: 3,
      filled: false,
    };
    this.overlays = [...this.overlays, overlay];
    this.selectedId = overlay.id;
    return overlay.id;
  }

  updateOverlay(id: string, patch: Partial<Overlay>) {
    this.overlays = this.overlays.map((o: Overlay) =>
      o.id === id ? ({ ...o, ...patch } as Overlay) : o,
    );
  }

  removeOverlay(id: string) {
    this.overlays = this.overlays.filter((o: Overlay) => o.id !== id);
    if (this.selectedId === id) this.selectedId = null;
  }

  selectOverlay(id: string | null) {
    this.selectedId = id;
  }

  // --- Animation ---------------------------------------------------------

  /** Select an animation and start playing it (null clears + stops). */
  setAnimation(id: string | null) {
    this.animationId = id;
    this.playhead = 0;
    this.playing = id !== null;
  }

  clearAnimation() {
    this.animationId = null;
    this.playing = false;
    this.playhead = 0;
  }

  togglePlay() {
    if (!this.animationId) return;
    this.playing = !this.playing;
  }

  seek(ms: number) {
    this.playhead = Math.max(0, Math.min(this.animationDuration, ms));
  }

  /** Advance playback by `dtMs`, looping. Called from the component's rAF. */
  advance(dtMs: number) {
    if (!this.playing || this.animationDuration <= 0) return;
    this.playhead = (this.playhead + dtMs) % this.animationDuration;
  }

  reset() {
    this.background = DEFAULT_BACKGROUND.background;
    this.backgroundId = DEFAULT_BACKGROUND.id;
    this.frame = structuredClone(DEFAULT_FRAME);
    this.shadow = { ...DEFAULT_SHADOW };
    this.mockup = { ...DEFAULT_MOCKUP };
    this.transform = { ...DEFAULT_TRANSFORM };
    this.aspect = DEFAULT_ASPECT;
    this.overlays = [];
    this.selectedId = null;
    this.clearAnimation();
  }

  // --- Undo / redo -------------------------------------------------------

  /** Serialize the design (not the image) for the history stack. */
  private serialize(): string {
    return JSON.stringify({
      background: this.background,
      backgroundId: this.backgroundId,
      frame: this.frame,
      shadow: this.shadow,
      mockup: this.mockup,
      transform: this.transform,
      aspectId: this.aspect.id,
      overlays: this.overlays,
    });
  }

  /** Record the current design as a history step. Reads every design field so a
   * single `$effect(() => editor.record())` subscribes to all of them; rapid
   * changes within {@link HISTORY_COALESCE_MS} collapse into one undo entry. */
  record() {
    if (this._restoring) return;
    const cur = this.serialize();
    if (cur === this._lastSnap) return;
    const now = Date.now();
    if (this._lastSnap !== "" && now - this._lastAt > HISTORY_COALESCE_MS) {
      this._undo = [...this._undo, this._lastSnap].slice(-HISTORY_LIMIT);
      this._redo = [];
    }
    this._lastSnap = cur;
    this._lastAt = now;
  }

  undo() {
    if (this._undo.length === 0) return;
    this._redo = [...this._redo, this._lastSnap];
    const prev = this._undo[this._undo.length - 1];
    this._undo = this._undo.slice(0, -1);
    this.restoreSnapshot(prev);
  }

  redo() {
    if (this._redo.length === 0) return;
    this._undo = [...this._undo, this._lastSnap];
    const next = this._redo[this._redo.length - 1];
    this._redo = this._redo.slice(0, -1);
    this.restoreSnapshot(next);
  }

  private restoreSnapshot(snap: string) {
    this._restoring = true;
    const d = JSON.parse(snap);
    this.background = d.background;
    this.backgroundId = d.backgroundId;
    this.frame = d.frame;
    this.shadow = d.shadow;
    this.mockup = d.mockup;
    this.transform = d.transform;
    this.aspect = ASPECT_PRESETS.find((a: AspectPreset) => a.id === d.aspectId) ?? DEFAULT_ASPECT;
    this.overlays = d.overlays ?? [];
    if (this.selectedId && !this.overlays.some((o: Overlay) => o.id === this.selectedId)) {
      this.selectedId = null;
    }
    this._lastSnap = snap;
    this._lastAt = Date.now();
    this._restoring = false;
  }
}
