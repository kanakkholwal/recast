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
import { keyframesToPreset, presetById } from "./animation";
import type { AnimatableProperties, Easing, KeyframeEntry } from "./animation";
import { DEFAULT_FONT_ID } from "./fonts";

/** Coalesce rapid edits (a slider drag) into one undo step. */
const HISTORY_COALESCE_MS = 500;
const HISTORY_LIMIT = 40;
/** Stage corner radius default (clone `backgroundBorderRadius`). */
const DEFAULT_CANVAS_RADIUS = 10;
import type {
	AspectPreset,
	Background,
	EditorImage,
	ExportFormat,
	Frame,
	ImageFilters,
	ImageStyle,
	ImageStylePreset,
	BlurOverlay,
	DesignObject,
	EditorSnapshot,
	ImageOverlay,
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
	// Slideshow: the set of images that share the current design. `image` mirrors
	// the active slide; a single upload is just a one-slide set. Used for batch
	// export. (Persisted in the autosave snapshot, not in design/history.)
	slides = $state<EditorImage[]>([]);
	activeSlide = $state(0);
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
	canvasRadius = $state<number>(DEFAULT_CANVAS_RADIUS); // px corner radius on the whole stage

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

	// User-authored keyframes. When any exist they become a synthetic preset that
	// drives playback/export, overriding any selected preset (persisted in the
	// design; selection is transient).
	keyframes = $state<KeyframeEntry[]>([]);
	selectedKeyframeId = $state<string | null>(null);

	// Export options live with the session so they persist across edits.
	exportFormat = $state<ExportFormat>("png");
	exportScale = $state<number>(2);
	exportQuality = $state<number>(0.95); // 0..1, used by jpeg/webp only

	// Editing guides. View-only, so they stay out of history and out of exports
	// (their nodes carry `data-export-ignore`).
	showRulers = $state(false);
	showGrid = $state(false);
	gridSize = $state(50); // px between grid lines
	rulerInterval = $state(100); // px between major ruler ticks (clone default)

	toggleRulers() {
		this.showRulers = !this.showRulers;
	}

	setRulerInterval(px: number) {
		this.rulerInterval = Math.max(10, Math.round(px));
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
	/** User keyframes as a synthetic preset (null when none). */
	readonly keyframePreset = $derived(keyframesToPreset(this.keyframes));
	/** True when user keyframes drive the animation. */
	readonly keyframeMode = $derived(this.keyframes.length >= 1);
	/** The preset actually played/exported: user keyframes win over a preset. */
	readonly activePreset = $derived(this.keyframePreset ?? this.animationPreset);
	readonly animationDuration = $derived(this.activePreset?.duration ?? 0);
	/** The clip's length on the timeline (stretched, if the user resized it). */
	readonly clipLength = $derived(this.clipDuration ?? this.activePreset?.duration ?? 0);
	readonly clipEnd = $derived(this.clipStart + this.clipLength);
	/** Timeline playhead mapped into preset-local time (see `clipTime`). */
	readonly animationTime = $derived(
		this.activePreset
			? clipTime(this.playhead, this.clipStart, this.clipLength, this.activePreset.duration)
			: 0,
	);

	setImage(image: EditorImage) {
		// A fresh upload starts from default styling (matches the reference's
		// reset-on-upload), but keeps the chosen aspect ratio. First load is a
		// no-op reset since everything is already default. Resets the slide set.
		if (this.image) this.resetDesign(true);
		this.image = image;
		this.slides = [image];
		this.activeSlide = 0;
	}

	/** Add another image that shares the current design (does NOT reset styling). */
	addSlide(image: EditorImage) {
		this.slides = [...this.slides, image];
		this.activeSlide = this.slides.length - 1;
		this.image = image;
	}

	/** Switch the displayed image to another slide, keeping the design. */
	setActiveSlide(i: number) {
		if (i < 0 || i >= this.slides.length) return;
		this.activeSlide = i;
		this.image = this.slides[i];
	}

	removeSlide(i: number) {
		if (i < 0 || i >= this.slides.length) return;
		// Track the active slide by identity so removing one BEFORE it keeps the
		// same slide active (index-clamping would shift to a different image).
		const activeImg = this.slides[this.activeSlide];
		const next = this.slides.filter((_, j) => j !== i);
		this.slides = next;
		if (next.length === 0) {
			this.image = null;
			this.activeSlide = 0;
			return;
		}
		const keep = next.indexOf(activeImg);
		this.activeSlide = keep >= 0 ? keep : Math.min(i, next.length - 1);
		this.image = next[this.activeSlide];
	}

	clear() {
		this.image = null;
		this.slides = [];
		this.activeSlide = 0;
		this.resetDesign(false);
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

	/** Apply a style-frame preset; seeds its padding/opacity from the clone map.
	 * Style frame and mockup are mutually exclusive (the clone models both as one
	 * `imageBorder.type`), so a non-default style clears any active mockup. */
	setImageStylePreset(preset: ImageStylePreset) {
		this.imageStyle = { preset, ...STYLE_PRESETS[preset] };
		if (preset !== "default" && this.mockup.kind !== "none") {
			this.mockup = { ...this.mockup, kind: "none" };
		}
	}

	patchImageStyle(patch: Partial<ImageStyle>) {
		this.imageStyle = { ...this.imageStyle, ...patch };
	}

	patchMockup(patch: Partial<Mockup>) {
		this.mockup = { ...this.mockup, ...patch };
		// Applying a mockup replaces the style frame (they are exclusive).
		if (this.mockup.kind !== "none" && this.imageStyle.preset !== "default") {
			this.imageStyle = { ...DEFAULT_STYLE };
		}
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
			opacity: 1,
			isVisible: true,
			text: "Text",
			fontSize: 24,
			fontFamily: DEFAULT_FONT_ID,
			fontWeight: "normal",
			color: "#ffffff",
			align: "center",
			orientation: "horizontal",
			shadow: { enabled: true, color: "rgba(0,0,0,0.5)", blur: 4, offsetX: 2, offsetY: 2 },
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
			opacity: 1,
			isVisible: true,
			strokeColor: "#ef4444",
			fillColor: "#ef4444",
			strokeWidth: 6,
			filled: false,
		};
		this.overlays = [...this.overlays, overlay];
		this.selectedId = overlay.id;
		return overlay.id;
	}

	/** Duplicate an overlay (offset a touch) and select the copy. */
	duplicateOverlay(id: string): string | null {
		const src = this.overlays.find((o: Overlay) => o.id === id);
		if (!src) return null;
		const copy = {
			...structuredClone($state.snapshot(src)),
			id: newId(),
			x: Math.min(95, src.x + 4),
			y: Math.min(95, src.y + 4),
		} as Overlay;
		// A duplicate is never the singleton light/shadow overlay.
		if (copy.type === "image") copy.role = undefined;
		this.overlays = [...this.overlays, copy];
		this.selectedId = copy.id;
		return copy.id;
	}

	toggleOverlayVisible(id: string) {
		this.updateOverlay(id, {
			isVisible: !(this.overlays.find((o: Overlay) => o.id === id)?.isVisible ?? true),
		});
	}

	/** Add an image/sticker overlay (custom upload or a built-in overlay asset). */
	addImageOverlay(
		src: string,
		opts: { size?: number; isCustom?: boolean; opacity?: number } = {},
	): string {
		const overlay: ImageOverlay = {
			id: newId(),
			type: "image",
			x: 50,
			y: 50,
			rotation: 0,
			opacity: opts.opacity ?? 1,
			isVisible: true,
			src,
			size: opts.size ?? 40,
			blur: 0,
			flipX: false,
			flipY: false,
			layer: "front",
			isCustom: opts.isCustom ?? false,
		};
		this.overlays = [...this.overlays, overlay];
		this.selectedId = overlay.id;
		return overlay.id;
	}

	/** Add a rectangular blur/redaction region and select it. */
	addBlur(): string {
		const overlay: BlurOverlay = {
			id: newId(),
			type: "blur",
			x: 35,
			y: 35,
			w: 30,
			h: 20,
			rotation: 0,
			opacity: 1,
			isVisible: true,
			blurAmount: 10,
		};
		this.overlays = [...this.overlays, overlay];
		this.selectedId = overlay.id;
		return overlay.id;
	}

	/** The single light/shadow overlay, if one is applied. */
	readonly shadowOverlay = $derived(
		this.overlays.find((o: Overlay) => o.type === "image" && o.role === "shadow") as
			| ImageOverlay
			| undefined,
	);

	/** Apply a built-in light/shadow overlay as a singleton (replaces any
	 * existing one); `null` removes it. Kept at the back of the overlay stack so
	 * stickers/text stay above it, but above the screenshot. */
	setShadowOverlay(url: string | null) {
		const existing = this.overlays.find(
			(o: Overlay) => o.type === "image" && o.role === "shadow",
		) as ImageOverlay | undefined;
		if (!url) {
			if (existing) this.removeOverlay(existing.id);
			return;
		}
		if (existing) {
			this.updateOverlay(existing.id, { src: url });
			return;
		}
		const overlay: ImageOverlay = {
			id: newId(),
			type: "image",
			x: 50,
			y: 50,
			rotation: 0,
			opacity: 0.5,
			isVisible: true,
			src: url,
			size: 100,
			blur: 0,
			flipX: false,
			flipY: false,
			layer: "front",
			isCustom: false,
			role: "shadow",
		};
		// Insert at the bottom of the stack so it underlays other overlays.
		this.overlays = [overlay, ...this.overlays];
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

	/** Reorder in z (paint) order: raise/lower one step or jump to front/back. */
	reorderOverlay(id: string, where: "up" | "down" | "top" | "bottom") {
		const from = this.overlays.findIndex((o: Overlay) => o.id === id);
		if (from < 0) return;
		const next = [...this.overlays];
		const [moved] = next.splice(from, 1);
		const to =
			where === "top"
				? next.length
				: where === "bottom"
					? 0
					: Math.max(0, Math.min(next.length, from + (where === "up" ? 1 : -1)));
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
		if (!this.animationId && !this.keyframeMode) return;
		this.playing = !this.playing;
	}

	// --- Keyframes ---------------------------------------------------------

	/** The live 3D transform + opacity as a full animatable snapshot. */
	private currentAnimProps(): AnimatableProperties {
		const t = this.transform;
		return {
			rotateX: t.rotateX,
			rotateY: t.rotateY,
			rotateZ: t.rotateZ,
			scale: t.scale,
			translateX: t.translateX,
			translateY: t.translateY,
			perspective: t.perspective,
			opacity: this.imageOpacity,
		};
	}

	/** Capture the current 3D look as a keyframe at `time` (defaults to the
	 * playhead). Re-captures an existing keyframe within 30ms instead of stacking. */
	addKeyframe(time?: number): string {
		const at = Math.max(0, Math.round(time ?? this.playhead));
		const props = this.currentAnimProps();
		const existing = this.keyframes.find((k) => Math.abs(k.time - at) < 30);
		if (existing) {
			this.updateKeyframe(existing.id, { props });
			this.selectedKeyframeId = existing.id;
			return existing.id;
		}
		const kf: KeyframeEntry = { id: newId(), time: at, props, easing: "ease-in-out" };
		this.keyframes = [...this.keyframes, kf].sort((a, b) => a.time - b.time);
		this.selectedKeyframeId = kf.id;
		if (at > this.timelineDuration) this.timelineDuration = at;
		this.clipStart = 0;
		this.clipDuration = null;
		return kf.id;
	}

	updateKeyframe(id: string, patch: Partial<Omit<KeyframeEntry, "id">>) {
		this.keyframes = this.keyframes
			.map((k) => (k.id === id ? { ...k, ...patch } : k))
			.sort((a, b) => a.time - b.time);
	}

	/** Move a keyframe in time, clamped to the track. */
	moveKeyframe(id: string, time: number) {
		this.updateKeyframe(id, {
			time: Math.max(0, Math.min(this.timelineDuration, Math.round(time))),
		});
	}

	removeKeyframe(id: string) {
		this.keyframes = this.keyframes.filter((k) => k.id !== id);
		if (this.selectedKeyframeId === id) this.selectedKeyframeId = null;
		if (this.keyframes.length === 0) this.playing = false;
	}

	clearKeyframes() {
		this.keyframes = [];
		this.selectedKeyframeId = null;
		this.playing = false;
		this.playhead = 0;
	}

	/** Select a keyframe: load its look into the live transform (so the controls
	 * and the paused stage show it) and move the playhead to its time. */
	selectKeyframe(id: string | null) {
		this.selectedKeyframeId = id;
		const kf = this.keyframes.find((k) => k.id === id);
		if (!kf) return;
		const p = kf.props;
		this.transform = {
			perspective: p.perspective,
			rotateX: p.rotateX,
			rotateY: p.rotateY,
			rotateZ: p.rotateZ,
			scale: p.scale,
			translateX: p.translateX,
			translateY: p.translateY,
		};
		this.imageOpacity = p.opacity;
		this.playhead = kf.time;
	}

	setKeyframeEasing(id: string, easing: Easing) {
		this.updateKeyframe(id, { easing });
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

	/** Reset all design fields to defaults. `keepAspect` preserves the chosen
	 * output ratio (used on image replace); the toolbar Reset clears it too. */
	resetDesign(keepAspect = false) {
		this.background = DEFAULT_BACKGROUND.background;
		this.backgroundId = DEFAULT_BACKGROUND.id;
		this.frame = structuredClone(DEFAULT_FRAME);
		this.shadow = { ...DEFAULT_SHADOW };
		this.shadowPreset = "soft";
		this.imageStyle = { ...DEFAULT_STYLE };
		this.mockup = { ...DEFAULT_MOCKUP };
		this.transform = { ...DEFAULT_TRANSFORM };
		if (!keepAspect) this.aspect = DEFAULT_ASPECT;
		this.overlays = [];
		this.selectedId = null;
		this.filters = { ...DEFAULT_FILTERS };
		this.imageScale = 100;
		this.imageOpacity = 1;
		this.backgroundBlur = 0;
		this.backgroundNoise = 0;
		this.canvasRadius = DEFAULT_CANVAS_RADIUS;
		this.keyframes = [];
		this.selectedKeyframeId = null;
		this.clearAnimation();
	}

	reset() {
		this.resetDesign(false);
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
			keyframes: this.keyframes,
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

	/** Apply a plain design object (from a history snapshot, preset, or draft). */
	private applyDesign(d: DesignObject) {
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
		this.canvasRadius = d.canvasRadius ?? DEFAULT_CANVAS_RADIUS;
		this.keyframes = d.keyframes ?? [];
		if (this.selectedKeyframeId && !this.keyframes.some((k) => k.id === this.selectedKeyframeId)) {
			this.selectedKeyframeId = null;
		}
		if (this.selectedId && !this.overlays.some((o: Overlay) => o.id === this.selectedId)) {
			this.selectedId = null;
		}
	}

	private restoreSnapshot(snap: string) {
		this._restoring = true;
		this.applyDesign(JSON.parse(snap));
		this._lastSnap = snap;
		this._lastAt = Date.now();
		this._restoring = false;
	}

	// --- Persistence: full snapshot (autosave) + design-only (custom presets) ---

	/** The current design as a plain object (no image), for a custom preset. */
	designObject(): DesignObject {
		return JSON.parse(this.serialize());
	}

	/** Apply a saved custom preset. Like the reference, this applies only the
	 * "look" (background/frame/shadow/style/mockup/3D/canvas/scale/opacity) and
	 * PRESERVES the user's overlays, filters, keyframes, and aspect ratio. */
	applyDesignObject(d: DesignObject) {
		this._restoring = true;
		this.background = d.background;
		this.backgroundId = d.backgroundId;
		this.frame = d.frame;
		this.shadow = d.shadow;
		this.shadowPreset = d.shadowPreset ?? "soft";
		this.imageStyle = d.imageStyle ?? { ...DEFAULT_STYLE };
		this.mockup = d.mockup;
		this.transform = d.transform;
		this.imageScale = d.imageScale ?? 100;
		this.imageOpacity = d.imageOpacity ?? 1;
		this.backgroundBlur = d.backgroundBlur ?? 0;
		this.backgroundNoise = d.backgroundNoise ?? 0;
		this.canvasRadius = d.canvasRadius ?? DEFAULT_CANVAS_RADIUS;
		this._lastSnap = this.serialize();
		this._lastAt = Date.now();
		this._restoring = false;
	}

	/** A full, serializable snapshot for the autosave draft (design + image). */
	toSnapshot(): EditorSnapshot {
		return {
			v: 1,
			image: this.image ? { ...this.image } : null,
			slides: this.slides.map((s) => ({ ...s })),
			activeSlide: this.activeSlide,
			design: this.designObject(),
			exportFormat: this.exportFormat,
			exportScale: this.exportScale,
			exportQuality: this.exportQuality,
		};
	}

	/** Restore a full snapshot (from a loaded draft). */
	loadSnapshot(snap: EditorSnapshot) {
		this._restoring = true;
		this.image = snap.image ?? null;
		this.slides = snap.slides ?? (snap.image ? [snap.image] : []);
		this.activeSlide = Math.min(snap.activeSlide ?? 0, Math.max(0, this.slides.length - 1));
		this.applyDesign(snap.design);
		this.exportFormat = snap.exportFormat ?? "png";
		this.exportScale = snap.exportScale ?? 2;
		this.exportQuality = snap.exportQuality ?? 0.95;
		this._lastSnap = this.serialize();
		this._lastAt = Date.now();
		this._restoring = false;
	}
}
