/**
 * Editor Store: Central reactive state for the video editor.
 * Uses Svelte 5 runes ($state) for granular reactivity.
 */

import type { Transcript } from '../ipc';
import { type CaptionAnimation, DEFAULT_CAPTION_ANIMATION } from '../captions/animation';
import { googleFontStack } from '../fonts/google-fonts';
import type { CursorSampleLike } from '../cursor/smoothing';
import { EASE, type Easing } from '../easing/cubic-bezier';
import { log } from '../logger';
import { resolveTokenRgb, resolveTokenRgba } from '../annotations/canvas-tokens';
// Narrow import (not `$lib/registry`) so the registry's `builtins` side-effect
// (which pulls this store's catalogs) can't form an import cycle.
import { resolveBackgroundWireValue } from '../registry/resolve';
import { totalCutDuration, type CutSource, type TimelineCut } from '../timeline/cuts';
import { deriveSegments, planDeleteSegment, planSplit, segmentAt, type Segment } from '../timeline/segments';
import {
	buildSpeedOf,
	pruneSegmentSpeeds,
	type SegmentSpeed,
	segmentSpeedAt as speedAtAnchor,
	segmentSpeedAtTime as speedAtTime,
	setSegmentSpeed as upsertSegmentSpeed,
} from '../timeline/segment-speed';
import {
	type MotionTone,
	pruneSegmentAnims,
	retuneAnimsForTone,
	type SceneAnimSpec,
	type SegmentAnim,
	segmentAnimAt as animAtAnchor,
	setSegmentAnim as upsertSegmentAnim,
} from '../scenes/segment-anim';
import {
	seamTransitionAt as readSeamTransition,
	type SeamTransition,
	setSeamTransition as applySeamTransition,
} from '../scenes/seam';
import { displayTimeMap, timeMapFromSegments } from '../timeline/time-map';
import { experimentalStore } from './experimental.svelte';

export type BackgroundType = 'wallpaper' | 'image' | 'color' | 'gradient';


export interface WallpaperOption {
	/** Matches the `id` in `assets/manifest.json`. Stored in `backgroundValue`
	 *  as `asset:<id>` so preview and export resolve against the downloaded cache. */
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
	centerX: number; // UV 0..1: focus point X; 0.5 = center crop
	centerY: number; // UV 0..1: focus point Y; 0.5 = center crop
	motionBlur: number; // 0..1: preview motion-blur strength multiplier
	/**
	 * Origin of the region. "auto" means added by Smart Auto-Zoom on first
	 * load; flipped to "manual" the moment the user edits any field so
	 * "Clear auto zooms" leaves their tweaks alone.
	 */
	source: "manual" | "auto";
	/**
	 * Muted in preview AND export but kept in the project: a non-destructive
	 * toggle so you can A/B a zoom without losing its settings. Absent = visible.
	 */
	hidden?: boolean;
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

// Annotations. Position/size live in video UV space (0..1) so they follow zoom
// and crop transforms without re-projection. `kind` is a discriminated union.

export type AnnotationStrokeStyle = "solid" | "dashed" | "dotted";

export interface AnnotationStroke {
	width: number; // UV
	color: string; // CSS colour
	/** Stroke pattern. Defaults to "solid" for v1 projects (`undefined` ↔ solid). */
	style?: AnnotationStrokeStyle;
}

/**
 * Optional glow / soft shadow. Renders in export for rect, ellipse, image, and
 * text; arrow glow is preview-only (the Annotations tab notes this per kind).
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
		radius: number; // corner radius, fraction of the shorter side (0..0.5)
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
		/** Blur strength 0..1, mapping to a box-blur radius up to ~5% of the canvas. */
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
	/** Optional glow / soft shadow. Exports for rect/ellipse/image/text; arrow is preview-only. */
	glow?: AnnotationGlow;
	/** What the annotation is pinned to. "video" (default) tracks the zoomed
	 *  video content; "frame" pins it to the output frame so zoom/focus never
	 *  moves it. Absent = "video". */
	anchor?: AnnotationAnchor;
}

/** Coordinate space an annotation is anchored to. */
export type AnnotationAnchor = "video" | "frame";

export const DEFAULT_ANNOTATION_RAMP = 0.2;
export const DEFAULT_ANNOTATION_STROKE: AnnotationStroke = {
	width: 0.004,
	color: "#3b82f6",
};
export const DEFAULT_ANNOTATION_FILL = "rgba(59,130,246,0.20)";

// Bundled built-in cursor styles. `dot` is the default soft circle (drawn by
// the WebGL2 shader and the Rust export overlay); the system sets are SVG
// sprites. The legacy macos/windows/outline/target styles moved into the
// installable "Classic Cursors" pack (`ext:classic-cursors:<id>`).
export type CursorStyleId = 'dot' | 'macos-system' | 'windows-system';

/**
 * Stored cursor selection: a built-in {@link CursorStyleId} or an
 * `ext:<extId>:<localId>` id contributed by an installed cursor pack. Kept as a
 * widened string (the `string & {}` trick preserves built-in autocomplete)
 * because extension ids can't be enumerated at compile time. Resolution +
 * graceful fallback (unknown id → soft dot) lives in `lib/registry/resolve.ts`.
 */
export type StoredCursorId = CursorStyleId | (string & {});

export interface CursorSettings {
	enabled: boolean;
	size: number; // 1-5 scale
	style: StoredCursorId;
	smoothing: number; // 0-100 → Gaussian σ in ms (0 = raw capture, 100 ≈ 150 ms)
	snapToClicks: boolean; // anchor smoothed path to exact click x/y around mouse-down
	snapWindowMs: number; // half-width (ms) of the snap anchor, 0..200
	highlightClicks: boolean;
	highlightColor: string;
	highlightOpacity: number; // 0-100
	hideWhenIdle: boolean;
	idleTimeout: number; // seconds
	/** Motion-blur strength: 0 = off, 1 = strong velocity trail. */
	motionBlur: number;
	/** Click-bounce amplitude: 0 = no bounce, 5 = exaggerated squash. */
	clickBounce: number;
	/** Bounce/squash duration in ms. */
	bounceSpeedMs: number;
	/** Idle sway amplitude: subtle wobble during slow motion. 0 = off, 1 = max. */
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
 * `custom` returns the current bottom-right placement as a sane fallback;
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
 * Inverse of `cameraPlacementFromPreset`: find which preset (if any) the
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

// Pure padding maths lives in `$lib/editor/frame-padding` so `.logic.ts` modules
// and unit tests can import it without loading this runes store. Imported for
// internal use and re-exported to keep existing import sites working.
import {
	MAX_FRAME_PADDING_PERCENT,
	clampFramePaddingPercent,
	framePaddingPixels,
	normalizeFramePaddingPercent,
} from "$lib/editor/frame-padding";
export {
	MAX_FRAME_PADDING_PERCENT,
	clampFramePaddingPercent,
	framePaddingPixels,
	normalizeFramePaddingPercent,
};

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
	/** Generated captions (transcript) + how they render. Optional: projects
	 *  saved before captions landed simply omit these. */
	transcript?: Transcript | null;
	captionStyle?: CaptionStyle;
	cursorEnabled: boolean;
	cursorSize: number;
	/**
	 * User-picked cursor sprite style (`dot` / `macos` / `windows` /
	 * `outline` / `target`). Optional for backwards compatibility: projects
	 * saved before this field landed default to `dot` on load.
	 */
	cursorStyle?: StoredCursorId;
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
		/** Stable identity, persisted so sections diff cleanly across saves. */
		id?: string;
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
		hidden?: boolean;
	}>;
	autoZoomApplied?: boolean;
	autoZoomEnabled?: boolean;
	/** Silence / manual cuts removed from the timeline. */
	cuts?: TimelineCut[];
	/** Whether cuts apply in preview/export (false = bypassed but preserved). */
	cutsEnabled?: boolean;
	/** Split markers (original-recording seconds) dividing the clip into
	 *  individually deletable segments. Editor-only: has no export effect on
	 *  its own; deleting a segment is what produces a cut. */
	splitPoints?: number[];
	/** Per-segment speed overrides, anchored to a segment's original start. A
	 *  segment with no entry plays at 1×. */
	segmentSpeeds?: SegmentSpeed[];
	/** Per-segment scene animations (entrance/exit video-layer transforms),
	 *  anchored to a segment's original start. A segment with no entry is static. */
	segmentAnims?: SegmentAnim[];
	/** Project-wide scene-animation motion style (defaults to "balanced"). */
	motionTone?: MotionTone;
	/** Whether zoom regions apply in preview/export. */
	focusEnabled?: boolean;
	/** Whether annotations render in preview/export. Negation of the
	 * pre-existing `annotationsGloballyHidden` flag, surfaced here so all
	 * three lane toggles round-trip through the project file. */
	annotationsEnabled?: boolean;
	/** Silence suggestions the user dismissed, kept so they don't resurface. */
	dismissedSilences?: Array<{ start: number; end: number }>;
	cursorMotionEasing: Easing | null;
	annotations: Array<Omit<Annotation, "id">>;
	shadow: ShadowSettings;
	audioSettings: AudioSettings;
	watermarkSettings: WatermarkSettings;
	cameraOverlay: CameraOverlaySettings;
	/**
	 * Editor layout mode (`auto` / column-stacked variants etc.). Optional
	 * for backwards compatibility: pre-field projects keep their default.
	 */
	layoutMode?: LayoutMode;
	// Hybrid-raster cursor sprite, populated only on the export path
	// (the editor route runs `rasterizeCursorSprites` right before invoking
	// `export_video`). Not persisted to disk; never set by `loadRenderState`.
	cursorSpriteRest?: string;          // data:image/png;base64,…
	cursorSpritePress?: string;         // optional; falls back to rest in Rust
	cursorSpriteRightPress?: string;    // optional; falls back to press → rest in Rust
	cursorSpriteDrag?: string;          // optional; falls back to press → rest in Rust
	cursorSpriteHotspotRest?: [number, number];   // 0..1 sprite UV
	cursorSpriteHotspotPress?: [number, number];
	cursorSpriteHotspotRightPress?: [number, number];
	cursorSpriteHotspotDrag?: [number, number];
	cursorSpriteSizePx?: number;        // sprite render size in source pixels
}

export type ExportFormat = 'mp4' | 'gif' | 'webm';
export type ExportQuality = 'small' | 'hd' | '4k' | 'source';
/** Encoder effort axis, orthogonal to {@link ExportQuality} (resolution).
 *  'balanced' reproduces the historical encoder settings exactly. */
export type ExportSpeed = 'fast' | 'balanced' | 'quality';

/** GIF dithering algorithm. Trades file size against gradient quality. */
export type GifDither = 'bayer' | 'sierra2' | 'none';
/** GIF quality preset: controls palette size + dither bias. */
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
 * the final canvas to a target ratio: the source video stays centred,
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

export type PanelTab = 'clip' | 'background' | 'focus' | 'annotations' | 'cursor' | 'camera' | 'audio' | 'captions' | 'extensions' | 'info';

// Wallpapers 19–23 were moved into the installable "Waves" extension pack
// (extensions/packs/waves-wallpapers); keep the built-in default set at 18 so
// the extension flow has real background content to exercise.
export const WALLPAPERS: WallpaperOption[] = Array.from({ length: 18 }, (_, i) => ({
	id: `wallpaper${i + 1}`,
	label: `Wallpaper ${i + 1}`,
}));

/**
 * A single gradient color stop. `pos` is a percentage (0–100) along the
 * gradient line, matching the CSS `linear-gradient` stop syntax.
 */
export interface GradientStop {
	color: string;
	pos: number;
}

/** A parsed linear gradient: an angle (CSS degrees) and 2+ color stops. */
export interface GradientSpec {
	angle: number;
	stops: GradientStop[];
}

/** Max stops the preview shader / export rasteriser support. */
export const MAX_GRADIENT_STOPS = 8;

/**
 * Curated gradient presets, authored as full `linear-gradient(...)` strings:
 * the exact source of truth the preview shader and export rasteriser both parse.
 */
export const GRADIENT_PRESETS: { label: string; value: string }[] = [
	{ label: 'Indigo', value: 'linear-gradient(135deg, #6366f1 0%, #8b5cf6 50%, #d946ef 100%)' },
	{ label: 'Sunset', value: 'linear-gradient(120deg, #ff6a00 0%, #ee0979 100%)' },
	{ label: 'Ocean', value: 'linear-gradient(135deg, #2193b0 0%, #6dd5ed 100%)' },
	{ label: 'Aurora', value: 'linear-gradient(135deg, #00c9ff 0%, #92fe9d 100%)' },
	{ label: 'Ember', value: 'linear-gradient(135deg, #f12711 0%, #f5af19 100%)' },
	{ label: 'Mint', value: 'linear-gradient(135deg, #11998e 0%, #38ef7d 100%)' },
	{ label: 'Grape', value: 'linear-gradient(135deg, #7028e4 0%, #e5b2ca 100%)' },
	{ label: 'Berry', value: 'linear-gradient(135deg, #c31432 0%, #240b36 100%)' },
	{ label: 'Royal', value: 'linear-gradient(160deg, #141e30 0%, #243b55 100%)' },
	{ label: 'Slate', value: 'linear-gradient(135deg, #232526 0%, #414345 100%)' },
	{ label: 'Peach', value: 'linear-gradient(135deg, #ed4264 0%, #ffedbc 100%)' },
	{ label: 'Lagoon', value: 'linear-gradient(160deg, #43c6ac 0%, #191654 100%)' },
];

/** Default gradient used when a fresh custom gradient is created. */
export const DEFAULT_GRADIENT = GRADIENT_PRESETS[0].value;

function clampNum(v: number, lo: number, hi: number): number {
	return Math.min(hi, Math.max(lo, v));
}

/** Expand #rgb/#rgba shorthand and lowercase, so downstream parsing is uniform. */
function normalizeHex(hex: string): string {
	let h = hex.trim().replace(/^#/, '');
	if (h.length === 3 || h.length === 4) {
		h = h.split('').map((c) => c + c).join('');
	}
	return `#${h.toLowerCase()}`;
}

/**
 * Parse a CSS `linear-gradient(...)` string into an angle + stops. Tolerant of
 * a missing angle (defaults 135°) and missing stop positions (distributes them
 * evenly). Always returns at least two stops so the builder UI and the
 * renderers have a well-formed spec to work with.
 */
export function parseGradient(value: string): GradientSpec {
	const angleMatch = value.match(/(-?\d+(?:\.\d+)?)deg/);
	const angle = angleMatch
		? (((parseFloat(angleMatch[1]) % 360) + 360) % 360)
		: 135;

	const stopRe = /(#(?:[0-9a-fA-F]{8}|[0-9a-fA-F]{6}|[0-9a-fA-F]{3,4}))(?:\s+(-?\d+(?:\.\d+)?)%)?/g;
	const raw: { color: string; pos: number | null }[] = [];
	let m: RegExpExecArray | null;
	while ((m = stopRe.exec(value)) !== null) {
		raw.push({
			color: normalizeHex(m[1]),
			pos: m[2] != null ? clampNum(parseFloat(m[2]), 0, 100) : null,
		});
	}

	if (raw.length === 0) {
		return { angle, stops: [{ color: '#6366f1', pos: 0 }, { color: '#d946ef', pos: 100 }] };
	}
	if (raw.length === 1) {
		return { angle, stops: [{ color: raw[0].color, pos: 0 }, { color: raw[0].color, pos: 100 }] };
	}
	const n = raw.length;
	const stops = raw.map((s, i) => ({
		color: s.color,
		pos: s.pos != null ? s.pos : (i / (n - 1)) * 100,
	}));
	return { angle, stops };
}

/** Serialize a {@link GradientSpec} back to a canonical CSS gradient string. */
export function serializeGradient(spec: GradientSpec): string {
	const angle = (((Math.round(spec.angle) % 360) + 360) % 360);
	const body = [...spec.stops]
		.sort((a, b) => a.pos - b.pos)
		.map((s) => `${normalizeHex(s.color)} ${Math.round(clampNum(s.pos, 0, 100))}%`)
		.join(', ');
	return `linear-gradient(${angle}deg, ${body})`;
}

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
/** How generated captions render over the preview / export. */
export interface CaptionStyle {
	enabled: boolean;
	/** CSS font-family stack. */
	fontFamily: string;
	/** Font weight (400–800). */
	fontWeight: number;
	/** Font size as a percent of the preview/video height. */
	fontSizePct: number;
	position: 'bottom' | 'center' | 'top';
	/** Horizontal alignment of the caption block. */
	align: 'left' | 'center' | 'right';
	/** Top/bottom nudge, percent of frame height: + pushes the caption outward
	 *  into the padding, − pulls it back onto the video. Ignored for `center`. */
	offsetPct: number;
	/** Text colour (hex). */
	color: string;
	/** Render text in uppercase. */
	uppercase: boolean;
	/** Letter spacing, in em (can be negative). */
	letterSpacing: number;
	/** Backing behind the text: none, soft shadow, or a solid box. */
	background: 'none' | 'soft' | 'box';
	/** Box backing colour (hex), used when `background` is `box`. */
	backgroundColor: string;
	/** Box backing opacity (0–100), used when `background` is `box`. */
	backgroundOpacity: number;
	/** Outline / stroke thickness as a percent of font size (0 = none). */
	outlineWidth: number;
	/** Outline / stroke colour (hex). */
	outlineColor: string;
	/** Max lines shown at once before clamping. */
	maxLines: number;
	/** Word-by-word animation. Absent = static (today's behaviour). */
	animation?: CaptionAnimation;
}

export const DEFAULT_CAPTION_STYLE: CaptionStyle = {
	enabled: true,
	fontFamily: 'system-ui, sans-serif',
	fontWeight: 600,
	fontSizePct: 5,
	position: 'bottom',
	align: 'center',
	offsetPct: 6,
	color: '#ffffff',
	uppercase: false,
	letterSpacing: 0,
	background: 'soft',
	backgroundColor: '#000000',
	backgroundOpacity: 65,
	outlineWidth: 0,
	outlineColor: '#000000',
	maxLines: 2,
	animation: DEFAULT_CAPTION_ANIMATION,
};

/** A named caption look: the visual half of {@link CaptionStyle}. Applied
 *  wholesale; users then tweak. Built-ins ship a few; extension packs add more
 *  via the asset registry (`captionPreset` kind). */
export interface CaptionPreset {
	id: string;
	label: string;
	description?: string;
	style: Omit<CaptionStyle, 'enabled'>;
}

/** Shipped caption themes: a modern, cohesive set spanning minimal → impact.
 *  Each preset is a complete look (typography + colour + animation). Presets
 *  aren't stored by id (applying one copies its style), so this list can evolve
 *  freely. Ordered clean → bold so the picker reads as a spectrum. */
export const CAPTION_PRESETS: CaptionPreset[] = [
	{
		id: 'clean',
		label: 'Clean',
		description: 'Minimal',
		style: {
			fontFamily: googleFontStack('Inter'),
			fontWeight: 600,
			fontSizePct: 5,
			position: 'bottom',
			align: 'center',
			offsetPct: 8,
			color: '#ffffff',
			uppercase: false,
			letterSpacing: 0,
			background: 'soft',
			backgroundColor: '#000000',
			backgroundOpacity: 65,
			outlineWidth: 0,
			outlineColor: '#0a0a0a',
			maxLines: 2,
			animation: {
				chunk: 'line',
				chunkSize: 3,
				emphasis: 'none',
				emphasisColor: '#4ade80',
				entrance: 'fade',
				entranceMs: 160,
				holdGaps: true,
			},
		},
	},
	{
		id: 'pill',
		label: 'Pill',
		description: 'Rounded bar',
		style: {
			fontFamily: googleFontStack('Plus Jakarta Sans'),
			fontWeight: 700,
			fontSizePct: 4.6,
			position: 'bottom',
			align: 'center',
			offsetPct: 8,
			color: '#ffffff',
			uppercase: false,
			letterSpacing: 0,
			background: 'box',
			backgroundColor: '#0b0b12',
			backgroundOpacity: 80,
			outlineWidth: 0,
			outlineColor: '#0a0a0a',
			maxLines: 2,
			animation: {
				chunk: 'phrase',
				chunkSize: 3,
				emphasis: 'none',
				emphasisColor: '#4ade80',
				entrance: 'fade',
				entranceMs: 200,
				holdGaps: true,
			},
		},
	},
	{
		id: 'bold',
		label: 'Bold',
		description: 'Strong outline',
		style: {
			fontFamily: googleFontStack('Montserrat'),
			fontWeight: 800,
			fontSizePct: 5.6,
			position: 'bottom',
			align: 'center',
			offsetPct: 10,
			color: '#ffffff',
			uppercase: false,
			letterSpacing: 0,
			background: 'none',
			backgroundColor: '#000000',
			backgroundOpacity: 0,
			outlineWidth: 6,
			outlineColor: '#0a0a0a',
			maxLines: 2,
			animation: {
				chunk: 'line',
				chunkSize: 3,
				emphasis: 'none',
				emphasisColor: '#4ade80',
				entrance: 'fade',
				entranceMs: 150,
				holdGaps: true,
			},
		},
	},
	{
		id: 'spotlight',
		label: 'Spotlight',
		description: 'Active word',
		style: {
			fontFamily: googleFontStack('Inter'),
			fontWeight: 800,
			fontSizePct: 5.4,
			position: 'bottom',
			align: 'center',
			offsetPct: 10,
			color: '#ffffff',
			uppercase: false,
			letterSpacing: 0,
			background: 'soft',
			backgroundColor: '#000000',
			backgroundOpacity: 0,
			outlineWidth: 4,
			outlineColor: '#0a0a0a',
			maxLines: 2,
			animation: {
				chunk: 'line',
				chunkSize: 4,
				emphasis: 'color',
				emphasisColor: '#4ade80',
				entrance: 'none',
				entranceMs: 0,
				holdGaps: true,
			},
		},
	},
	{
		id: 'wave',
		label: 'Wave',
		description: 'Cyan reveal',
		style: {
			fontFamily: googleFontStack('Outfit'),
			fontWeight: 700,
			fontSizePct: 5,
			position: 'bottom',
			align: 'center',
			offsetPct: 8,
			color: '#ffffff',
			uppercase: false,
			letterSpacing: 0,
			background: 'soft',
			backgroundColor: '#000000',
			backgroundOpacity: 0,
			outlineWidth: 0,
			outlineColor: '#0a0a0a',
			maxLines: 2,
			animation: {
				chunk: 'phrase',
				chunkSize: 3,
				emphasis: 'color',
				emphasisColor: '#38bdf8',
				entrance: 'fade',
				entranceMs: 200,
				holdGaps: true,
			},
		},
	},
	{
		id: 'punch',
		label: 'Punch',
		description: 'Word pop',
		style: {
			fontFamily: googleFontStack('Anton'),
			fontWeight: 700,
			fontSizePct: 7.5,
			position: 'center',
			align: 'center',
			offsetPct: 0,
			color: '#ffffff',
			uppercase: true,
			letterSpacing: 0.01,
			background: 'none',
			backgroundColor: '#000000',
			backgroundOpacity: 0,
			outlineWidth: 8,
			outlineColor: '#0a0a0a',
			maxLines: 1,
			animation: {
				chunk: 'word',
				chunkSize: 1,
				emphasis: 'scale',
				emphasisColor: '#4ade80',
				entrance: 'pop',
				entranceMs: 170,
				holdGaps: true,
			},
		},
	},
	{
		id: 'hype',
		label: 'Hype',
		description: 'Big impact',
		style: {
			fontFamily: googleFontStack('Anton'),
			fontWeight: 700,
			fontSizePct: 8,
			position: 'center',
			align: 'center',
			offsetPct: 0,
			color: '#ffffff',
			uppercase: true,
			letterSpacing: 0.01,
			background: 'none',
			backgroundColor: '#000000',
			backgroundOpacity: 0,
			outlineWidth: 9,
			outlineColor: '#0a0a0a',
			maxLines: 2,
			animation: {
				chunk: 'phrase',
				chunkSize: 3,
				emphasis: 'color',
				emphasisColor: '#fde047',
				entrance: 'pop',
				entranceMs: 150,
				holdGaps: true,
			},
		},
	},
];

/** What to do with generated captions on export. Independent choices: you can
 *  burn captions into the pixels AND keep a sidecar file. The sidecar is also
 *  what Cloud uploads as a selectable caption track. */
export interface CaptionExportOptions {
	/** Burn the captions into the video (overlay). Ignored for GIF. */
	burnIn: boolean;
	/** Write a separate subtitle file next to the export ('none' to skip). */
	sidecar: 'none' | 'vtt' | 'srt';
}

export const DEFAULT_CAPTION_EXPORT: CaptionExportOptions = {
	burnIn: false,
	sidecar: 'vtt',
};

export function createEditorStore() {
	// Video source
	let videoPath = $state('');
	let cursorPath = $state<string | null>(null);
	// Raw on-disk media paths (the extracted recording / audio tracks), needed
	// by Rust-side analysis commands such as silence detection.
	let recordingPath = $state<string | null>(null);
	let audioPath = $state<string | null>(null);
	let microphonePath = $state<string | null>(null);
	// Captions: the generated transcript (session-only for now; project-format
	// persistence is deferred) + how it renders over the preview.
	let transcript = $state.raw<Transcript | null>(null);
	let captionStyle = $state<CaptionStyle>({ ...DEFAULT_CAPTION_STYLE });
	let metadata = $state<VideoMetadata | null>(null);
	// `$state.raw` for large replace-only arrays: swapped wholesale, never
	// mutated element-wise, so deep-proxying thousands of entries is pure
	// overhead; only the array identity needs reactivity. See AGENTS.md.
	let thumbnailStrip = $state.raw<string[]>([]);
	// Audio peak envelope (0..1 per bucket) for the timeline waveform. Transient.
	let waveform = $state.raw<number[]>([]);

	// Playback
	let currentTime = $state(0);
	let isPlaying = $state(false);
	// The component that owns the <video>/picture-clock registers a transport
	// seek here. `seek()` moves the playhead AND the transport in lockstep, so a
	// seek from a panel (e.g. a transcript line) lands even mid-playback; setting
	// `currentTime` alone is overwritten by the next playback time publish.
	let seekHandler: ((time: number) => void) | null = null;

	// Trim
	let trimStart = $state(0);
	let trimEnd = $state(0); // will be set to duration on load

	// Silence / manual cuts: removed ranges, in original-recording seconds.
	let cuts = $state<TimelineCut[]>([]);
	// Split markers (original-recording seconds) that divide the clip into
	// individually deletable segments. Purely an editing aid, with no export effect
	// until a segment between two boundaries is ripple-deleted (→ a manual cut).
	let splitPoints = $state<number[]>([]);
	let segmentSpeeds = $state<SegmentSpeed[]>([]);
	// Per-segment scene animations (entrance/exit transforms on the video layer),
	// anchored to a segment's original start. A segment with no entry is static.
	let segmentAnims = $state<SegmentAnim[]>([]);
	// Project-wide motion style for scene animations. Authoring-only: it bakes
	// concrete values into each spec, so the export pipeline never reads it.
	let motionTone = $state<MotionTone>('balanced');
	// Transient (not serialized): true only while a trim handle is being dragged.
	// Flips the timeline onto the full-recording axis so the handle can move
	// across the whole source and reveal the trimmed head/tail (Cap-style ghost).
	let isTrimming = $state(false);
	// Transient UI selection: the start time of the highlighted clip block, or
	// null. Not serialized (mirrors selectedZoomRegionId).
	let selectedClipStart = $state<number | null>(null);
	// Silence suggestions the user has dismissed. Persisted so a re-scan or a
	// project reopen doesn't resurface ranges they already rejected.
	let dismissedSilences = $state<Array<{ start: number; end: number }>>([]);
	// Per-lane "enable" toggles. When off, the lane greys out, its effect is
	// bypassed in preview, and the data is excluded from the export pipeline.
	// The underlying data (cuts, zoom regions) is preserved either way so the
	// toggle is fully reversible.
	let cutsEnabled = $state(true);
	let focusEnabled = $state(true);

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

	// Id of the most recently applied preset. Pure UI affordance that lets the
	// toolbar surface "Story · 9:16" so users see, per project, what's in
	// effect. Cleared when the user resets back to source.
	let lastAppliedPresetId = $state<string | null>(null);

	// Raw cursor samples shared by the preview compositor and the Cursor panel's
	// trajectory minimap. Set by VideoPreview on load; read-only elsewhere.
	// `$state.raw`: tens of thousands of samples, replace-only on load.
	let cursorSamplesRaw = $state.raw<CursorSampleLike[]>([]);

	// Annotations + active tool (for the preview canvas's place-mode).
	let annotations = $state<Annotation[]>([]);
	let selectedAnnotationId = $state<string | null>(null);
	let annotationTool = $state<AnnotationKindName | null>(null);
	// Layer-panel hover state: when set, the overlay flashes the matching
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
	// Camera overlay defaults, Phase 1 spec:
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
	// Default to source resolution; downscaling a 1080p+ screen recording to a
	// fixed "HD" needlessly softens sharp text/UI.
	let exportQuality = $state<ExportQuality>('source');
	let exportSpeed = $state<ExportSpeed>('balanced');
	// Output frame rate for MP4/WebM. `null` = keep the source recording's rate
	// (the quality-preserving default). A number requests a clean downsample to
	// that rate (only ever offered ≤ source, so we never duplicate frames). GIF
	// has its own fps control in `gifSettings`.
	let exportFps = $state<number | null>(null);
	// Seed the 60fps export default once per project (see the `metadata` setter):
	// a >60fps recording exports far faster at 60 with no perceptible loss, but
	// Original/30/24 stay selectable. The flag stops a later metadata set from
	// clobbering a user's choice.
	let exportFpsDefaulted = false;
	let gifSettings = $state<GifSettings>({ ...DEFAULT_GIF_SETTINGS });
	// How captions are emitted on export (burn-in / sidecar). Session-only, like
	// the other export prefs.
	let captionExport = $state<CaptionExportOptions>({ ...DEFAULT_CAPTION_EXPORT });
	let exportProgress = $state<number | null>(null);
	let isExporting = $state(false);

	// Undo/redo history: arrays of de-proxied settings snapshots, always
	// reassigned with spreads (never index-mutated) and bounded to
	// `MAX_UNDO_HISTORY`. `$state.raw` since only the array identity is read
	// reactively; the snapshot contents are already plain.
	type EditorSettings = ReturnType<typeof captureSettings>;
	let undoStack = $state.raw<EditorSettings[]>([]);
	let redoStack = $state.raw<EditorSettings[]>([]);

	// Dirty tracking: flips to true the moment the user makes any undoable edit,
	// clears when the edits are persisted to the .recast archive (markSaved) or
	// when a fresh render state is loaded from disk.
	let isDirty = $state(false);
	let lastSavedAt = $state<number | null>(null);
	// Frozen snapshot of the last on-disk state. Used by `revertToSaved` to
	// blow away every unsaved edit at once without walking the undo stack.
	// Captured whenever the project is loaded from disk or successfully saved.
	let savedSnapshot = $state<EditorSettings | null>(null);

	// Timeline zoom
	let timelineZoom = $state(1); // 1x = fit to width

	// Every undoable state field. Anything left out here silently survives a
	// `pushUndoState` call but isn't restored on undo: the user sees unrelated
	// edits revert while their tweak stays put. Keep in sync with `applySnapshot`.
	function captureSettings() {
		return {
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
			splitPoints,
			segmentSpeeds,
			segmentAnims,
			motionTone,
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
		};
	}

	// Deep, de-proxied clone via `$state.snapshot` rather than a JSON round-trip,
	// so undo/redo never pays a stringify+parse on the press. `applySnapshot`
	// copies each field into fresh state, so the stored snapshot stays pristine.
	function getSettingsSnapshot(): EditorSettings {
		return $state.snapshot(captureSettings()) as EditorSettings;
	}

	const MAX_UNDO_HISTORY = 50;

	function pushUndoState() {
		undoStack = [...undoStack, getSettingsSnapshot()].slice(-MAX_UNDO_HISTORY);
		redoStack = [];
		isDirty = true;
	}

	// Drop the most recent undo entry. For unwinding a push that turned into a
	// no-op, e.g. a placement that pushed at pointer-down but was cancelled
	// (too-small drag / stray click) before it changed anything.
	function popUndoState() {
		if (undoStack.length > 0) undoStack = undoStack.slice(0, -1);
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
		savedSnapshot = getSettingsSnapshot();
	}

	function revertToSaved() {
		if (!savedSnapshot) return;
		// Push the current state onto the undo stack so the revert itself
		// is undoable, so a user who reverts by mistake can Ctrl+Z their way
		// back to the work they discarded.
		undoStack = [...undoStack, getSettingsSnapshot()].slice(-MAX_UNDO_HISTORY);
		redoStack = [];
		applySnapshot(savedSnapshot);
		isDirty = false;
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

	// Each field is copied (spread/map) into fresh state, never aliased: the
	// snapshot lives on the undo stack and must stay immutable across re-applies.
	function applySnapshot(s: EditorSettings) {
		backgroundType = s.backgroundType;
		backgroundValue = s.backgroundValue;
		backgroundBlur = s.backgroundBlur;
		padding = normalizeFramePaddingPercent(s.padding, metadata);
		borderRadius = s.borderRadius ?? 0;
		shadow = s.shadow ? { ...s.shadow } : shadow;
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
		splitPoints = [...(s.splitPoints ?? [])];
		segmentSpeeds = (s.segmentSpeeds ?? []).map((o: SegmentSpeed) => ({ ...o }));
		segmentAnims = (s.segmentAnims ?? []).map((o: SegmentAnim) => ({ ...o }));
		motionTone = s.motionTone ?? 'balanced';
		// Annotation undo: restore the captured array. Each entry already
		// carries its own id from the snapshot, so we keep them so refs from
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
		cursorSettings = { ...s.cursorSettings };
		audioSettings = s.audioSettings ? { ...s.audioSettings } : audioSettings;
		watermarkSettings = s.watermarkSettings
			? { ...s.watermarkSettings }
			: watermarkSettings;
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
		log.info('focus', 'zoom_added', { id: region.id, start, end, scale });
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
		// `value` can be a long wallpaper/gradient string, so log only the type.
		log.info('background', 'changed', { type: selection.type });
	}

	/**
	 * Stream a background value during a continuous gesture (dragging a
	 * gradient stop's color/position or the angle). Updates fire live so the
	 * WebGL preview tracks the drag, but the whole gesture coalesces into a
	 * single undo entry (mirrors the keyboard-nudge / slider pattern) instead
	 * of spamming one push per pointer-move. Discrete actions (presets, add /
	 * remove stop) should use {@link setBackground} for a clean undo step.
	 */
	function setBackgroundLive(type: BackgroundType, value: string) {
		pushUndoStateCoalesced('background-live');
		backgroundType = type;
		backgroundValue = value;
		isDirty = true;
	}

	function updateCursorSettings(updates: Partial<CursorSettings>) {
		cursorSettings = { ...cursorSettings, ...updates };
		// Sliders (size, smoothing) fire continuously, so debounce to one line.
		log.debounced('cursor-settings', 'cursor', 'settings_changed', { ...updates });
	}

	function updateAudioSettings(updates: Partial<AudioSettings>) {
		audioSettings = { ...audioSettings, ...updates };
		log.debounced('audio-settings', 'audio', 'settings_changed', { ...updates });
	}

	function updateWatermarkSettings(updates: Partial<WatermarkSettings>) {
		watermarkSettings = { ...watermarkSettings, ...updates };
	}

	function updateShadow(updates: Partial<ShadowSettings>) {
		shadow = { ...shadow, ...updates };
	}

	/**
	 * Patch the camera overlay settings. Mirrors `updateCursorSettings`
	 * shape; callers handle their own `pushUndoState` so coalesced
	 * interactions (drag, slider) can batch into a single undo entry.
	 */
	function updateCameraOverlay(updates: Partial<CameraOverlaySettings>) {
		cameraOverlay = { ...cameraOverlay, ...updates };
	}

	function removeZoomRegion(id: string) {
		pushUndoState();
		zoomRegions = zoomRegions.filter((z) => z.id !== id);
		if (selectedZoomRegionId === id) selectedZoomRegionId = null;
		log.info('focus', 'zoom_removed', { id });
	}

	/** Remove every zoom region in one undo step. */
	function clearZoomRegions() {
		if (zoomRegions.length === 0) return;
		pushUndoState();
		zoomRegions = [];
		selectedZoomRegionId = null;
		log.info('focus', 'zoom_cleared_all', {});
	}

	/**
	 * Duplicate a region's settings into a new one placed immediately after it
	 * (back-to-back, same duration), clamped to the clip. A duplicate is always
	 * "manual" (it's an explicit user edit) and the copy becomes selected.
	 */
	function duplicateZoomRegion(id: string) {
		const src = zoomRegions.find((z) => z.id === id);
		if (!src) return;
		pushUndoState();
		const clipEnd = trimEnd || metadata?.duration || src.end;
		const duration = Math.max(0.1, src.end - src.start);
		let start = src.end;
		let end = start + duration;
		if (end > clipEnd) {
			// No room after the original: clamp the tail; if that collapses the
			// window, just stack it on top of the original (user can drag it).
			end = clipEnd;
			start = Math.max(trimStart, end - duration);
			if (end - start < 0.1) {
				start = src.start;
				end = src.end;
			}
		}
		const copy: ZoomRegion = {
			...src,
			id: generateId(),
			easeIn: { ...src.easeIn },
			easeOut: { ...src.easeOut },
			start,
			end,
			source: "manual",
			hidden: src.hidden ?? false,
		};
		// Insert right after the source so list order matches the timeline.
		const idx = zoomRegions.findIndex((z) => z.id === id);
		zoomRegions = [
			...zoomRegions.slice(0, idx + 1),
			copy,
			...zoomRegions.slice(idx + 1),
		];
		selectedZoomRegionId = copy.id;
		log.info('focus', 'zoom_duplicated', { from: id, id: copy.id });
		return copy.id;
	}

	/** Toggle (or set) a region's hidden flag: non-destructive mute. */
	function setZoomRegionHidden(id: string, hidden?: boolean) {
		const src = zoomRegions.find((z) => z.id === id);
		if (!src) return;
		pushUndoState();
		const next = hidden ?? !(src.hidden ?? false);
		zoomRegions = zoomRegions.map((z) => (z.id === id ? { ...z, hidden: next } : z));
		log.info('focus', 'zoom_hidden_toggled', { id, hidden: next });
	}

	function updateZoomRegion(id: string, updates: Partial<ZoomRegion>) {
		// Drag/resize/slider edits stream in, so debounce per region id.
		log.debounced(`zoom-${id}`, 'focus', 'zoom_updated', { id, ...updates });
		zoomRegions = zoomRegions.map((z) => {
			if (z.id !== id) return z;
			// First user edit on an auto region detaches it, so "Clear auto
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
		// New annotations pick up the current theme colour rather than a fixed
		// blue, so markup matches the app out of the box (resolved to a concrete
		// colour here since the export bakes it).
		const themeColor = resolveTokenRgb('var(--primary)');
		const annotation: Annotation = {
			id: generateId(),
			start: s,
			end: e,
			rampIn: DEFAULT_ANNOTATION_RAMP,
			rampOut: DEFAULT_ANNOTATION_RAMP,
			easeIn: { ...EASE },
			easeOut: { ...EASE },
			// Images start borderless (opt-in via the Appearance controls); shapes
			// keep the default hairline stroke.
			stroke: {
				...DEFAULT_ANNOTATION_STROKE,
				color: themeColor,
				width: kind.kind === 'image' ? 0 : DEFAULT_ANNOTATION_STROKE.width,
			},
			fill: resolveTokenRgba('var(--primary)', 0.18),
			kind,
			zIndex: annotationZSeq++,
			opacity: 1,
		};
		annotations = [...annotations, annotation];
		selectedAnnotationId = annotation.id;
		log.info('annotation', 'added', { id: annotation.id, kind: kind.kind });
		return annotation;
	}

	function updateAnnotation(id: string, updates: Partial<Annotation>) {
		// Position/style edits stream from drags + property sliders, so debounce.
		log.debounced(`annotation-${id}`, 'annotation', 'updated', {
			id,
			fields: Object.keys(updates),
		});
		annotations = annotations.map((a) => (a.id === id ? { ...a, ...updates } : a));
	}

	function removeAnnotation(id: string, pushUndo = true) {
		if (pushUndo) pushUndoState();
		annotations = annotations.filter((a) => a.id !== id);
		if (selectedAnnotationId === id) selectedAnnotationId = null;
		if (hoveredAnnotationId === id) hoveredAnnotationId = null;
		log.info('annotation', 'removed', { id });
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
		if (dup.kind.kind === "rect" || dup.kind.kind === "ellipse" || dup.kind.kind === "image" || dup.kind.kind === "text" || dup.kind.kind === "blur") {
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
		splitPoints = [];
		segmentSpeeds = [];
		segmentAnims = [];
		motionTone = 'balanced';
		cutsEnabled = true;
		focusEnabled = true;
		dismissedSilences = [];
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
		exportQuality = 'source';
		exportSpeed = 'balanced';
		exportFps = null;
		exportFpsDefaulted = false;
		undoStack = [];
		redoStack = [];
	}

	/**
	 * Add a removed range. Returns the new cut id, or null if the range is
	 * too short to be meaningful. Each call is its own undo entry; callers
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

	/**
	 * Resize a cut. Does NOT push undo; callers (the cut lane's drag
	 * handlers) own coalescing via `pushUndoStateCoalesced` so a whole drag
	 * is one undo entry.
	 */
	function updateCut(id: string, start: number, end: number) {
		cuts = cuts.map((c) => (c.id === id ? { ...c, start, end } : c));
	}

	/**
	 * Merge overlapping or touching cuts into one, keeping the earliest id so
	 * the lane's keyed `{#each}` stays stable. Called at the end of a
	 * create/resize drag, never mid-drag, which would yank the dragged card.
	 */
	function mergeCuts() {
		const sorted = [...cuts].sort((a, b) => a.start - b.start);
		const merged: TimelineCut[] = [];
		for (const c of sorted) {
			const last = merged[merged.length - 1];
			if (last && c.start <= last.end + 0.001) {
				last.end = Math.max(last.end, c.end);
				if (c.source === 'manual') last.source = 'manual';
			} else {
				merged.push({ ...c });
			}
		}
		cuts = merged;
	}

	/** The clip's effective kept bounds [start, end] in original seconds. */
	function clipBounds(): { start: number; end: number } {
		const d = metadata?.duration ?? 0;
		return {
			start: Math.max(0, trimStart),
			end: trimEnd > 0 ? Math.min(trimEnd, d) : d,
		};
	}

	// Manual split/cut is an always-on feature now; auto silence is still its own
	// opt-in flag (`silenceDetection`). A cut is only "active" (i.e. shown on the
	// timeline and applied in playback/export) when its source allows it AND the
	// cuts lane is enabled. A disabled silence flag preserves those edits but
	// ignores them, so toggling it back on restores them.
	function cutFlagAllows(c: TimelineCut): boolean {
		return c.source === 'silence' ? experimentalStore.silenceDetection : true;
	}
	/** Cuts that actually apply right now (flag-gated + lane-enabled). */
	function effectiveCutList(): TimelineCut[] {
		if (!cutsEnabled) return [];
		return cuts.filter(cutFlagAllows);
	}
	function activeSplitPoints(): number[] {
		return splitPoints;
	}

	/** The current clip's kept segments: trim − active cuts, subdivided by
	 * active splits. Drives both the timeline display and the edit math, so the
	 * two never disagree. */
	function currentSegments(): Segment[] {
		const { start, end } = clipBounds();
		return deriveSegments({
			trimStart: start,
			trimEnd: end,
			cuts: effectiveCutList(),
			splitPoints: activeSplitPoints(),
		});
	}

	/** The timeline axis: the KEPT clip only (trimmed head/tail collapse away,
	 * Cap-style), with each kept segment warped by its per-segment speed and cuts
	 * closed to seams. `output 0 == inPoint`; the clip fills the track from the
	 * left. Every lane, the playhead, and the preview clock position against this.
	 * At all-1× speeds it's the cut translation map restricted to [inPoint,outPoint]. */
	function currentTimeMap() {
		const segs = currentSegments();
		const speedOf = buildSpeedOf(segs, segmentSpeeds);
		// While trimming, un-collapse onto the full recording so the handle can
		// move across the whole source (and reveal/restore the trimmed head/tail).
		if (isTrimming) {
			const { start, end } = clipBounds();
			return displayTimeMap({
				trimStart: start,
				trimEnd: end,
				durationSec: metadata?.duration ?? end,
				segments: segs,
				cuts: effectiveCutList(),
				speedOf,
			});
		}
		return timeMapFromSegments(segs, speedOf);
	}

	/** Speed of the segment anchored at original `start` (1 when unset). */
	function segmentSpeedAtStart(start: number): number {
		return speedAtAnchor(segmentSpeeds, start);
	}

	/** Speed of the segment that CONTAINS original time `t` (1 when none / unset).
	 * The legacy `<video>` preview reads this to set `playbackRate` per segment,
	 * since that path plays at the element's native rate rather than the warped
	 * output clock. Tolerant at seams: forward-biased onto the next segment. */
	function segmentSpeedAtTime(t: number): number {
		return speedAtTime(currentSegments(), segmentSpeeds, t);
	}

	/** Set the speed of the segment anchored at original `start`. Coalesced into
	 * one undo entry per anchor while a slider drags; orphaned anchors are pruned. */
	function setSegmentSpeed(start: number, speed: number) {
		pushUndoStateCoalesced(`segment-speed-${start.toFixed(3)}`, 400);
		const next = upsertSegmentSpeed(segmentSpeeds, start, speed);
		segmentSpeeds = pruneSegmentSpeeds(next, currentSegments());
		isDirty = true;
	}

	/** The scene animation anchored at original `start` (null when unset). */
	function segmentAnimAtStart(start: number): SegmentAnim | null {
		return animAtAnchor(segmentAnims, start);
	}

	/** Set or clear (spec = null) the entrance/exit animation of the segment
	 * anchored at original `start`. Coalesced into one undo entry per anchor+side
	 * while presets/sliders change; orphaned anchors are pruned. */
	function setSegmentAnim(start: number, side: 'in' | 'out', spec: SceneAnimSpec | null) {
		pushUndoStateCoalesced(`segment-anim-${side}-${start.toFixed(3)}`, 400);
		const next = upsertSegmentAnim(segmentAnims, start, side, spec);
		segmentAnims = pruneSegmentAnims(next, currentSegments());
		isDirty = true;
	}

	/** Set the project-wide scene-animation motion style, restyling every existing
	 * animation to match (so the dial visibly re-tones the whole video). */
	function setMotionTone(tone: MotionTone) {
		if (tone === motionTone) return;
		pushUndoState();
		motionTone = tone;
		segmentAnims = retuneAnimsForTone(segmentAnims, tone);
		isDirty = true;
	}

	/** Set the transition across the seam between the segments anchored at
	 * `leftStart` and `rightStart`: a matched exit+entrance styled by the current
	 * motion tone. One undo entry for the pair. */
	function setSeamTransition(leftStart: number, rightStart: number, kind: SeamTransition) {
		pushUndoState();
		const next = applySeamTransition(segmentAnims, leftStart, rightStart, kind, motionTone);
		segmentAnims = pruneSegmentAnims(next, currentSegments());
		isDirty = true;
	}

	/** The transition currently spanning that seam ("none" / a push / "custom"). */
	function seamTransitionAt(leftStart: number, rightStart: number): SeamTransition | 'custom' {
		return readSeamTransition(segmentAnims, leftStart, rightStart);
	}

	/** Split the clip at original time `t`. Returns true if a split was added. */
	function splitAt(t: number): boolean {
		const { start, end } = clipBounds();
		const next = planSplit(t, {
			trimStart: start,
			trimEnd: end,
			cuts: effectiveCutList(),
			splitPoints,
		});
		if (!next) return false;
		pushUndoState();
		splitPoints = next;
		selectedClipStart = null;
		return true;
	}

	/** Remove the split marker at (≈) original time `t`, rejoining the clips. */
	function removeSplit(t: number) {
		const next = splitPoints.filter((p) => Math.abs(p - t) > 1e-4);
		if (next.length === splitPoints.length) return;
		pushUndoState();
		splitPoints = next;
	}

	function clearSplits() {
		if (splitPoints.length === 0) return;
		pushUndoState();
		splitPoints = [];
	}

	/**
	 * Ripple-delete the segment containing original time `t`: the segment's
	 * range becomes a manual cut and the gap closes via the cut time-map. Pruned
	 * split points that bordered it are dropped. Returns the original-time
	 * "join" (the first kept frame after the deletion) to park the playhead on,
	 * or null when there's nothing to delete (only one segment, or `t` is not in
	 * a segment): the whole recording can't be removed this way.
	 */
	function deleteSegmentAt(t: number): number | null {
		const segs = currentSegments();
		if (segs.length <= 1) return null;
		const seg = segmentAt(segs, t);
		if (!seg) return null;
		pushUndoState();
		const plan = planDeleteSegment(seg, splitPoints);
		splitPoints = plan.splitPoints;
		cuts = [
			...cuts,
			{
				id: generateId(),
				start: plan.cut.start,
				end: plan.cut.end,
				source: 'manual' as CutSource,
			},
		].sort((a, b) => a.start - b.start);
		selectedClipStart = null;
		// `seg.end` is the first kept frame after the removed range (end-exclusive
		// cut), so it's a safe spot for the playhead; `seg.start` would land
		// inside the new cut.
		return seg.end;
	}

	/** Record a dismissed silence range so detection won't suggest it again. */
	function dismissSilence(start: number, end: number) {
		dismissedSilences = [...dismissedSilences, { start, end }];
		isDirty = true;
	}

	/** Wipe all dismissed silence ranges so the next detection pass surfaces
	 *  every candidate again. Used by the popover's "Reset dismissed" button
	 *  when the user wants to reconsider previously-rejected suggestions. */
	function clearDismissedSilences() {
		if (dismissedSilences.length === 0) return;
		dismissedSilences = [];
		isDirty = true;
	}

	function toRenderState(): EditorRenderState {
		return {
			trimStart,
			trimEnd,
			outputAspect,
			lastAppliedPresetId,
			backgroundType,
			// `ext:` background ids → the pack's hydrated absolute path; built-in
			// values (hex/gradient/`asset:<id>`) pass through unchanged.
			backgroundValue: resolveBackgroundWireValue(backgroundValue),
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
				id: region.id,
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
				hidden: region.hidden ?? false,
			})),
			autoZoomApplied,
			autoZoomEnabled,
			cuts: cuts.map((cut) => ({ ...cut })),
			splitPoints: [...splitPoints],
			// Prune orphaned anchors on save so the section diffs cleanly.
			segmentSpeeds: pruneSegmentSpeeds(segmentSpeeds, currentSegments()),
			segmentAnims: pruneSegmentAnims(segmentAnims, currentSegments()),
			motionTone,
			cutsEnabled,
			focusEnabled,
			annotationsEnabled: !annotationsGloballyHidden,
			dismissedSilences: dismissedSilences.map((d) => ({ ...d })),
			cursorMotionEasing,
			annotations: annotations.map((annotation) => ({ ...annotation })),
			shadow: { ...shadow },
			audioSettings: { ...audioSettings },
			transcript,
			captionStyle: { ...captionStyle },
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
			id: region.id ?? generateId(),
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
			hidden: region.hidden ?? false,
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
		dismissedSilences = (state.dismissedSilences ?? []).map((d) => ({
			start: d.start,
			end: d.end,
		}));
		cutsEnabled = state.cutsEnabled ?? true;
		splitPoints = [...(state.splitPoints ?? [])];
		segmentSpeeds = (state.segmentSpeeds ?? []).map((o) => ({ ...o }));
		segmentAnims = (state.segmentAnims ?? []).map((o) => ({ ...o }));
		motionTone = state.motionTone ?? 'balanced';
		focusEnabled = state.focusEnabled ?? true;
		shadow = state.shadow ?? shadow;
		audioSettings = state.audioSettings ?? audioSettings;
		transcript = state.transcript ?? null;
		captionStyle = state.captionStyle
			? { ...DEFAULT_CAPTION_STYLE, ...state.captionStyle }
			: { ...DEFAULT_CAPTION_STYLE };
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
			anchor: a.anchor,
		}));
		annotationZSeq = annotations.length + 1;
		selectedAnnotationId = null;
		annotationTool = null;
		hoveredAnnotationId = null;
		// Restore the "hide all annotations" toggle. Hidden only when explicitly
		// disabled; absent (older projects) or true → visible.
		annotationsGloballyHidden = state.annotationsEnabled === false;
		// A freshly loaded document matches on-disk state, so no unsaved edits.
		isDirty = false;
		// Anchor `revertToSaved` to the just-loaded state.
		savedSnapshot = getSettingsSnapshot();
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
		set metadata(v: VideoMetadata | null) {
			metadata = v;
			// Default export to 60fps for >60fps recordings: imperceptible for a
			// screen demo, ~halves export time. Seeded once so a later user choice is
			// never clobbered; Original/30/24 remain selectable in the dialog.
			if (v) {
				if (!exportFpsDefaulted && v.fps > 60.5) exportFps = 60;
				exportFpsDefaulted = true;
			}
		},

		get thumbnailStrip() { return thumbnailStrip; },
		set thumbnailStrip(v: string[]) { thumbnailStrip = v; },

		get waveform() { return waveform; },
		set waveform(v: number[]) { waveform = v; },

		get currentTime() { return currentTime; },
		set currentTime(v: number) { currentTime = v; },

		/** Register the transport seek (the <video>/clock owner). Returns an
		 *  unsubscribe to call on teardown. */
		registerSeekHandler(fn: (time: number) => void) {
			seekHandler = fn;
			return () => {
				if (seekHandler === fn) seekHandler = null;
			};
		},
		/** Move the playhead AND the playback transport to `time`. Use this for
		 *  any seek that originates outside the player (transcript, chapters, …)
		 *  so it lands whether paused or playing. */
		seek(time: number) {
			currentTime = time;
			seekHandler?.(time);
		},

		get isPlaying() { return isPlaying; },
		set isPlaying(v: boolean) { isPlaying = v; },

		// Raw mark fields. Setters intentionally do NOT push undo; callers
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

		// Silence / manual cuts. `cuts` is the raw stored list (used by feature
		// banners and the silence review); `effectiveCuts` is the flag-gated +
		// lane-enabled subset that actually applies in playback/export/display.
		get cuts() { return cuts; },
		get effectiveCuts() { return effectiveCutList(); },
		get cutDuration() { return totalCutDuration(cuts); },
		get dismissedSilences() { return dismissedSilences; },

		// Lane "enable" toggles: bypass the lane's effect in preview/export
		// while keeping the underlying data intact.
		get cutsEnabled() { return cutsEnabled; },
		set cutsEnabled(v: boolean) { cutsEnabled = v; isDirty = true; log.info('feature', 'toggled', { feature: 'cuts', enabled: v }); },

		// Split/segment editing. `segments` is derived (trim − active cuts, sliced
		// by active splits); `splitPoints` is the marker list the timeline renders
		// Both empty out when timeline editing is opted off.
		get segments() { return currentSegments(); },
		get splitPoints() { return activeSplitPoints(); },
		get segmentSpeeds() { return segmentSpeeds; },
		// Warped output axis (kept segments × per-segment speed). Reduces to the
		// cut translation map when every segment is 1×. Un-collapses to the full
		// recording while `isTrimming` so a trim drag can reveal the trimmed parts.
		get timeMap() { return currentTimeMap(); },
		get isTrimming() { return isTrimming; },
		set isTrimming(v: boolean) { isTrimming = v; },
		segmentSpeedAt: segmentSpeedAtStart,
		segmentSpeedAtTime,
		setSegmentSpeed,
		get segmentAnims() { return segmentAnims; },
		segmentAnimAt: segmentAnimAtStart,
		setSegmentAnim,
		get motionTone() { return motionTone; },
		setMotionTone,
		setSeamTransition,
		seamTransitionAt,
		get selectedClipStart() { return selectedClipStart; },
		set selectedClipStart(v: number | null) { selectedClipStart = v; },
		get focusEnabled() { return focusEnabled; },
		set focusEnabled(v: boolean) { focusEnabled = v; isDirty = true; log.info('feature', 'toggled', { feature: 'focus', enabled: v }); },

		get autoZoomEnabled() { return autoZoomEnabled; },
		set autoZoomEnabled(v: boolean) { autoZoomEnabled = v; isDirty = true; log.info('feature', 'toggled', { feature: 'autoZoom', enabled: v }); },

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
		set annotationsGloballyHidden(v: boolean) { annotationsGloballyHidden = v; log.info('feature', 'toggled', { feature: 'annotations', enabled: !v }); },
		get annotationSnapEnabled() { return annotationSnapEnabled; },
		set annotationSnapEnabled(v: boolean) { annotationSnapEnabled = v; },

		get cursorSettings() { return cursorSettings; },
		set cursorSettings(v: CursorSettings) { cursorSettings = v; },

		get audioSettings() { return audioSettings; },
		set audioSettings(v: AudioSettings) { audioSettings = v; },

		get transcript() { return transcript; },
		set transcript(v: Transcript | null) { transcript = v; isDirty = true; },
		get captionStyle() { return captionStyle; },
		set captionStyle(v: CaptionStyle) { captionStyle = v; isDirty = true; },
		updateCaptionStyle(updates: Partial<CaptionStyle>) {
			captionStyle = { ...captionStyle, ...updates };
			isDirty = true;
		},

		get watermarkSettings() { return watermarkSettings; },
		set watermarkSettings(v: WatermarkSettings) { watermarkSettings = v; },

		get cameraOverlay() { return cameraOverlay; },
		set cameraOverlay(v: CameraOverlaySettings) { cameraOverlay = v; },

		get exportFormat() { return exportFormat; },
		set exportFormat(v: ExportFormat) { exportFormat = v; },

		get exportQuality() { return exportQuality; },
		set exportQuality(v: ExportQuality) { exportQuality = v; },

		get exportSpeed() { return exportSpeed; },
		set exportSpeed(v: ExportSpeed) { exportSpeed = v; },

		get exportFps() { return exportFps; },
		set exportFps(v: number | null) { exportFps = v; },

		get captionExport() { return captionExport; },
		set captionExport(v: CaptionExportOptions) { captionExport = v; },
		updateCaptionExport(updates: Partial<CaptionExportOptions>) {
			captionExport = { ...captionExport, ...updates };
		},

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
		// Revert is only meaningful once we have a saved baseline AND the
		// user has diverged from it. Without `isDirty` the button would be
		// a no-op that still consumed an undo slot.
		get canRevert() { return isDirty && savedSnapshot !== null; },

		get isDirty() { return isDirty; },
		get lastSavedAt() { return lastSavedAt; },

		// Methods
		undo,
		redo,
		pushUndoState,
		popUndoState,
		pushUndoStateCoalesced,
		markSaved,
		revertToSaved,
		setBackground,
		setBackgroundLive,
		updateCursorSettings,
		updateAudioSettings,
		updateWatermarkSettings,
		updateShadow,
		updateCameraOverlay,
		addZoomRegion,
		addAutoZoomRegion,
		clearAutoZooms,
		removeZoomRegion,
		clearZoomRegions,
		duplicateZoomRegion,
		setZoomRegionHidden,
		updateZoomRegion,
		selectZoomRegion,
		addCut,
		removeCut,
		clearCuts,
		updateCut,
		mergeCuts,
		splitAt,
		removeSplit,
		clearSplits,
		deleteSegmentAt,
		dismissSilence,
		clearDismissedSilences,
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
