/**
 * Editor Store — Central reactive state for the video editor.
 * Uses Svelte 5 runes ($state) for granular reactivity.
 */

import type { CursorSampleLike } from '../cursor/smoothing';
import { EASE, type Easing } from '../easing/cubic-bezier';

export type BackgroundType = 'wallpaper' | 'image' | 'color' | 'gradient';


export interface WallpaperOption {
	/**
	 * Stable identifier — matches the `id` in `assets/manifest.json`. Stored
	 * in `backgroundValue` as `asset:<id>` so both preview and export can
	 * resolve against the downloaded cache.
	 */
	id: string;
	/** Small WebP preview (~3 KB). Bundled; used as the picker thumbnail and as
	 * the offline placeholder before the full-res download lands. */
	thumb: string;
	label: string;
}

/** Encode a wallpaper id as a `backgroundValue` string. */
export function wallpaperBackgroundValue(id: string): string {
	return `asset:${id}`;
}

export interface ZoomRegion {
	id: string;
	start: number; // seconds
	end: number; // seconds
	scale: number; // 1.0 - 3.0
	easeIn: Easing;
	easeOut: Easing;
	rampIn: number; // seconds spent ramping from 1.0 → scale
	rampOut: number; // seconds spent ramping from scale → 1.0
	centerX: number; // UV 0..1 — focus point X; 0.5 = center crop
	centerY: number; // UV 0..1 — focus point Y; 0.5 = center crop
	motionBlur: number; // 0..1 — preview motion-blur strength multiplier
}

export const DEFAULT_ZOOM_RAMP = 0.35;
export const DEFAULT_ZOOM_CENTER = 0.5;
export const DEFAULT_ZOOM_MOTION_BLUR = 0.5;

export interface ShadowSettings {
	enabled: boolean;
	blur: number; // px
	spread: number; // px
	offsetY: number; // px (positive = downward)
	opacity: number; // 0..100
	color: string; // hex
}

// ── Annotations ────────────────────────────────────────────────────────
//
// Position / size live in video UV space (0..1) so annotations follow zoom
// and crop transforms without re-projection. `kind` is a discriminated union
// so arrows / polygons / text / image slot in without churn later.

export interface AnnotationStroke {
	width: number; // UV
	color: string; // CSS colour
}

export type AnnotationKind =
	| {
			kind: "rect";
			x: number;
			y: number;
			w: number;
			h: number;
			radius: number; // UV corner radius; 0 = sharp
	  }
	| {
			kind: "ellipse";
			x: number; // UV bounding-box top-left
			y: number;
			w: number;
			h: number;
	  };

export type AnnotationKindName = AnnotationKind["kind"];

export interface Annotation {
	id: string;
	start: number; // seconds
	end: number; // seconds
	rampIn: number; // seconds fade-in
	rampOut: number; // seconds fade-out
	easeIn: Easing;
	easeOut: Easing;
	stroke: AnnotationStroke;
	fill: string; // CSS colour with alpha; "transparent" disables fill
	kind: AnnotationKind;
}

export const DEFAULT_ANNOTATION_RAMP = 0.2;
export const DEFAULT_ANNOTATION_STROKE: AnnotationStroke = {
	width: 0.004,
	color: "#3b82f6",
};
export const DEFAULT_ANNOTATION_FILL = "rgba(59,130,246,0.20)";

export interface CursorSettings {
	enabled: boolean;
	size: number; // 1-5 scale
	smoothing: number; // 0-100 → Gaussian σ in ms (0 = raw capture, 100 ≈ 150 ms)
	snapToClicks: boolean; // anchor smoothed path to exact click x/y around mouse-down
	snapWindowMs: number; // half-width (ms) of the snap anchor — 0..200
	highlightClicks: boolean;
	highlightColor: string;
	highlightOpacity: number; // 0-100
	hideWhenIdle: boolean;
	idleTimeout: number; // seconds
}

export interface BackgroundSelection {
	type: BackgroundType;
	value: string;
}

export interface AudioSettings {
	volume: number; // 0-100
	muted: boolean;
	fadeIn: number; // seconds
	fadeOut: number; // seconds
}

export type WatermarkPosition =
	| 'top-left'
	| 'top-right'
	| 'bottom-left'
	| 'bottom-right';

export interface WatermarkSettings {
	enabled: boolean;
	imagePath: string;
	imageSrc: string;
	opacity: number; // 0-100
	scale: number; // 8-35 percent of frame width
	position: WatermarkPosition;
	inset: number; // pixels
}

export interface VideoMetadata {
	duration: number;
	width: number;
	height: number;
	fps: number;
	codec: string;
	sizeBytes: number;
}

export interface EditorRenderState {
	trimStart: number;
	trimEnd: number;
	backgroundType: BackgroundType;
	backgroundValue: string;
	backgroundBlur: number;
	padding: number;
	borderRadius: number;
	cursorEnabled: boolean;
	cursorSize: number;
	cursorSmoothing: number;
	cursorSnapToClicks: boolean;
	cursorSnapWindowMs: number;
	cursorHighlightClicks: boolean;
	cursorHighlightColor: string;
	cursorHighlightOpacity: number;
	cursorHideWhenIdle: boolean;
	cursorIdleTimeout: number;
	zoomRegions: Array<{
		start: number;
		end: number;
		scale: number;
		easeIn: Easing;
		easeOut: Easing;
		rampIn: number;
		rampOut: number;
		centerX: number;
		centerY: number;
		motionBlur: number;
	}>;
	cursorMotionEasing: Easing | null;
	annotations: Array<Omit<Annotation, "id">>;
	shadow: ShadowSettings;
}

export type ExportFormat = 'mp4' | 'gif' | 'webm';
export type ExportQuality = 'small' | 'hd' | '4k' | 'source';

export type LayoutMode = 'auto' | 'crop';

export type EditorWindowBehavior = 'navigate' | 'new-window';

export type PanelTab = 'background' | 'focus' | 'annotations' | 'cursor' | 'audio';

export const WALLPAPERS: WallpaperOption[] = Array.from({ length: 23 }, (_, i) => ({
	id: `wallpaper${i + 1}`,
	thumb: `/backgrounds/thumbs/wallpaper${i + 1}.webp`,
	label: `Wallpaper ${i + 1}`,
}));

export const GRADIENT_PRESETS = [
	{ label: 'Sunset', value: 'linear-gradient(135deg, #f093fb 0%, #f5576c 100%)' },
	{ label: 'Ocean', value: 'linear-gradient(135deg, #4facfe 0%, #00f2fe 100%)' },
	{ label: 'Forest', value: 'linear-gradient(135deg, #43e97b 0%, #38f9d7 100%)' },
	{ label: 'Lavender', value: 'linear-gradient(135deg, #a18cd1 0%, #fbc2eb 100%)' },
	{ label: 'Midnight', value: 'linear-gradient(135deg, #0c3483 0%, #a2b6df 100%)' },
	{ label: 'Ember', value: 'linear-gradient(135deg, #f83600 0%, #f9d423 100%)' },
];

export const COLOR_PRESETS = [
	'#000000', '#1a1a2e', '#16213e', '#0f3460',
	'#533483', '#e94560', '#f38181', '#fce38a',
	'#eaffd0', '#95e1d3', '#ffffff', '#f5f5f5',
];

function generateId(): string {
	return Math.random().toString(36).substring(2, 9);
}

/**
 * Creates an editor store instance.
 * Call once per editor page mount, or use a singleton.
 */
export function createEditorStore() {
	// Video source
	let videoPath = $state('');
	let cursorPath = $state<string | null>(null);
	let metadata = $state<VideoMetadata | null>(null);
	let thumbnailStrip = $state<string[]>([]);

	// Playback
	let currentTime = $state(0);
	let isPlaying = $state(false);

	// Trim
	let trimStart = $state(0);
	let trimEnd = $state(0); // will be set to duration on load

	// Background
	let backgroundType = $state<BackgroundType>('wallpaper');
	let backgroundValue = $state(wallpaperBackgroundValue(WALLPAPERS[0].id));
	let backgroundBlur = $state(40);
	let padding = $state(32);
	let borderRadius = $state(0); // 0..50 (% of shorter video edge)

	// Drop shadow cast by the video rect onto the background.
	let shadow = $state<ShadowSettings>({
		enabled: false,
		blur: 40,
		spread: 0,
		offsetY: 24,
		opacity: 40,
		color: '#000000',
	});

	// Layout
	let layoutMode = $state<LayoutMode>('auto');

	// Raw cursor samples, shared between the preview (which runs the actual
	// compositor) and the Cursor panel (which needs them for the trajectory
	// minimap). Set by VideoPreview on load; read-only elsewhere.
	let cursorSamplesRaw = $state<CursorSampleLike[]>([]);

	// Annotations + active tool (for the preview canvas's place-mode).
	let annotations = $state<Annotation[]>([]);
	let selectedAnnotationId = $state<string | null>(null);
	let annotationTool = $state<AnnotationKindName | null>(null);

	// Zoom regions
	let zoomRegions = $state<ZoomRegion[]>([]);
	let selectedZoomRegionId = $state<string | null>(null);

	// Which properties-panel tab is active. Overlays (FocusOverlay,
	// AnnotationOverlay) gate their editing UI on this so users don't interact
	// with handles for a feature whose panel isn't visible.
	let activePanel = $state<PanelTab>('background');

	// Global cursor motion easing. `null` means linear (today's behaviour);
	// a non-null curve reshapes the per-sample lerp in the WebGL preview.
	let cursorMotionEasing = $state<Easing | null>(null);

	// Cursor settings
	let cursorSettings = $state<CursorSettings>({
		enabled: true,
		size: 3,
		smoothing: 50,
		snapToClicks: true,
		snapWindowMs: 80,
		highlightClicks: true,
		highlightColor: '#3b82f6',
		highlightOpacity: 40,
		hideWhenIdle: false,
		idleTimeout: 3,
	});

	// Audio settings
	let audioSettings = $state<AudioSettings>({
		volume: 100,
		muted: false,
		fadeIn: 0,
		fadeOut: 0,
	});

	// Watermark settings
	let watermarkSettings = $state<WatermarkSettings>({
		enabled: false,
		imagePath: '',
		imageSrc: '',
		opacity: 70,
		scale: 18,
		position: 'bottom-right',
		inset: 24,
	});

	// Export
	let exportFormat = $state<ExportFormat>('mp4');
	let exportQuality = $state<ExportQuality>('hd');
	let exportProgress = $state<number | null>(null);
	let isExporting = $state(false);

	// Undo/Redo stacks (simplified — stores snapshots of key settings)
	let undoStack = $state<string[]>([]);
	let redoStack = $state<string[]>([]);

	// Timeline zoom
	let timelineZoom = $state(1); // 1x = fit to width

	function getSettingsSnapshot(): string {
		return JSON.stringify({
			backgroundType,
			backgroundValue,
			backgroundBlur,
			padding,
			borderRadius,
			shadow,
			trimStart,
			trimEnd,
			zoomRegions,
			cursorSettings,
			audioSettings,
			watermarkSettings,
			layoutMode,
			cursorMotionEasing,
		});
	}

	const MAX_UNDO_HISTORY = 50;

	function pushUndoState() {
		undoStack = [...undoStack, getSettingsSnapshot()].slice(-MAX_UNDO_HISTORY);
		redoStack = [];
	}

	function undo() {
		if (undoStack.length === 0) return;
		const prev = undoStack[undoStack.length - 1];
		redoStack = [...redoStack, getSettingsSnapshot()];
		undoStack = undoStack.slice(0, -1);
		applySnapshot(prev);
	}

	function redo() {
		if (redoStack.length === 0) return;
		const next = redoStack[redoStack.length - 1];
		undoStack = [...undoStack, getSettingsSnapshot()];
		redoStack = redoStack.slice(0, -1);
		applySnapshot(next);
	}

	function applySnapshot(json: string) {
		const s = JSON.parse(json);
		backgroundType = s.backgroundType;
		backgroundValue = s.backgroundValue;
		backgroundBlur = s.backgroundBlur;
		padding = s.padding;
		borderRadius = s.borderRadius ?? 0;
		shadow = s.shadow ?? shadow;
		trimStart = s.trimStart;
		trimEnd = s.trimEnd;
		zoomRegions = (s.zoomRegions ?? []).map((r: ZoomRegion) => ({
			...r,
			centerX: r.centerX ?? DEFAULT_ZOOM_CENTER,
			centerY: r.centerY ?? DEFAULT_ZOOM_CENTER,
			motionBlur: r.motionBlur ?? DEFAULT_ZOOM_MOTION_BLUR,
		}));
		cursorSettings = s.cursorSettings;
		audioSettings = s.audioSettings ?? audioSettings;
		watermarkSettings = s.watermarkSettings ?? watermarkSettings;
		layoutMode = s.layoutMode;
		cursorMotionEasing = s.cursorMotionEasing ?? null;
	}

	function addZoomRegion(start: number, end: number, scale = 1.5) {
		pushUndoState();
		const region: ZoomRegion = {
			id: generateId(),
			start,
			end,
			scale,
			easeIn: { ...EASE },
			easeOut: { ...EASE },
			rampIn: DEFAULT_ZOOM_RAMP,
			rampOut: DEFAULT_ZOOM_RAMP,
			centerX: DEFAULT_ZOOM_CENTER,
			centerY: DEFAULT_ZOOM_CENTER,
			motionBlur: DEFAULT_ZOOM_MOTION_BLUR,
		};
		zoomRegions = [...zoomRegions, region];
		selectedZoomRegionId = region.id;
	}

	function setBackground(selection: BackgroundSelection) {
		const hasChanged =
			backgroundType !== selection.type || backgroundValue !== selection.value;
		if (!hasChanged) return;
		pushUndoState();
		backgroundType = selection.type;
		backgroundValue = selection.value;
	}

	function updateCursorSettings(updates: Partial<CursorSettings>) {
		cursorSettings = { ...cursorSettings, ...updates };
	}

	function updateAudioSettings(updates: Partial<AudioSettings>) {
		audioSettings = { ...audioSettings, ...updates };
	}

	function updateWatermarkSettings(updates: Partial<WatermarkSettings>) {
		watermarkSettings = { ...watermarkSettings, ...updates };
	}

	function updateShadow(updates: Partial<ShadowSettings>) {
		shadow = { ...shadow, ...updates };
	}

	function removeZoomRegion(id: string) {
		pushUndoState();
		zoomRegions = zoomRegions.filter((z) => z.id !== id);
		if (selectedZoomRegionId === id) selectedZoomRegionId = null;
	}

	function updateZoomRegion(id: string, updates: Partial<ZoomRegion>) {
		zoomRegions = zoomRegions.map((z) => (z.id === id ? { ...z, ...updates } : z));
	}

	function selectZoomRegion(id: string | null) {
		selectedZoomRegionId = id;
	}

	function addAnnotation(kind: AnnotationKind, start?: number, end?: number): Annotation {
		pushUndoState();
		const now = currentTime;
		const clipEnd = trimEnd || metadata?.duration || 0;
		const s = start ?? Math.max(trimStart, now);
		const e = end ?? Math.min(clipEnd, Math.max(s + 2.0, now + 2.0));
		const annotation: Annotation = {
			id: generateId(),
			start: s,
			end: e,
			rampIn: DEFAULT_ANNOTATION_RAMP,
			rampOut: DEFAULT_ANNOTATION_RAMP,
			easeIn: { ...EASE },
			easeOut: { ...EASE },
			stroke: { ...DEFAULT_ANNOTATION_STROKE },
			fill: DEFAULT_ANNOTATION_FILL,
			kind,
		};
		annotations = [...annotations, annotation];
		selectedAnnotationId = annotation.id;
		return annotation;
	}

	function updateAnnotation(id: string, updates: Partial<Annotation>) {
		annotations = annotations.map((a) => (a.id === id ? { ...a, ...updates } : a));
	}

	function removeAnnotation(id: string) {
		pushUndoState();
		annotations = annotations.filter((a) => a.id !== id);
		if (selectedAnnotationId === id) selectedAnnotationId = null;
	}

	function reset() {
		currentTime = 0;
		isPlaying = false;
		trimStart = 0;
		trimEnd = metadata?.duration ?? 0;
		backgroundType = 'wallpaper';
		backgroundValue = wallpaperBackgroundValue(WALLPAPERS[0].id);
		backgroundBlur = 40;
		padding = 32;
		borderRadius = 0;
		shadow = {
			enabled: false,
			blur: 40,
			spread: 0,
			offsetY: 24,
			opacity: 40,
			color: '#000000',
		};
		layoutMode = 'auto';
		zoomRegions = [];
		selectedZoomRegionId = null;
		annotations = [];
		selectedAnnotationId = null;
		annotationTool = null;
		cursorMotionEasing = null;
		cursorSettings = {
			enabled: true,
			size: 3,
			smoothing: 50,
			snapToClicks: true,
			snapWindowMs: 80,
			highlightClicks: true,
			highlightColor: '#3b82f6',
			highlightOpacity: 40,
			hideWhenIdle: false,
			idleTimeout: 3,
		};
		audioSettings = {
			volume: 100,
			muted: false,
			fadeIn: 0,
			fadeOut: 0,
		};
		watermarkSettings = {
			enabled: false,
			imagePath: '',
			imageSrc: '',
			opacity: 70,
			scale: 18,
			position: 'bottom-right',
			inset: 24,
		};
		exportQuality = 'hd';
		undoStack = [];
		redoStack = [];
	}

	function toRenderState(): EditorRenderState {
		return {
			trimStart,
			trimEnd,
			backgroundType,
			backgroundValue,
			backgroundBlur,
			padding,
			borderRadius,
			cursorEnabled: cursorSettings.enabled,
			cursorSize: cursorSettings.size,
			cursorSmoothing: cursorSettings.smoothing,
			cursorSnapToClicks: cursorSettings.snapToClicks,
			cursorSnapWindowMs: cursorSettings.snapWindowMs,
			cursorHighlightClicks: cursorSettings.highlightClicks,
			cursorHighlightColor: cursorSettings.highlightColor,
			cursorHighlightOpacity: cursorSettings.highlightOpacity,
			cursorHideWhenIdle: cursorSettings.hideWhenIdle,
			cursorIdleTimeout: cursorSettings.idleTimeout,
			zoomRegions: zoomRegions.map((region) => ({
				start: region.start,
				end: region.end,
				scale: region.scale,
				easeIn: region.easeIn,
				easeOut: region.easeOut,
				rampIn: region.rampIn,
				rampOut: region.rampOut,
				centerX: region.centerX,
				centerY: region.centerY,
				motionBlur: region.motionBlur,
			})),
			cursorMotionEasing,
			annotations: annotations.map(({ id: _id, ...rest }) => rest),
			shadow: { ...shadow },
		};
	}

	function loadRenderState(state: Partial<EditorRenderState>) {
		trimStart = state.trimStart ?? 0;
		trimEnd = state.trimEnd ?? metadata?.duration ?? 0;
		backgroundType = state.backgroundType ?? 'color';
		backgroundValue = state.backgroundValue ?? '#111111';
		backgroundBlur = state.backgroundBlur ?? 0;
		padding = state.padding ?? 0;
		borderRadius = state.borderRadius ?? 0;
		cursorSettings = {
			...cursorSettings,
			enabled: state.cursorEnabled ?? cursorSettings.enabled,
			size: state.cursorSize ?? cursorSettings.size,
			smoothing: state.cursorSmoothing ?? cursorSettings.smoothing,
			snapToClicks: state.cursorSnapToClicks ?? cursorSettings.snapToClicks,
			snapWindowMs: state.cursorSnapWindowMs ?? cursorSettings.snapWindowMs,
			highlightClicks:
				state.cursorHighlightClicks ?? cursorSettings.highlightClicks,
			highlightColor:
				state.cursorHighlightColor ?? cursorSettings.highlightColor,
			highlightOpacity:
				state.cursorHighlightOpacity ?? cursorSettings.highlightOpacity,
			hideWhenIdle:
				state.cursorHideWhenIdle ?? cursorSettings.hideWhenIdle,
			idleTimeout:
				state.cursorIdleTimeout ?? cursorSettings.idleTimeout,
		};
		zoomRegions = (state.zoomRegions ?? []).map((region) => ({
			id: generateId(),
			start: region.start,
			end: region.end,
			scale: region.scale,
			easeIn: region.easeIn ?? { ...EASE },
			easeOut: region.easeOut ?? { ...EASE },
			rampIn: region.rampIn ?? DEFAULT_ZOOM_RAMP,
			rampOut: region.rampOut ?? DEFAULT_ZOOM_RAMP,
			centerX: region.centerX ?? DEFAULT_ZOOM_CENTER,
			centerY: region.centerY ?? DEFAULT_ZOOM_CENTER,
			motionBlur: region.motionBlur ?? DEFAULT_ZOOM_MOTION_BLUR,
		}));
		shadow = state.shadow ?? shadow;
		cursorMotionEasing = state.cursorMotionEasing ?? null;
		annotations = (state.annotations ?? []).map((a) => ({
			id: generateId(),
			start: a.start,
			end: a.end,
			rampIn: a.rampIn ?? DEFAULT_ANNOTATION_RAMP,
			rampOut: a.rampOut ?? DEFAULT_ANNOTATION_RAMP,
			easeIn: a.easeIn ?? { ...EASE },
			easeOut: a.easeOut ?? { ...EASE },
			stroke: a.stroke ?? { ...DEFAULT_ANNOTATION_STROKE },
			fill: a.fill ?? DEFAULT_ANNOTATION_FILL,
			kind: a.kind,
		}));
		selectedAnnotationId = null;
		annotationTool = null;
	}

	return {
		// Getters (reactive reads)
		get videoPath() { return videoPath; },
		set videoPath(v: string) { videoPath = v; },

		get cursorPath() { return cursorPath; },
		set cursorPath(v: string | null) { cursorPath = v; },

		get metadata() { return metadata; },
		set metadata(v: VideoMetadata | null) { metadata = v; },

		get thumbnailStrip() { return thumbnailStrip; },
		set thumbnailStrip(v: string[]) { thumbnailStrip = v; },

		get currentTime() { return currentTime; },
		set currentTime(v: number) { currentTime = v; },

		get isPlaying() { return isPlaying; },
		set isPlaying(v: boolean) { isPlaying = v; },

		get trimStart() { return trimStart; },
		set trimStart(v: number) { pushUndoState(); trimStart = v; },

		get trimEnd() { return trimEnd; },
		set trimEnd(v: number) { pushUndoState(); trimEnd = v; },

		get backgroundType() { return backgroundType; },
		set backgroundType(v: BackgroundType) { pushUndoState(); backgroundType = v; },

		get backgroundValue() { return backgroundValue; },
		set backgroundValue(v: string) { pushUndoState(); backgroundValue = v; },

		get backgroundBlur() { return backgroundBlur; },
		set backgroundBlur(v: number) { backgroundBlur = v; },

		get padding() { return padding; },
		set padding(v: number) { padding = v; },

		get borderRadius() { return borderRadius; },
		set borderRadius(v: number) { borderRadius = v; },

		get shadow() { return shadow; },
		set shadow(v: ShadowSettings) { shadow = v; },

		get layoutMode() { return layoutMode; },
		set layoutMode(v: LayoutMode) { pushUndoState(); layoutMode = v; },

		get zoomRegions() { return zoomRegions; },

		get cursorSamplesRaw() { return cursorSamplesRaw; },
		set cursorSamplesRaw(v: CursorSampleLike[]) { cursorSamplesRaw = v; },

		get selectedZoomRegionId() { return selectedZoomRegionId; },
		set selectedZoomRegionId(v: string | null) { selectedZoomRegionId = v; },

		get activePanel() { return activePanel; },
		set activePanel(v: PanelTab) { activePanel = v; },

		get cursorMotionEasing() { return cursorMotionEasing; },
		set cursorMotionEasing(v: Easing | null) { pushUndoState(); cursorMotionEasing = v; },

		get annotations() { return annotations; },
		get selectedAnnotationId() { return selectedAnnotationId; },
		set selectedAnnotationId(v: string | null) { selectedAnnotationId = v; },
		get annotationTool() { return annotationTool; },
		set annotationTool(v: AnnotationKindName | null) { annotationTool = v; },

		get cursorSettings() { return cursorSettings; },
		set cursorSettings(v: CursorSettings) { cursorSettings = v; },

		get audioSettings() { return audioSettings; },
		set audioSettings(v: AudioSettings) { audioSettings = v; },

		get watermarkSettings() { return watermarkSettings; },
		set watermarkSettings(v: WatermarkSettings) { watermarkSettings = v; },

		get exportFormat() { return exportFormat; },
		set exportFormat(v: ExportFormat) { exportFormat = v; },

		get exportQuality() { return exportQuality; },
		set exportQuality(v: ExportQuality) { exportQuality = v; },

		get exportProgress() { return exportProgress; },
		set exportProgress(v: number | null) { exportProgress = v; },

		get isExporting() { return isExporting; },
		set isExporting(v: boolean) { isExporting = v; },

		get timelineZoom() { return timelineZoom; },
		set timelineZoom(v: number) { timelineZoom = v; },

		get canUndo() { return undoStack.length > 0; },
		get canRedo() { return redoStack.length > 0; },

		// Methods
		undo,
		redo,
		pushUndoState,
		setBackground,
		updateCursorSettings,
		updateAudioSettings,
		updateWatermarkSettings,
		updateShadow,
		addZoomRegion,
		removeZoomRegion,
		updateZoomRegion,
		selectZoomRegion,
		addAnnotation,
		updateAnnotation,
		removeAnnotation,
		reset,
		toRenderState,
		loadRenderState,
	};
}

export type EditorStore = ReturnType<typeof createEditorStore>;
