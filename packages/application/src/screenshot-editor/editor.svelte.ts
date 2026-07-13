import { ASPECT_PRESETS, DEFAULT_ASPECT, DEFAULT_BACKGROUND, DEFAULT_TRANSFORM } from "./presets";
import {
  clipTime,
  DEFAULT_FILTERS,
  DEFAULT_FRAME,
  DEFAULT_MOCKUP,
  DEFAULT_SHADOW,
  DEFAULT_STYLE,
  SHADOW_PRESETS,
  STYLE_PRESETS,
} from "./defaults";
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
  ImageFilters,
  ImageStyle,
  ImageStylePreset,
  Mockup,
  Overlay,
  Shadow,
  ShadowPreset,
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

/** Reactive state for one screenshot-editing session. One-way flow: the UI
 * calls these setters, the stage + controls read the getters. Nothing here
 * touches the DOM, so it stays unit-testable and SSR-safe. */
export class ScreenshotEditorState {
  image = $state<EditorImage | null>(null);
  background = $state<Background>(DEFAULT_BACKGROUND.background);
  backgroundId = $state<string>(DEFAULT_BACKGROUND.id);
  frame = $state<Frame>(structuredClone(DEFAULT_FRAME));
  shadow = $state<Shadow>({ ...DEFAULT_SHADOW });
  shadowPreset = $state<ShadowPreset>("soft");
  imageStyle = $state<ImageStyle>({ ...DEFAULT_STYLE });
  mockup = $state<Mockup>({ ...DEFAULT_MOCKUP });
  transform = $state<Transform3D>({ ...DEFAULT_TRANSFORM });
  aspect = $state<AspectPreset>(DEFAULT_ASPECT);
  overlays = $state<Overlay[]>([]);
  selectedId = $state<string | null>(null);

  // Screenshot color adjustments + size/opacity (mirror the clone's
  // imageFilters/imageScale/imageOpacity).
  filters = $state<ImageFilters>({ ...DEFAULT_FILTERS });
  imageScale = $state<number>(100); // percent of natural fit
  imageOpacity = $state<number>(1); // 0..1

  // Canvas (backdrop) treatment beyond the paint itself.
  backgroundBlur = $state<number>(0); // px blur applied to the backdrop only
  backgroundNoise = $state<number>(0); // 0..100 grain overlay opacity
  canvasRadius = $state<number>(0); // px corner radius on the whole stage

  // Live animation + timeline (playback state is transient — not in history).
  // `playhead` is TIMELINE time; the clip maps it into preset-local time, so a
  // clip can be moved and stretched without touching the preset itself.
  animationId = $state<string | null>(null);
  playing = $state(false);
  playhead = $state(0); // ms along the timeline
  timelineDuration = $state(5000); // ms of track
  loop = $state(true);
  clipStart = $state(0); // ms where the clip begins
  clipDuration = $state<number | null>(null); // ms; null = the preset's own length

  // Export options live with the session so they persist across edits.
  exportFormat = $state<ExportFormat>("png");
  exportScale = $state<number>(2);

  // Editing guides. View-only, so they stay out of history and out of exports
  // (their nodes carry `data-export-ignore`).
  showRulers = $state(false);
  showGrid = $state(false);
  gridSize = $state(50); // px between grid lines

  toggleRulers() {
    this.showRulers = !this.showRulers;
  }

  toggleGrid() {
    this.showGrid = !this.showGrid;
  }

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
  /** The clip's length on the timeline (stretched, if the user resized it). */
  readonly clipLength = $derived(this.clipDuration ?? this.animationPreset?.duration ?? 0);
  readonly clipEnd = $derived(this.clipStart + this.clipLength);
  /** Timeline playhead mapped into preset-local time (see `clipTime`). */
  readonly animationTime = $derived(
    this.animationPreset
      ? clipTime(this.playhead, this.clipStart, this.clipLength, this.animationPreset.duration)
      : 0,
  );

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

  /** Apply a shadow strength preset, mapping to the structured Shadow. */
  setShadowPreset(preset: ShadowPreset) {
    this.shadowPreset = preset;
    this.shadow = { ...SHADOW_PRESETS[preset] };
  }

  /** Apply a style-frame preset; seeds its padding/opacity from the clone map. */
  setImageStylePreset(preset: ImageStylePreset) {
    this.imageStyle = { preset, ...STYLE_PRESETS[preset] };
  }

  patchImageStyle(patch: Partial<ImageStyle>) {
    this.imageStyle = { ...this.imageStyle, ...patch };
  }

  patchMockup(patch: Partial<Mockup>) {
    this.mockup = { ...this.mockup, ...patch };
  }

  patchTransform(patch: Partial<Transform3D>) {
    this.transform = { ...this.transform, ...patch };
  }

  patchFilters(patch: Partial<ImageFilters>) {
    this.filters = { ...this.filters, ...patch };
  }

  resetFilters() {
    this.filters = { ...DEFAULT_FILTERS };
  }

  setImageScale(scale: number) {
    this.imageScale = scale;
  }

  setImageOpacity(opacity: number) {
    this.imageOpacity = Math.max(0, Math.min(1, opacity));
  }

  setBackgroundBlur(px: number) {
    this.backgroundBlur = Math.max(0, px);
  }

  setBackgroundNoise(amount: number) {
    this.backgroundNoise = Math.max(0, Math.min(100, amount));
  }

  setCanvasRadius(px: number) {
    this.canvasRadius = Math.max(0, px);
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
    this.imageStyle = { ...DEFAULT_STYLE };
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

  /** Move an overlay one step in z-order. Array order is paint order, so a
   * higher index sits on top; `+1` raises, `-1` lowers. */
  moveOverlay(id: string, delta: 1 | -1) {
    const from = this.overlays.findIndex((o: Overlay) => o.id === id);
    if (from < 0) return;
    const to = from + delta;
    if (to < 0 || to >= this.overlays.length) return;
    const next = [...this.overlays];
    const [moved] = next.splice(from, 1);
    next.splice(to, 0, moved);
    this.overlays = next;
  }

  // --- Animation ---------------------------------------------------------

  /** Select an animation and start playing it (null clears + stops). The clip
   * lands at the start of the timeline at the preset's natural length. */
  setAnimation(id: string | null) {
    this.animationId = id;
    this.playhead = 0;
    this.playing = id !== null;
    if (id) {
      const preset = presetById(id);
      const len = preset?.duration ?? 0;
      this.clipStart = 0;
      this.clipDuration = len;
      // Grow the track if the clip wouldn't fit.
      if (len > this.timelineDuration) this.timelineDuration = len;
    }
  }

  clearAnimation() {
    this.animationId = null;
    this.playing = false;
    this.playhead = 0;
    this.clipStart = 0;
    this.clipDuration = null;
  }

  togglePlay() {
    if (!this.animationId) return;
    this.playing = !this.playing;
  }

  toggleLoop() {
    this.loop = !this.loop;
  }

  seek(ms: number) {
    this.playhead = Math.max(0, Math.min(this.timelineDuration, ms));
  }

  setTimelineDuration(ms: number) {
    this.timelineDuration = Math.max(1000, ms);
    if (this.playhead > this.timelineDuration) this.playhead = this.timelineDuration;
    // Keep the clip inside the track.
    if (this.clipEnd > this.timelineDuration) {
      this.clipStart = Math.max(0, this.timelineDuration - this.clipLength);
    }
  }

  /** Move/resize the clip, clamped to the track. */
  setClip(startMs: number, durationMs: number) {
    const dur = Math.max(200, Math.min(this.timelineDuration, durationMs));
    this.clipDuration = dur;
    this.clipStart = Math.max(0, Math.min(this.timelineDuration - dur, startMs));
  }

  /** Advance playback by `dtMs` along the timeline. Loops, or stops at the end. */
  advance(dtMs: number) {
    if (!this.playing || this.timelineDuration <= 0) return;
    const next = this.playhead + dtMs;
    if (next >= this.timelineDuration) {
      if (this.loop) this.playhead = next % this.timelineDuration;
      else {
        this.playhead = this.timelineDuration;
        this.playing = false;
      }
    } else {
      this.playhead = next;
    }
  }

  reset() {
    this.background = DEFAULT_BACKGROUND.background;
    this.backgroundId = DEFAULT_BACKGROUND.id;
    this.frame = structuredClone(DEFAULT_FRAME);
    this.shadow = { ...DEFAULT_SHADOW };
    this.shadowPreset = "soft";
    this.imageStyle = { ...DEFAULT_STYLE };
    this.mockup = { ...DEFAULT_MOCKUP };
    this.transform = { ...DEFAULT_TRANSFORM };
    this.aspect = DEFAULT_ASPECT;
    this.overlays = [];
    this.selectedId = null;
    this.filters = { ...DEFAULT_FILTERS };
    this.imageScale = 100;
    this.imageOpacity = 1;
    this.backgroundBlur = 0;
    this.backgroundNoise = 0;
    this.canvasRadius = 0;
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
      shadowPreset: this.shadowPreset,
      imageStyle: this.imageStyle,
      mockup: this.mockup,
      transform: this.transform,
      aspectId: this.aspect.id,
      overlays: this.overlays,
      filters: this.filters,
      imageScale: this.imageScale,
      imageOpacity: this.imageOpacity,
      backgroundBlur: this.backgroundBlur,
      backgroundNoise: this.backgroundNoise,
      canvasRadius: this.canvasRadius,
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
    this.shadowPreset = d.shadowPreset ?? "soft";
    this.imageStyle = d.imageStyle ?? { ...DEFAULT_STYLE };
    this.mockup = d.mockup;
    this.transform = d.transform;
    this.aspect = ASPECT_PRESETS.find((a: AspectPreset) => a.id === d.aspectId) ?? DEFAULT_ASPECT;
    this.overlays = d.overlays ?? [];
    this.filters = d.filters ?? { ...DEFAULT_FILTERS };
    this.imageScale = d.imageScale ?? 100;
    this.imageOpacity = d.imageOpacity ?? 1;
    this.backgroundBlur = d.backgroundBlur ?? 0;
    this.backgroundNoise = d.backgroundNoise ?? 0;
    this.canvasRadius = d.canvasRadius ?? 0;
    if (this.selectedId && !this.overlays.some((o: Overlay) => o.id === this.selectedId)) {
      this.selectedId = null;
    }
    this._lastSnap = snap;
    this._lastAt = Date.now();
    this._restoring = false;
  }
}
