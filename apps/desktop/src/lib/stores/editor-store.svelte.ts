/**
 * Editor Store — Central reactive state for the video editor.
 * Uses Svelte 5 runes ($state) for granular reactivity.
 */

import type { CursorSampleLike } from '../cursor/smoothing';
import { EASE, type Easing } from '../easing/cubic-bezier';
import { totalCutDuration, type CutSource, type TimelineCut } from '../timeline/cuts';

export type BackgroundType = 'wallpaper' | 'image' | 'color' | 'gradient';


export interface WallpaperOption {
	/**
	 * Stable identifier — matches the `id` in `assets/manifest.json`. Stored
	 * in `backgroundValue` as `asset:<id>` so both preview and export can
	 * resolve against the downloaded cache. No bundled thumbnail — the
	 * LazyExternalImage component reads thumbs from the same downloaded
	 * cache, with a CSS placeholder while nothing is available yet.
	 */
	id: string;
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
	/**
	 * Origin of the region. "auto" means added by Smart Auto-Zoom on first
	 * load; flipped to "manual" the moment the user edits any field so
	 * "Clear auto zooms" leaves their tweaks alone.
	 */
	source: "manual" | "auto";
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

//  Annotations 
//
// Position / size live in video UV space (0..1) so annotations follow zoom
// and crop transforms without re-projection. `kind` is a discriminated union
// so arrows / polygons / text / image slot in without churn later.

export type AnnotationStrokeStyle = "solid" | "dashed" | "dotted";

export interface AnnotationStroke {
	width: number; // UV
	color: string; // CSS colour
	/** Stroke pattern. Defaults to "solid" for v1 projects (`undefined` ↔ solid). */
	style?: AnnotationStrokeStyle;
}

/**
 * Optional preview-only glow / shadow. Renders in the editor but **not** in
 * the Rust export pipeline yet — the Annotations tab surfaces this trade-off
 * with a banner so the user is never surprised at export time.
 */
export interface AnnotationGlow {
	color: string;
	/** Blur radius in UV (≈ 0..0.05 ≈ 0..27 px at 1080p). */
	blur: number;
	opacity: number; // 0..1
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
	}
	| {
		kind: "arrow";
		// Endpoints in UV; the arrow head is drawn at (x2, y2).
		x1: number;
		y1: number;
		x2: number;
		y2: number;
		/** Head length as a fraction of line length (0.05–0.4). */
		headSize: number;
	}
	| {
		// Text overlays render in the WebView only and are rasterized to a
		// PNG (kind=image) at export time. They never reach the Rust enum.
		kind: "text";
		x: number; // UV top-left of bounding box
		y: number;
		w: number;
		h: number;
		content: string;
		fontFamily: string; // CSS family name; whitelisted in TextProps
		/** Font size as a fraction of canvas height (0.02–0.20). */
		fontSize: number;
		fontWeight: 400 | 500 | 600 | 700;
		color: string; // CSS colour
		align: "left" | "center" | "right";
		/** Multiplier on font size; default 1.2. */
		lineHeight: number;
	}
	| {
		// Generic image overlay: a PNG/JPG composited at the UV rect.
		// Used both for the (deferred) Image tool and as the export
		// substitute for text annotations after hybrid rasterization.
		kind: "image";
		x: number;
		y: number;
		w: number;
		h: number;
		path: string; // absolute file path or asset URL
		opacity: number; // 0..1
	}
	| {
		// Privacy / focus blur. Applies a box blur (separable, kernel
		// proportional to `strength`) over the bounding rect, optionally
		// tinted by `variant`. `glass` = clear blur, white/black tint at
		// 30% over the blurred pixels, `color` = `tintColor` at 30%.
		kind: "blur";
		x: number;
		y: number;
		w: number;
		h: number;
		/** Blur strength 0..1 — maps to a box-blur radius up to ~5% of the canvas. */
		strength: number;
		/** Tint mode applied over the blurred pixels. */
		variant: "glass" | "white" | "black" | "color";
		/** Tint colour used when `variant === "color"`. CSS `#rrggbb`. */
		tintColor: string;
		/** Corner rounding in UV space. 0 = sharp. */
		radius: number;
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

	// v2 envelope. Every field is optional; absence = v1 default. The render
	// path reads these via `??` defaults so older projects keep loading.
	/** User-renamed label. Falls back to `kindLabel(a)` when empty. */
	name?: string;
	/** Stacking order; higher draws later (on top). Default = insertion order
	 *  (assigned at creation, monotonically increasing). */
	zIndex?: number;
	/** When true, canvas pointer events ignore this annotation. */
	locked?: boolean;
	/** When true, the annotation is skipped at draw time entirely. */
	hidden?: boolean;
	/** Master opacity 0..1; multiplied with the split-ramp opacity. */
	opacity?: number;
	/** Optional glow / soft shadow. Preview only in v2 (Rust glow follows). */
	glow?: AnnotationGlow;
}

export const DEFAULT_ANNOTATION_RAMP = 0.2;
export const DEFAULT_ANNOTATION_STROKE: AnnotationStroke = {
	width: 0.004,
	color: "#3b82f6",
};
export const DEFAULT_ANNOTATION_FILL = "rgba(59,130,246,0.20)";

/**
 * Cursor style id — matches an entry in `lib/cursor/styles.ts`.
 *  - `dot`: the legacy soft white circle (rendered by the WebGL2 shader and
 *    the Rust export overlay; what users see by default).
 *  - Anything else: an SVG cursor sprite drawn by `CursorOverlayLayer` over
 *    the preview. Preview-only today; export currently falls back to `dot`.
 */
export type CursorStyleId = 'dot' | 'macos' | 'windows' | 'outline' | 'target';

export interface CursorSettings {
	enabled: boolean;
	size: number; // 1-5 scale
	style: CursorStyleId;
	smoothing: number; // 0-100 → Gaussian σ in ms (0 = raw capture, 100 ≈ 150 ms)
	snapToClicks: boolean; // anchor smoothed path to exact click x/y around mouse-down
	snapWindowMs: number; // half-width (ms) of the snap anchor — 0..200
	highlightClicks: boolean;
	highlightColor: string;
	highlightOpacity: number; // 0-100
	hideWhenIdle: boolean;
	idleTimeout: number; // seconds
	/** Motion-blur strength — 0 = off, 1 = strong velocity trail. */
	motionBlur: number;
	/** Click-bounce amplitude — 0 = no bounce, 5 = exaggerated squash. */
	clickBounce: number;
	/** Bounce/squash duration in ms. */
	bounceSpeedMs: number;
	/** Idle sway amplitude — subtle wobble during slow motion. 0 = off, 1 = max. */
	sway: number;
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

export type CameraOverlayShape = 'square' | 'rectangle' | 'rounded' | 'circle';
export type CameraOverlayAnimationPreset = 'none' | 'soft' | 'lively';
export type CameraMotionSource = 'live-recorded' | 'manual';

export interface CameraPlacement {
	x: number;
	y: number;
	width: number;
	height: number;
}

export interface CameraMotionSegment {
	start: number;
	end: number;
	fromX: number;
	fromY: number;
	fromWidth: number;
	fromHeight: number;
	toX: number;
	toY: number;
	toWidth: number;
	toHeight: number;
	easeIn: Easing;
	easeOut: Easing;
	source?: CameraMotionSource;
}

export interface CameraOverlaySettings {
	enabled: boolean;
	mirror: boolean;
	shape: CameraOverlayShape;
	cornerRadius: number;
	animationPreset: CameraOverlayAnimationPreset;
	defaultPlacement: CameraPlacement;
	motionSegments: CameraMotionSegment[];
}

/**
 * The 8 standard camera-bubble positions plus `custom` for free-drag.
 * Used by `CameraPanel` for the preset chip row, and by
 * `cameraPresetFromPlacement` to identify which chip should be highlighted
 * after a drag-snap.
 */
export type CameraPositionPreset =
	| 'top-left' | 'top-center' | 'top-right'
	| 'left-center' | 'right-center'
	| 'bottom-left' | 'bottom-center' | 'bottom-right'
	| 'custom';

/** Default size (16% of frame) and inset (2% margin) for preset placements. */
export const CAMERA_DEFAULT_SIZE = 0.16;
export const CAMERA_PRESET_INSET = 0.02;

/**
 * Resolve a preset name to a normalized {x, y, width, height}. The bubble
 * is square by default (Phase 1 ships rounded 1:1 only); width == height.
 * x/y are the top-left corner of the bubble in 0..1 UV.
 *
 * `custom` returns the current bottom-right placement as a sane fallback —
 * the panel never actually invokes this with `custom`; that branch exists
 * so callers don't have to special-case the union.
 */
export function cameraPlacementFromPreset(
	preset: CameraPositionPreset,
	size: number = CAMERA_DEFAULT_SIZE,
	inset: number = CAMERA_PRESET_INSET,
): CameraPlacement {
	const near = inset;
	const far = 1 - size - inset;
	const center = (1 - size) / 2;
	const xByCol: Record<string, number> = { left: near, center, right: far };
	const yByRow: Record<string, number> = { top: near, center, bottom: far };
	if (preset === 'custom') {
		return { x: far, y: far, width: size, height: size };
	}
	const [row, col] = preset.split('-') as [string, string];
	return {
		x: xByCol[col] ?? far,
		y: yByRow[row] ?? far,
		width: size,
		height: size,
	};
}

/**
 * Inverse of `cameraPlacementFromPreset` — find which preset (if any) the
 * given placement matches within a 0.5% tolerance. Returns `custom` for
 * free-drag positions. Used by the panel to highlight the active chip.
 */
export function cameraPresetFromPlacement(p: CameraPlacement): CameraPositionPreset {
	const presets: CameraPositionPreset[] = [
		'top-left', 'top-center', 'top-right',
		'left-center', 'right-center',
		'bottom-left', 'bottom-center', 'bottom-right',
	];
	const tolerance = 0.005;
	for (const preset of presets) {
		const ref = cameraPlacementFromPreset(preset, p.width);
		if (
			Math.abs(p.x - ref.x) < tolerance &&
			Math.abs(p.y - ref.y) < tolerance
		) {
			return preset;
		}
	}
	return 'custom';
}

export interface VideoMetadata {
	duration: number;
	width: number;
	height: number;
	fps: number;
	codec: string;
	sizeBytes: number;
}

export const MAX_FRAME_PADDING_PERCENT = 20;

export function clampFramePaddingPercent(value: number): number {
	if (!Number.isFinite(value)) return 0;
	return Math.max(0, Math.min(MAX_FRAME_PADDING_PERCENT, value));
}

export function framePaddingPixels(
	paddingPercent: number,
	metadata: Pick<VideoMetadata, 'width' | 'height'> | null | undefined,
): number {
	if (!metadata?.width || !metadata?.height) return 0;
	const shorterEdge = Math.min(metadata.width, metadata.height);
	const pct = clampFramePaddingPercent(paddingPercent);
	return (pct / 100) * shorterEdge;
}

export function normalizeFramePaddingPercent(
	value: number,
	metadata: Pick<VideoMetadata, 'width' | 'height'> | null | undefined,
): number {
	if (!Number.isFinite(value)) return 0;
	const nonNegative = Math.max(0, value);
	if (nonNegative <= MAX_FRAME_PADDING_PERCENT) {
		return clampFramePaddingPercent(nonNegative);
	}
	// Legacy projects stored padding as raw pixels.
	if (metadata?.width && metadata?.height) {
		const shorterEdge = Math.min(metadata.width, metadata.height);
		if (shorterEdge > 0) {
			return clampFramePaddingPercent((nonNegative / shorterEdge) * 100);
		}
	}
	return 0;
}

export interface EditorRenderState {
	trimStart: number;
	trimEnd: number;
	/**
	 * Final-canvas aspect. Optional/absent = 'source' (the v1 default), so
	 * older project files keep loading. The Rust pipeline letterboxes the
	 * source-plus-padding inside this canvas via the chosen background.
	 */
	outputAspect?: OutputAspect;
	/**
	 * Id of the most recently applied preset (matches `Preset.id` in
	 * `PresetPicker.svelte`). Display-only; the actual canvas/background
	 * effects are stored in the individual fields above.
	 */
	lastAppliedPresetId?: string | null;
	backgroundType: BackgroundType;
	backgroundValue: string;
	backgroundBlur: number;
	/** Frame padding as percent of the shorter source edge (0..20). */
	padding: number;
	borderRadius: number;
	cursorEnabled: boolean;
	cursorSize: number;
	/**
	 * User-picked cursor sprite style (`dot` / `macos` / `windows` /
	 * `outline` / `target`). Optional for backwards compatibility — projects
	 * saved before this field landed default to `dot` on load.
	 */
	cursorStyle?: CursorStyleId;
	cursorSmoothing: number;
	cursorSnapToClicks: boolean;
	cursorSnapWindowMs: number;
	cursorHighlightClicks: boolean;
	cursorHighlightColor: string;
	cursorHighlightOpacity: number;
	cursorHideWhenIdle: boolean;
	cursorIdleTimeout: number;
	cursorMotionBlur: number;
	cursorClickBounce: number;
	cursorBounceSpeedMs: number;
	cursorSway: number;
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
		source?: "manual" | "auto";
	}>;
	autoZoomApplied?: boolean;
	autoZoomEnabled?: boolean;
	/** Silence / manual cuts removed from the timeline. */
	cuts?: TimelineCut[];
	cursorMotionEasing: Easing | null;
	annotations: Array<Omit<Annotation, "id">>;
	shadow: ShadowSettings;
	audioSettings: AudioSettings;
	watermarkSettings: WatermarkSettings;
	cameraOverlay: CameraOverlaySettings;
	/**
	 * Editor layout mode (`auto` / column-stacked variants etc.). Optional
	 * for backwards compatibility — pre-field projects keep their default.
	 */
	layoutMode?: LayoutMode;
	// Hybrid-raster cursor sprite — populated only on the export path
	// (the editor route runs `rasterizeCursorSprites` right before invoking
	// `export_video`). Not persisted to disk; never set by `loadRenderState`.
	cursorSpriteRest?: string;          // data:image/png;base64,…
	cursorSpritePress?: string;         // optional; falls back to rest in Rust
	cursorSpriteHotspotRest?: [number, number];   // 0..1 sprite UV
	cursorSpriteHotspotPress?: [number, number];
	cursorSpriteSizePx?: number;        // sprite render size in source pixels
}

export type ExportFormat = 'mp4' | 'gif' | 'webm';
export type ExportQuality = 'small' | 'hd' | '4k' | 'source';

/** GIF dithering algorithm. Trades file size against gradient quality. */
export type GifDither = 'bayer' | 'sierra2' | 'none';
/** GIF quality preset — controls palette size + dither bias. */
export type GifQuality = 'low' | 'medium' | 'high';
/** GIF loop behavior. `infinite` writes Netscape loop=0, `once` writes loop=-1, `n` writes loop=n. */
export type GifLoop = 'infinite' | 'once' | number;

export interface GifSettings {
	/** Output frame rate. `null` = inherit from quality profile. */
	fps: number | null;
	quality: GifQuality;
	loop: GifLoop;
	dither: GifDither;
}

export const DEFAULT_GIF_SETTINGS: GifSettings = {
	fps: null,
	quality: 'medium',
	loop: 'infinite',
	dither: 'bayer',
};

export type LayoutMode = 'auto' | 'crop';

/**
 * Final-canvas aspect ratio. `source` keeps the canvas matched to the
 * source video plus padding (the v1 behaviour). The other values reframe
 * the final canvas to a target ratio — the source video stays centred,
 * and the chosen background fills the new horizontal/vertical bars.
 *
 * Strings are kept human-readable so they round-trip through the preset
 * picker (`preset.aspect`) and the project JSON without translation.
 */
export type OutputAspect = 'source' | '16:9' | '9:16' | '1:1' | '1.91:1';

/** Parse an OutputAspect to a width/height ratio. Returns null for `source`. */
export function aspectRatio(a: OutputAspect): number | null {
	switch (a) {
		case 'source':
			return null;
		case '16:9':
			return 16 / 9;
		case '9:16':
			return 9 / 16;
		case '1:1':
			return 1;
		case '1.91:1':
			return 1.91;
	}
}

export type EditorWindowBehavior = 'navigate' | 'new-window';

export type PanelTab = 'background' | 'focus' | 'annotations' | 'cursor' | 'camera' | 'audio' | 'info';

export const WALLPAPERS: WallpaperOption[] = Array.from({ length: 23 }, (_, i) => ({
	id: `wallpaper${i + 1}`,
	label: `Wallpaper ${i + 1}`,
}));

const rawGradientPresets: [string, string[]][] = [
	['Forest', ["#80dbef", "#51b2b9", "#2f8d87", "#216a57", "#134830"]],
	['Sunset', ["#dcb2b3", "#e9c6be", "#dfad90", "#e9b89e", "#dcc2a8"]],
	['Ocean', ["#24707e", "#3d9886", "#6ba072", "#9fbb7e", "#c0d09a"]],
	['Lavender', ["#facac2", "#dca0aa", "#ad667d", "#7b3f62", "#461a48"]],
	['Beige',["#d5ac9c", "#e6bfa6", "#f1d2ae", "#d0ae85", "#cebc8c"]],
	['Midnight', ["#cbb8f3", "#9892c9", "#7664ae", "#4d417e", "#2d204b"]],
	['Ember', ["#f8b3c9", "#f3ac9e", "#f1bf9f", "#f5d4a4", "#d4d294"]],
	['Sky', ["#bbd2f9", "#99aed1", "#7b8aac", "#4f699c", "#3a4877"]],
]

export const GRADIENT_PRESETS = rawGradientPresets.map(([label, colors]) => ({
	label,
	value: `linear-gradient(135deg, ${colors.map((c, i) => `${c} ${(i / (colors.length - 1)) * 100}%`).join(', ')})`,
}))

export const COLOR_PRESETS = [
	'#eaffd0', '#95e1d3', '#ffffff', '#f5f5f5',
	'#533483', '#e94560', '#f38181', '#fce38a',
	'#0f3460', '#16213e', '#1a1a2e', '#000000',
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
	// Raw on-disk media paths (the extracted recording / audio tracks), needed
	// by Rust-side analysis commands such as silence detection.
	let recordingPath = $state<string | null>(null);
	let audioPath = $state<string | null>(null);
	let microphonePath = $state<string | null>(null);
	let metadata = $state<VideoMetadata | null>(null);
	let thumbnailStrip = $state<string[]>([]);

	// Playback
	let currentTime = $state(0);
	let isPlaying = $state(false);

	// Trim
	let trimStart = $state(0);
	let trimEnd = $state(0); // will be set to duration on load

	// Silence / manual cuts — removed ranges, in original-recording seconds.
	let cuts = $state<TimelineCut[]>([]);

	// Background
	let backgroundType = $state<BackgroundType>('wallpaper');
	let backgroundValue = $state(wallpaperBackgroundValue(WALLPAPERS[0].id));
	let backgroundBlur = $state(40);
	let padding = $state(3);
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

	// Final-canvas aspect. `source` means "follow the input video"; any other
	// value reframes the canvas via letterbox/pillarbox bars. The preset
	// picker writes this when the user picks an Instagram/YouTube/X preset.
	let outputAspect = $state<OutputAspect>('source');

	// Id of the most recently applied preset. Pure UI affordance — lets the
	// toolbar surface "Story · 9:16" so users see, per project, what's in
	// effect. Cleared when the user resets back to source.
	let lastAppliedPresetId = $state<string | null>(null);

	// Raw cursor samples, shared between the preview (which runs the actual
	// compositor) and the Cursor panel (which needs them for the trajectory
	// minimap). Set by VideoPreview on load; read-only elsewhere.
	let cursorSamplesRaw = $state<CursorSampleLike[]>([]);

	// Annotations + active tool (for the preview canvas's place-mode).
	let annotations = $state<Annotation[]>([]);
	let selectedAnnotationId = $state<string | null>(null);
	let annotationTool = $state<AnnotationKindName | null>(null);
	// Layer-panel hover state — when set, the overlay flashes the matching
	// annotation so users can find a layer in a busy frame.
	let hoveredAnnotationId = $state<string | null>(null);
	// Master visibility toggle (the status rail's eye icon). Independent of
	// per-annotation `hidden` so it can flip without trampling user state.
	let annotationsGloballyHidden = $state<boolean>(false);
	// Snap engine on/off. Default on. Alt held during drag bypasses regardless.
	let annotationSnapEnabled = $state<boolean>(true);
	// Monotonic z-index counter so newly created annotations always start
	// above existing ones and ordering survives reorder operations.
	let annotationZSeq = 1;

	// Zoom regions
	let zoomRegions = $state<ZoomRegion[]>([]);
	let selectedZoomRegionId = $state<string | null>(null);
	// Smart Auto-Zoom: persisted per-project so we only auto-apply on the
	// first editor load. `enabled` is the user's preference; `applied` is
	// the latch that prevents re-running on every reopen.
	let autoZoomEnabled = $state(true);
	let autoZoomApplied = $state(false);

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
		size: 2,
		style: 'dot',
		smoothing: 50,
		snapToClicks: true,
		snapWindowMs: 80,
		highlightClicks: true,
		highlightColor: '#3b82f6',
		highlightOpacity: 40,
		hideWhenIdle: false,
		idleTimeout: 3,
		motionBlur: 0,
		clickBounce: 0,
		bounceSpeedMs: 220,
		sway: 0,
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
	// Camera overlay defaults — Phase 1 spec:
	// - Bottom-right corner at 16% size, 1:1 aspect
	// - Rounded shape (16% corner radius)
	// - Mirrored on (matches the recording-time webcam preview)
	// - Soft drop shadow (applied unconditionally in the overlay component)
	// Position uses normalized 0..1 UV so it survives output-aspect changes.
	let cameraOverlay = $state<CameraOverlaySettings>({
		enabled: false,
		mirror: true,
		shape: 'rounded',
		cornerRadius: 0.16,
		animationPreset: 'soft',
		defaultPlacement: cameraPlacementFromPreset('bottom-right'),
		motionSegments: [],
	});

	// Export
	let exportFormat = $state<ExportFormat>('mp4');
	let exportQuality = $state<ExportQuality>('hd');
	let gifSettings = $state<GifSettings>({ ...DEFAULT_GIF_SETTINGS });
	let exportProgress = $state<number | null>(null);
	let isExporting = $state(false);

	// Undo/Redo stacks (simplified — stores snapshots of key settings)
	let undoStack = $state<string[]>([]);
	let redoStack = $state<string[]>([]);

	// Dirty tracking — flips to true the moment the user makes any undoable edit,
	// clears when the edits are persisted to the .recast archive (markSaved) or
	// when a fresh render state is loaded from disk.
	let isDirty = $state(false);
	let lastSavedAt = $state<number | null>(null);

	// Timeline zoom
	let timelineZoom = $state(1); // 1x = fit to width

	function getSettingsSnapshot(): string {
		// Captures every undoable state field. Anything left out here
		// silently survives a `pushUndoState` call but isn't restored on
		// undo — the user sees the unrelated edits revert while their
		// annotation/camera tweak stays put. Keep this list in sync with
		// `applySnapshot` so they round-trip 1:1.
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
			cuts,
			autoZoomEnabled,
			autoZoomApplied,
			annotations,
			cursorSettings,
			audioSettings,
			watermarkSettings,
			cameraOverlay,
			layoutMode,
			outputAspect,
			lastAppliedPresetId,
			cursorMotionEasing,
		});
	}

	const MAX_UNDO_HISTORY = 50;

	function pushUndoState() {
		undoStack = [...undoStack, getSettingsSnapshot()].slice(-MAX_UNDO_HISTORY);
		redoStack = [];
		isDirty = true;
	}

	// Coalesced undo: a sequence of small edits that share the same `key`
	// inside `ttlMs` of each other becomes a single undo entry. Used for
	// keyboard nudges (e.g. holding ArrowLeft on a trim handle) so a
	// 30-frame walk-back is one Ctrl+Z press, not thirty.
	let lastCoalesceKey: string | null = null;
	let lastCoalesceAt = 0;
	function pushUndoStateCoalesced(key: string, ttlMs = 500) {
		const now =
			typeof performance !== "undefined" ? performance.now() : Date.now();
		if (lastCoalesceKey === key && now - lastCoalesceAt < ttlMs) {
			lastCoalesceAt = now;
			isDirty = true;
			return;
		}
		lastCoalesceKey = key;
		lastCoalesceAt = now;
		pushUndoState();
	}

	function markSaved(savedAtUnixMs: number) {
		isDirty = false;
		lastSavedAt = savedAtUnixMs;
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
		padding = normalizeFramePaddingPercent(s.padding, metadata);
		borderRadius = s.borderRadius ?? 0;
		shadow = s.shadow ?? shadow;
		trimStart = s.trimStart;
		trimEnd = s.trimEnd;
		zoomRegions = (s.zoomRegions ?? []).map((r: ZoomRegion) => ({
			...r,
			centerX: r.centerX ?? DEFAULT_ZOOM_CENTER,
			centerY: r.centerY ?? DEFAULT_ZOOM_CENTER,
			motionBlur: r.motionBlur ?? DEFAULT_ZOOM_MOTION_BLUR,
			source: r.source ?? "manual",
		}));
		autoZoomEnabled = s.autoZoomEnabled ?? autoZoomEnabled;
		autoZoomApplied = s.autoZoomApplied ?? autoZoomApplied;
		cuts = (s.cuts ?? []).map((c: TimelineCut) => ({ ...c }));
		// Annotation undo: restore the captured array. Each entry already
		// carries its own id from the snapshot — we keep them so refs from
		// `selectedAnnotationId` etc. survive the undo cleanly.
		if (Array.isArray(s.annotations)) {
			annotations = s.annotations.map((a: Annotation) => ({ ...a }));
			annotationZSeq = annotations.length + 1;
			if (selectedAnnotationId && !annotations.find((a) => a.id === selectedAnnotationId)) {
				selectedAnnotationId = null;
			}
			if (hoveredAnnotationId && !annotations.find((a) => a.id === hoveredAnnotationId)) {
				hoveredAnnotationId = null;
			}
		}
		cursorSettings = s.cursorSettings;
		audioSettings = s.audioSettings ?? audioSettings;
		watermarkSettings = s.watermarkSettings ?? watermarkSettings;
		// Camera overlay was previously captured in the snapshot but not
		// restored here, which silently destroyed camera-overlay edits on
		// undo. Deep-copy so subsequent mutations don't alias the snapshot.
		if (s.cameraOverlay) {
			cameraOverlay = {
				...s.cameraOverlay,
				defaultPlacement: { ...s.cameraOverlay.defaultPlacement },
				motionSegments: (s.cameraOverlay.motionSegments ?? []).map(
					(seg: CameraOverlaySettings["motionSegments"][number]) => ({ ...seg }),
				),
			};
		}
		layoutMode = s.layoutMode;
		outputAspect = s.outputAspect ?? 'source';
		lastAppliedPresetId = s.lastAppliedPresetId ?? null;
		cursorMotionEasing = s.cursorMotionEasing ?? null;
	}

	function addZoomRegion(
		start: number,
		end: number,
		scale = 1.5,
		center?: { x: number; y: number },
	) {
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
			centerX: center?.x ?? DEFAULT_ZOOM_CENTER,
			centerY: center?.y ?? DEFAULT_ZOOM_CENTER,
			motionBlur: DEFAULT_ZOOM_MOTION_BLUR,
			source: "manual",
		};
		zoomRegions = [...zoomRegions, region];
		selectedZoomRegionId = region.id;
		return region.id;
	}

	/**
	 * Append an auto-generated zoom region without pushing undo (the caller
	 * batches all auto-applied regions into a single undo entry). Returns
	 * the new id so callers can correlate with their suggestion.
	 */
	function addAutoZoomRegion(
		start: number,
		end: number,
		scale: number,
		centerX: number,
		centerY: number,
	) {
		const region: ZoomRegion = {
			id: generateId(),
			start,
			end,
			scale,
			easeIn: { ...EASE },
			easeOut: { ...EASE },
			rampIn: DEFAULT_ZOOM_RAMP,
			rampOut: DEFAULT_ZOOM_RAMP,
			centerX,
			centerY,
			motionBlur: DEFAULT_ZOOM_MOTION_BLUR,
			source: "auto",
		};
		zoomRegions = [...zoomRegions, region];
		return region.id;
	}

	function clearAutoZooms() {
		const hasAuto = zoomRegions.some((z) => z.source === "auto");
		if (!hasAuto) return;
		pushUndoState();
		zoomRegions = zoomRegions.filter((z) => z.source !== "auto");
		if (
			selectedZoomRegionId &&
			!zoomRegions.find((z) => z.id === selectedZoomRegionId)
		) {
			selectedZoomRegionId = null;
		}
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

	/**
	 * Patch the camera overlay settings. Mirrors `updateCursorSettings`
	 * shape — callers handle their own `pushUndoState` so coalesced
	 * interactions (drag, slider) can batch into a single undo entry.
	 */
	function updateCameraOverlay(updates: Partial<CameraOverlaySettings>) {
		cameraOverlay = { ...cameraOverlay, ...updates };
	}

	function removeZoomRegion(id: string) {
		pushUndoState();
		zoomRegions = zoomRegions.filter((z) => z.id !== id);
		if (selectedZoomRegionId === id) selectedZoomRegionId = null;
	}

	function updateZoomRegion(id: string, updates: Partial<ZoomRegion>) {
		zoomRegions = zoomRegions.map((z) => {
			if (z.id !== id) return z;
			// First user edit on an auto region detaches it — "Clear auto
			// zooms" should leave anything they've tweaked alone.
			const next = { ...z, ...updates };
			if (z.source === "auto" && updates.source === undefined) {
				next.source = "manual";
			}
			return next;
		});
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
			zIndex: annotationZSeq++,
			opacity: 1,
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
		if (hoveredAnnotationId === id) hoveredAnnotationId = null;
	}

	/** Sorted view by (zIndex, insertion-order). Higher z draws later. */
	function annotationsByZ(): Annotation[] {
		return [...annotations]
			.map((a, idx) => ({ a, idx, z: a.zIndex ?? idx }))
			.sort((a, b) => (a.z - b.z) || (a.idx - b.idx))
			.map((e) => e.a);
	}

	function toggleAnnotationLock(id: string) {
		pushUndoState();
		annotations = annotations.map((a) =>
			a.id === id ? { ...a, locked: !(a.locked ?? false) } : a,
		);
	}

	function toggleAnnotationVisibility(id: string) {
		pushUndoState();
		annotations = annotations.map((a) =>
			a.id === id ? { ...a, hidden: !(a.hidden ?? false) } : a,
		);
	}

	function renameAnnotation(id: string, name: string) {
		const trimmed = name.trim();
		pushUndoState();
		annotations = annotations.map((a) =>
			a.id === id ? { ...a, name: trimmed || undefined } : a,
		);
	}

	function duplicateAnnotation(id: string): Annotation | null {
		const source = annotations.find((a) => a.id === id);
		if (!source) return null;
		pushUndoState();
		const offset = 0.01;
		const dup: Annotation = JSON.parse(JSON.stringify(source));
		dup.id = generateId();
		dup.zIndex = annotationZSeq++;
		dup.name = source.name ? `${source.name} copy` : undefined;
		// Nudge the geometry diagonally so the duplicate is visible.
		if (dup.kind.kind === "rect" || dup.kind.kind === "ellipse" || dup.kind.kind === "image" || dup.kind.kind === "text") {
			dup.kind = { ...dup.kind, x: dup.kind.x + offset, y: dup.kind.y + offset };
		} else if (dup.kind.kind === "arrow") {
			dup.kind = {
				...dup.kind,
				x1: dup.kind.x1 + offset,
				y1: dup.kind.y1 + offset,
				x2: dup.kind.x2 + offset,
				y2: dup.kind.y2 + offset,
			};
		}
		annotations = [...annotations, dup];
		selectedAnnotationId = dup.id;
		return dup;
	}

	/**
	 * Reorder by setting the annotation's `zIndex` relative to its neighbours.
	 * `direction = 1` brings forward, `-1` sends backward. Multiple steps will
	 * skip over multiple neighbours.
	 */
	function reorderAnnotation(id: string, direction: 1 | -1) {
		const ordered = annotationsByZ();
		const idx = ordered.findIndex((a) => a.id === id);
		if (idx === -1) return;
		const targetIdx = idx + direction;
		if (targetIdx < 0 || targetIdx >= ordered.length) return;
		pushUndoState();
		// Reassign z values to a strictly-monotonic 1..N sequence with the
		// pair swapped so the result is stable under repeated reorders.
		const next = [...ordered];
		[next[idx], next[targetIdx]] = [next[targetIdx], next[idx]];
		const zMap = new Map(next.map((a, i) => [a.id, i + 1]));
		annotations = annotations.map((a) => ({ ...a, zIndex: zMap.get(a.id) ?? a.zIndex }));
		annotationZSeq = next.length + 1;
	}

	/** Move to absolute z-position by id (used by drag-reorder in the layer panel). */
	function setAnnotationZOrder(orderedIds: string[]) {
		pushUndoState();
		const zMap = new Map(orderedIds.map((id, i) => [id, i + 1]));
		annotations = annotations.map((a) =>
			zMap.has(a.id) ? { ...a, zIndex: zMap.get(a.id)! } : a,
		);
		annotationZSeq = orderedIds.length + 1;
	}

	function reset() {
		currentTime = 0;
		isPlaying = false;
		trimStart = 0;
		trimEnd = metadata?.duration ?? 0;
		backgroundType = 'wallpaper';
		backgroundValue = wallpaperBackgroundValue(WALLPAPERS[0].id);
		backgroundBlur = 40;
		padding = 3;
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
		outputAspect = 'source';
		lastAppliedPresetId = null;
		zoomRegions = [];
		selectedZoomRegionId = null;
		cuts = [];
		autoZoomEnabled = true;
		autoZoomApplied = false;
		annotations = [];
		selectedAnnotationId = null;
		annotationTool = null;
		hoveredAnnotationId = null;
		annotationsGloballyHidden = false;
		annotationSnapEnabled = true;
		annotationZSeq = 1;
		cursorMotionEasing = null;
		cursorSettings = {
			enabled: true,
			size: 2,
			style: 'dot',
			smoothing: 50,
			snapToClicks: true,
			snapWindowMs: 80,
			highlightClicks: true,
			highlightColor: '#3b82f6',
			highlightOpacity: 40,
			hideWhenIdle: false,
			idleTimeout: 3,
			motionBlur: 0,
			clickBounce: 0,
			bounceSpeedMs: 220,
			sway: 0,
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
		cameraOverlay = {
			enabled: false,
			mirror: true,
			shape: 'rounded',
			cornerRadius: 0.16,
			animationPreset: 'soft',
			defaultPlacement: cameraPlacementFromPreset('bottom-right'),
			motionSegments: [],
		};
		exportQuality = 'hd';
		undoStack = [];
		redoStack = [];
	}

	/**
	 * Add a removed range. Returns the new cut id, or null if the range is
	 * too short to be meaningful. Each call is its own undo entry — callers
	 * accepting several silence suggestions at once should batch with their
	 * own `pushUndoState` and use the lower-level array if needed.
	 */
	function addCut(
		start: number,
		end: number,
		source: CutSource = 'silence',
	): string | null {
		if (end - start <= 0.01) return null;
		pushUndoState();
		const cut: TimelineCut = { id: generateId(), start, end, source };
		cuts = [...cuts, cut].sort((a, b) => a.start - b.start);
		return cut.id;
	}

	function removeCut(id: string) {
		if (!cuts.some((c) => c.id === id)) return;
		pushUndoState();
		cuts = cuts.filter((c) => c.id !== id);
	}

	function clearCuts() {
		if (cuts.length === 0) return;
		pushUndoState();
		cuts = [];
	}

	function toRenderState(): EditorRenderState {
		return {
			trimStart,
			trimEnd,
			outputAspect,
			lastAppliedPresetId,
			backgroundType,
			backgroundValue,
			backgroundBlur,
			padding,
			borderRadius,
			cursorEnabled: cursorSettings.enabled,
			cursorSize: cursorSettings.size,
			cursorStyle: cursorSettings.style,
			cursorSmoothing: cursorSettings.smoothing,
			cursorSnapToClicks: cursorSettings.snapToClicks,
			cursorSnapWindowMs: cursorSettings.snapWindowMs,
			cursorHighlightClicks: cursorSettings.highlightClicks,
			cursorHighlightColor: cursorSettings.highlightColor,
			cursorHighlightOpacity: cursorSettings.highlightOpacity,
			cursorHideWhenIdle: cursorSettings.hideWhenIdle,
			cursorIdleTimeout: cursorSettings.idleTimeout,
			cursorMotionBlur: cursorSettings.motionBlur,
			cursorClickBounce: cursorSettings.clickBounce,
			cursorBounceSpeedMs: cursorSettings.bounceSpeedMs,
			cursorSway: cursorSettings.sway,
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
				source: region.source,
			})),
			autoZoomApplied,
			autoZoomEnabled,
			cuts: cuts.map((cut) => ({ ...cut })),
			cursorMotionEasing,
			annotations: annotations.map((annotation) => ({ ...annotation })),
			shadow: { ...shadow },
			audioSettings: { ...audioSettings },
			watermarkSettings: { ...watermarkSettings },
			cameraOverlay: {
				...cameraOverlay,
				defaultPlacement: { ...cameraOverlay.defaultPlacement },
				motionSegments: cameraOverlay.motionSegments.map((segment) => ({
					...segment,
				})),
			},
			layoutMode,
		};
	}

	function loadRenderState(state: Partial<EditorRenderState>) {
		trimStart = state.trimStart ?? 0;
		trimEnd = state.trimEnd ?? metadata?.duration ?? 0;
		outputAspect = state.outputAspect ?? 'source';
		lastAppliedPresetId = state.lastAppliedPresetId ?? null;
		backgroundType = state.backgroundType ?? 'color';
		backgroundValue = state.backgroundValue ?? '#111111';
		backgroundBlur = state.backgroundBlur ?? 0;
		padding = normalizeFramePaddingPercent(state.padding ?? 0, metadata);
		borderRadius = state.borderRadius ?? 0;
		cursorSettings = {
			...cursorSettings,
			enabled: state.cursorEnabled ?? cursorSettings.enabled,
			size: state.cursorSize ?? cursorSettings.size,
			style: state.cursorStyle ?? cursorSettings.style,
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
			motionBlur: state.cursorMotionBlur ?? cursorSettings.motionBlur,
			clickBounce: state.cursorClickBounce ?? cursorSettings.clickBounce,
			bounceSpeedMs:
				state.cursorBounceSpeedMs ?? cursorSettings.bounceSpeedMs,
			sway: state.cursorSway ?? cursorSettings.sway,
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
			source: region.source ?? "manual",
		}));
		// Legacy projects predate the auto-zoom flags. Treat them as already
		// processed so we don't retroactively scatter zooms across footage
		// the user already finished editing.
		autoZoomEnabled = state.autoZoomEnabled ?? true;
		autoZoomApplied =
			state.autoZoomApplied ??
			(state.zoomRegions !== undefined ? true : false);
		cuts = (state.cuts ?? []).map((c) => ({
			id: c.id ?? generateId(),
			start: c.start,
			end: c.end,
			source: c.source ?? 'silence',
		}));
		shadow = state.shadow ?? shadow;
		audioSettings = state.audioSettings ?? audioSettings;
		watermarkSettings = state.watermarkSettings ?? watermarkSettings;
		// Camera overlay defaults match the Phase 1 spec: bottom-right at
		// 16% size. Older projects stored top-right at 22%; the explicit
		// `?? `-fallbacks below preserve those if present, only swapping in
		// the new defaults when the field is absent on the loaded state.
		const fallbackPlacement = cameraPlacementFromPreset('bottom-right');
		cameraOverlay = {
			enabled: state.cameraOverlay?.enabled ?? false,
			mirror: state.cameraOverlay?.mirror ?? true,
			shape: state.cameraOverlay?.shape ?? 'rounded',
			cornerRadius: state.cameraOverlay?.cornerRadius ?? 0.16,
			animationPreset: state.cameraOverlay?.animationPreset ?? 'soft',
			defaultPlacement: {
				x: state.cameraOverlay?.defaultPlacement?.x ?? fallbackPlacement.x,
				y: state.cameraOverlay?.defaultPlacement?.y ?? fallbackPlacement.y,
				width: state.cameraOverlay?.defaultPlacement?.width ?? fallbackPlacement.width,
				height: state.cameraOverlay?.defaultPlacement?.height ?? fallbackPlacement.height,
			},
			motionSegments: (state.cameraOverlay?.motionSegments ?? []).map((segment) => ({
				start: segment.start,
				end: segment.end,
				fromX: segment.fromX,
				fromY: segment.fromY,
				fromWidth: segment.fromWidth,
				fromHeight: segment.fromHeight,
				toX: segment.toX,
				toY: segment.toY,
				toWidth: segment.toWidth,
				toHeight: segment.toHeight,
				easeIn: segment.easeIn ?? { ...EASE },
				easeOut: segment.easeOut ?? { ...EASE },
				source: segment.source ?? 'manual',
			})),
		};
		cursorMotionEasing = state.cursorMotionEasing ?? null;
		layoutMode = state.layoutMode ?? layoutMode;
		annotations = (state.annotations ?? []).map((a, idx) => ({
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
			// v2 fields with sane defaults so v1 projects keep loading.
			name: a.name,
			zIndex: a.zIndex ?? idx + 1,
			locked: a.locked ?? false,
			hidden: a.hidden ?? false,
			opacity: a.opacity ?? 1,
			glow: a.glow,
		}));
		annotationZSeq = annotations.length + 1;
		selectedAnnotationId = null;
		annotationTool = null;
		hoveredAnnotationId = null;
		annotationsGloballyHidden = false;
		// A freshly loaded document matches on-disk state — no unsaved edits.
		isDirty = false;
	}

	return {
		// Getters (reactive reads)
		get videoPath() { return videoPath; },
		set videoPath(v: string) { videoPath = v; },

		get cursorPath() { return cursorPath; },
		set cursorPath(v: string | null) { cursorPath = v; },

		get recordingPath() { return recordingPath; },
		set recordingPath(v: string | null) { recordingPath = v; },

		get audioPath() { return audioPath; },
		set audioPath(v: string | null) { audioPath = v; },

		get microphonePath() { return microphonePath; },
		set microphonePath(v: string | null) { microphonePath = v; },

		get metadata() { return metadata; },
		set metadata(v: VideoMetadata | null) { metadata = v; },

		get thumbnailStrip() { return thumbnailStrip; },
		set thumbnailStrip(v: string[]) { thumbnailStrip = v; },

		get currentTime() { return currentTime; },
		set currentTime(v: number) { currentTime = v; },

		get isPlaying() { return isPlaying; },
		set isPlaying(v: boolean) { isPlaying = v; },

		// Raw mark fields. Setters intentionally do NOT push undo — callers
		// (Timeline drag/keyboard handlers) own undo coalescing via
		// `pushUndoStateCoalesced` so a single drag or held arrow key is one
		// undo entry, not one-per-pointer-frame.
		get trimStart() { return trimStart; },
		set trimStart(v: number) { trimStart = v; isDirty = true; },

		get trimEnd() { return trimEnd; },
		set trimEnd(v: number) { trimEnd = v; isDirty = true; },

		// Convenience accessors using NLE terminology. `outPoint` resolves
		// the legacy `0 = unset` sentinel against the source duration so
		// callers never need the `trimEnd || duration` dance.
		get inPoint() { return Math.max(0, trimStart); },
		get outPoint() {
			const d = metadata?.duration ?? 0;
			return trimEnd > 0 ? Math.min(trimEnd, d) : d;
		},
		get clipDuration() {
			const d = metadata?.duration ?? 0;
			const out = trimEnd > 0 ? Math.min(trimEnd, d) : d;
			return Math.max(0, out - Math.max(0, trimStart));
		},

		get backgroundType() { return backgroundType; },
		set backgroundType(v: BackgroundType) { pushUndoState(); backgroundType = v; },

		get backgroundValue() { return backgroundValue; },
		set backgroundValue(v: string) { pushUndoState(); backgroundValue = v; },

		get backgroundBlur() { return backgroundBlur; },
		set backgroundBlur(v: number) { backgroundBlur = v; },

		get padding() { return padding; },
		set padding(v: number) { padding = clampFramePaddingPercent(v); },

		get borderRadius() { return borderRadius; },
		set borderRadius(v: number) { borderRadius = v; },

		get shadow() { return shadow; },
		set shadow(v: ShadowSettings) { shadow = v; },

		get layoutMode() { return layoutMode; },
		set layoutMode(v: LayoutMode) { pushUndoState(); layoutMode = v; },
		get outputAspect() { return outputAspect; },
		set outputAspect(v: OutputAspect) { pushUndoState(); outputAspect = v; },
		get lastAppliedPresetId() { return lastAppliedPresetId; },
		set lastAppliedPresetId(v: string | null) { lastAppliedPresetId = v; },

		get zoomRegions() { return zoomRegions; },

		// Silence / manual cuts.
		get cuts() { return cuts; },
		get cutDuration() { return totalCutDuration(cuts); },

		get autoZoomEnabled() { return autoZoomEnabled; },
		set autoZoomEnabled(v: boolean) { autoZoomEnabled = v; isDirty = true; },

		get autoZoomApplied() { return autoZoomApplied; },
		set autoZoomApplied(v: boolean) { autoZoomApplied = v; isDirty = true; },

		get cursorSamplesRaw() { return cursorSamplesRaw; },
		set cursorSamplesRaw(v: CursorSampleLike[]) { cursorSamplesRaw = v; },

		get selectedZoomRegionId() { return selectedZoomRegionId; },
		set selectedZoomRegionId(v: string | null) { selectedZoomRegionId = v; },

		get activePanel() { return activePanel; },
		set activePanel(v: PanelTab) { activePanel = v; },

		get cursorMotionEasing() { return cursorMotionEasing; },
		set cursorMotionEasing(v: Easing | null) { pushUndoState(); cursorMotionEasing = v; },

		get annotations() { return annotations; },
		get annotationsByZ() { return annotationsByZ(); },
		get selectedAnnotationId() { return selectedAnnotationId; },
		set selectedAnnotationId(v: string | null) { selectedAnnotationId = v; },
		get annotationTool() { return annotationTool; },
		set annotationTool(v: AnnotationKindName | null) { annotationTool = v; },
		get hoveredAnnotationId() { return hoveredAnnotationId; },
		set hoveredAnnotationId(v: string | null) { hoveredAnnotationId = v; },
		get annotationsGloballyHidden() { return annotationsGloballyHidden; },
		set annotationsGloballyHidden(v: boolean) { annotationsGloballyHidden = v; },
		get annotationSnapEnabled() { return annotationSnapEnabled; },
		set annotationSnapEnabled(v: boolean) { annotationSnapEnabled = v; },

		get cursorSettings() { return cursorSettings; },
		set cursorSettings(v: CursorSettings) { cursorSettings = v; },

		get audioSettings() { return audioSettings; },
		set audioSettings(v: AudioSettings) { audioSettings = v; },

		get watermarkSettings() { return watermarkSettings; },
		set watermarkSettings(v: WatermarkSettings) { watermarkSettings = v; },

		get cameraOverlay() { return cameraOverlay; },
		set cameraOverlay(v: CameraOverlaySettings) { cameraOverlay = v; },

		get exportFormat() { return exportFormat; },
		set exportFormat(v: ExportFormat) { exportFormat = v; },

		get exportQuality() { return exportQuality; },
		set exportQuality(v: ExportQuality) { exportQuality = v; },

		get gifSettings() { return gifSettings; },
		set gifSettings(v: GifSettings) { gifSettings = v; },
		updateGifSettings(updates: Partial<GifSettings>) {
			gifSettings = { ...gifSettings, ...updates };
		},

		get exportProgress() { return exportProgress; },
		set exportProgress(v: number | null) { exportProgress = v; },

		get isExporting() { return isExporting; },
		set isExporting(v: boolean) { isExporting = v; },

		get timelineZoom() { return timelineZoom; },
		set timelineZoom(v: number) { timelineZoom = v; },

		get canUndo() { return undoStack.length > 0; },
		get canRedo() { return redoStack.length > 0; },

		get isDirty() { return isDirty; },
		get lastSavedAt() { return lastSavedAt; },

		// Methods
		undo,
		redo,
		pushUndoState,
		pushUndoStateCoalesced,
		markSaved,
		setBackground,
		updateCursorSettings,
		updateAudioSettings,
		updateWatermarkSettings,
		updateShadow,
		updateCameraOverlay,
		addZoomRegion,
		addAutoZoomRegion,
		clearAutoZooms,
		removeZoomRegion,
		updateZoomRegion,
		selectZoomRegion,
		addCut,
		removeCut,
		clearCuts,
		addAnnotation,
		updateAnnotation,
		removeAnnotation,
		toggleAnnotationLock,
		toggleAnnotationVisibility,
		renameAnnotation,
		duplicateAnnotation,
		reorderAnnotation,
		setAnnotationZOrder,
		reset,
		toRenderState,
		loadRenderState,
	};
}

export type EditorStore = ReturnType<typeof createEditorStore>;
