/**
 * The editor's project model: `EditorRenderState` and every type, default and
 * pure helper it is built from. Split out of the store so the IPC type layer can
 * depend on the document shape without depending on the store — and so the model
 * carries no runes, no Tauri, and no reactivity.
 */

import {
	CAPTION_PRESETS,
	type CaptionAnimation,
	type CaptionPreset,
	type CaptionStyle,
	DEFAULT_CAPTION_ANIMATION,
	DEFAULT_CAPTION_STYLE,
} from "@recast/captions";
import {
	BACKGROUND_COLORS,
	BACKGROUND_GRADIENTS,
	type BackgroundPreset,
} from "@recast/design/backgrounds";
import type { AudioClip } from "../audio/music";
import type { Easing } from "../easing/cubic-bezier";
import type { MotionTone, SegmentAnim } from "../scenes/segment-anim";
import type { TimelineCut } from "../timeline/cuts";
import type { SegmentSpeed } from "../timeline/segment-speed";
import type { Segment } from "../timeline/segments";

/** Transcript model. Lives here, not in `ipc-types`, because it is part of the
 *  project document (`EditorRenderState.transcript`) — the wire types depend on
 *  the model, never the other way round. */
export interface TranscriptWord {
	start: number;
	end: number;
	text: string;
}

export interface TranscriptSegment {
	id: string;
	start: number;
	end: number;
	text: string;
	words: TranscriptWord[];
}

export interface Transcript {
	engine: string;
	modelId: string;
	language: string | null;
	segments: TranscriptSegment[];
}

export type BackgroundType = "wallpaper" | "image" | "color" | "gradient";

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
export type CursorStyleId = "dot" | "macos-system" | "windows-system";

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
	/** Master output volume (0-100). Multiplied with the per-track gains. */
	volume: number;
	muted: boolean;
	/** System audio gain (0-100). 0 = silenced, 100 = unity. */
	systemVolume: number;
	systemMuted: boolean;
	/** Microphone gain (0-100). 0 = silenced, 100 = unity. */
	micVolume: number;
	micMuted: boolean;
	fadeIn: number; // seconds
	fadeOut: number; // seconds
	/** EBU R128 loudness normalize on the exported mix (export only). */
	normalizeLoudness: boolean;
}

/** Convenience: read a track's effective volume (0-1) with mute applied. */
export function effectiveTrackVolume(settings: AudioSettings, kind: "system" | "mic"): number {
	const muted = kind === "system" ? settings.systemMuted : settings.micMuted;
	if (settings.muted || muted) return 0;
	const v = kind === "system" ? settings.systemVolume : settings.micVolume;
	return Math.max(0, Math.min(1, v / 100));
}

export type CameraOverlayShape = "square" | "rectangle" | "rounded" | "circle";
export type CameraOverlayAnimationPreset = "none" | "soft" | "lively";
export type CameraMotionSource = "live-recorded" | "manual";

export interface CameraPlacement {
	x: number;
	y: number;
	width: number;
	height: number;
}

/** A camera position pinned at an original-recording time. The effective base
 *  placement glides (eased) between consecutive keyframes — the per-cut motion. */
export interface CameraKeyframe {
	atSec: number;
	placement: CameraPlacement;
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
	/** Grow + drift the camera away from a zoom's focus as it ramps in (default on). */
	zoomFollow: boolean;
	/** 0..1 strength of the zoom-follow grow + drift. */
	zoomFollowStrength: number;
	/** Seconds the grow/shrink takes to ramp in/out (its own transition timing). */
	zoomFollowDuration: number;
	/** Easing for the grow/shrink transition. */
	zoomFollowEasing: Easing;
	defaultPlacement: CameraPlacement;
	motionSegments: CameraMotionSegment[];
	/** Per-cut position keyframes (original-time). Empty → static defaultPlacement. */
	keyframes: CameraKeyframe[];
	/** Easing for the glide BETWEEN keyframes (the "animation smoothness"). */
	keyframeEasing: Easing;
	/** Drop-shadow strength 0..1 (0 = none). Scales blur + offset + opacity together. */
	shadow: number;
}

/**
 * The 8 standard camera-bubble positions plus `custom` for free-drag.
 * Used by `CameraPanel` for the preset chip row, and by
 * `cameraPresetFromPlacement` to identify which chip should be highlighted
 * after a drag-snap.
 */
export type CameraPositionPreset =
	| "top-left"
	| "top-center"
	| "top-right"
	| "left-center"
	| "right-center"
	| "bottom-left"
	| "bottom-center"
	| "bottom-right"
	| "custom";

/** Default size (16% of frame) and inset (2% margin) for preset placements. */
export const CAMERA_DEFAULT_SIZE = 0.16;
export const CAMERA_PRESET_INSET = 0.02;

/**
 * Resolve a preset name to a normalized {x, y, width, height}. `width` is a
 * fraction of the video WIDTH; the bubble is square in *pixels* (matching the
 * export), so its UV height is `width * aspect` where `aspect = videoW/videoH`.
 * The vertical anchors (top/center/bottom) therefore use that UV height, not
 * `width`, or a preset on a wide 16:9 screen lands off the bottom. x/y are the
 * bubble's top-left in 0..1 UV.
 *
 * `custom` returns the bottom-right placement as a sane fallback; the panel
 * never invokes this with `custom` — that branch just satisfies the union.
 */
export function cameraPlacementFromPreset(
	preset: CameraPositionPreset,
	size: number = CAMERA_DEFAULT_SIZE,
	inset: number = CAMERA_PRESET_INSET,
	aspect: number = 1,
): CameraPlacement {
	const height = Math.min(1, size * aspect);
	const farX = 1 - size - inset;
	const centerX = (1 - size) / 2;
	const farY = Math.max(0, 1 - height - inset);
	const centerY = Math.max(0, (1 - height) / 2);
	if (preset === "custom") {
		return { x: farX, y: farY, width: size, height };
	}
	// The preset ids mix conventions ('top-left' is row-col but 'left-center' is
	// col-row), so detect each axis by token rather than by split position — else
	// 'left-center'/'right-center' resolve to the wrong cell.
	const tokens = preset.split("-");
	const x = tokens.includes("left") ? inset : tokens.includes("right") ? farX : centerX;
	const y = tokens.includes("top") ? inset : tokens.includes("bottom") ? farY : centerY;
	return { x, y, width: size, height };
}

/**
 * Inverse of `cameraPlacementFromPreset`: find which preset (if any) the
 * given placement matches within a 0.5% tolerance. `aspect` must match the one
 * used to build the placement (video width/height) so the vertical anchors line
 * up. Returns `custom` for free-drag positions. Highlights the active chip.
 */
export function cameraPresetFromPlacement(
	p: CameraPlacement,
	aspect: number = 1,
): CameraPositionPreset {
	const presets: CameraPositionPreset[] = [
		"top-left",
		"top-center",
		"top-right",
		"left-center",
		"right-center",
		"bottom-left",
		"bottom-center",
		"bottom-right",
	];
	const tolerance = 0.005;
	for (const preset of presets) {
		const ref = cameraPlacementFromPreset(preset, p.width, CAMERA_PRESET_INSET, aspect);
		if (Math.abs(p.x - ref.x) < tolerance && Math.abs(p.y - ref.y) < tolerance) {
			return preset;
		}
	}
	return "custom";
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
	clampFramePaddingPercent,
	framePaddingPixels,
	MAX_FRAME_PADDING_PERCENT,
	normalizeFramePaddingPercent,
} from "./frame-padding";

export {
	clampFramePaddingPercent,
	framePaddingPixels,
	MAX_FRAME_PADDING_PERCENT,
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
	/** `id` included: Rust's `Annotation.id` has no `#[serde(default)]`, and a
	 *  missing one fails the WHOLE RenderState deserialize, not just that entry. */
	annotations: Annotation[];
	shadow: ShadowSettings;
	audioSettings: AudioSettings;
	/** Music / extra-audio clips on the output timeline. Optional for back-compat. */
	musicClips?: AudioClip[];
	cameraOverlay: CameraOverlaySettings;
	/**
	 * Editor layout mode (`auto` / column-stacked variants etc.). Optional
	 * for backwards compatibility: pre-field projects keep their default.
	 */
	layoutMode?: LayoutMode;
	// Hybrid-raster cursor sprite, populated only on the export path
	// (the editor route runs `rasterizeCursorSprites` right before invoking
	// `enqueue_export`). Not persisted to disk; never set by `loadRenderState`.
	cursorSpriteRest?: string; // data:image/png;base64,…
	cursorSpritePress?: string; // optional; falls back to rest in Rust
	cursorSpriteRightPress?: string; // optional; falls back to press → rest in Rust
	cursorSpriteDrag?: string; // optional; falls back to press → rest in Rust
	cursorSpriteHotspotRest?: [number, number]; // 0..1 sprite UV
	cursorSpriteHotspotPress?: [number, number];
	cursorSpriteHotspotRightPress?: [number, number];
	cursorSpriteHotspotDrag?: [number, number];
	cursorSpriteSizePx?: number; // sprite render size in source pixels
}

export type ExportFormat = "mp4" | "gif" | "webm";
export type ExportQuality = "small" | "hd" | "4k" | "source";
/** Encoder effort axis, orthogonal to {@link ExportQuality} (resolution).
 *  'balanced' reproduces the historical encoder settings exactly. */
export type ExportSpeed = "fast" | "balanced" | "quality";

/** GIF dithering algorithm. Trades file size against gradient quality. */
export type GifDither = "bayer" | "sierra2" | "none";
/** GIF quality preset: controls palette size + dither bias. */
export type GifQuality = "low" | "medium" | "high";
/** GIF loop behavior. `infinite` writes Netscape loop=0, `once` writes loop=-1, `n` writes loop=n. */
export type GifLoop = "infinite" | "once" | number;

export interface GifSettings {
	/** Output frame rate. `null` = inherit from quality profile. */
	fps: number | null;
	quality: GifQuality;
	loop: GifLoop;
	dither: GifDither;
}

export const DEFAULT_GIF_SETTINGS: GifSettings = {
	fps: null,
	quality: "medium",
	loop: "infinite",
	dither: "bayer",
};

export type LayoutMode = "auto" | "crop";

/**
 * Final-canvas aspect ratio. `source` keeps the canvas matched to the
 * source video plus padding (the v1 behaviour). The other values reframe
 * the final canvas to a target ratio: the source video stays centred,
 * and the chosen background fills the new horizontal/vertical bars.
 *
 * Strings are kept human-readable so they round-trip through the preset
 * picker (`preset.aspect`) and the project JSON without translation.
 */
export type OutputAspect = "source" | "16:9" | "9:16" | "1:1" | "1.91:1";

/** Parse an OutputAspect to a width/height ratio. Returns null for `source`. */
export function aspectRatio(a: OutputAspect): number | null {
	switch (a) {
		case "source":
			return null;
		case "16:9":
			return 16 / 9;
		case "9:16":
			return 9 / 16;
		case "1:1":
			return 1;
		case "1.91:1":
			return 1.91;
	}
}

export type EditorWindowBehavior = "navigate" | "new-window";

/** What the editor currently has selected. Exactly one, or nothing. */
export type SelectionKind = "clip" | "zoom" | "annotation" | "cut" | "music";
export interface EditorSelection {
	kind: SelectionKind;
	/** Segment start in original seconds for 'clip'; the entity id otherwise. */
	id: string | number;
}
export interface DeleteSelectionResult {
	kind: SelectionKind;
	/** Where to park the playhead after a clip delete; null for the others. */
	joinAt: number | null;
}

// Re-exported so the many existing `import type { PanelTab } from "…/editor-store"`
// sites keep working; the list itself lives in a module light enough to import
// from a unit test (see panel-tabs.ts).
export { PANEL_TABS, type PanelTab } from "./panel-tabs";

import type { PanelTab } from "./panel-tabs";

/** Active timeline pointer tool. `select` is the default (scrub/drag/select);
 *  `razor` arms the click-to-cut tool. A tool is state of the whole timeline,
 *  not of the focused element, so it lives here where every lane can read it and
 *  decline the gesture the tool owns. */
export type TimelineTool = "select" | "razor";

/** Timeline editing commands the route-level keyboard handler invokes so the
 *  S/C/I/O/Home/End keys work without the timeline scroller holding DOM focus.
 *  The timeline registers these on mount (it owns the frame-quantize math); the
 *  set is null while the timeline is collapsed/unmounted, so the keys no-op. */
export interface TimelineCommands {
	splitAtPlayhead: () => void;
	toggleRazor: () => void;
	exitTool: () => void;
	trimToPlayhead: (kind: "in" | "out") => void;
	seekToEdge: (which: "in" | "out") => void;
}

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
 * Values live in `@recast/design/backgrounds` so the screenshot editor shares them.
 */
export const GRADIENT_PRESETS: BackgroundPreset[] = BACKGROUND_GRADIENTS;

/** Default gradient used when a fresh custom gradient is created. */
export const DEFAULT_GRADIENT = GRADIENT_PRESETS[0].value;

function clampNum(v: number, lo: number, hi: number): number {
	return Math.min(hi, Math.max(lo, v));
}

/** Expand #rgb/#rgba shorthand and lowercase, so downstream parsing is uniform. */
function normalizeHex(hex: string): string {
	let h = hex.trim().replace(/^#/, "");
	if (h.length === 3 || h.length === 4) {
		h = h
			.split("")
			.map((c) => c + c)
			.join("");
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
	const angle = angleMatch ? ((parseFloat(angleMatch[1]) % 360) + 360) % 360 : 135;

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
		return {
			angle,
			stops: [
				{ color: "#6366f1", pos: 0 },
				{ color: "#d946ef", pos: 100 },
			],
		};
	}
	if (raw.length === 1) {
		return {
			angle,
			stops: [
				{ color: raw[0].color, pos: 0 },
				{ color: raw[0].color, pos: 100 },
			],
		};
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
	const angle = ((Math.round(spec.angle) % 360) + 360) % 360;
	const body = [...spec.stops]
		.sort((a, b) => a.pos - b.pos)
		.map((s) => `${normalizeHex(s.color)} ${Math.round(clampNum(s.pos, 0, 100))}%`)
		.join(", ");
	return `linear-gradient(${angle}deg, ${body})`;
}

/** Solid backdrop presets. Labelled (not a bare hex list) so the picker and
 *  screen readers can name a swatch. */
export const COLOR_PRESETS: BackgroundPreset[] = BACKGROUND_COLORS;

export function generateId(): string {
	return Math.random().toString(36).substring(2, 9);
}

export type { CaptionAnimation, CaptionPreset, CaptionStyle };
/**
 * Creates an editor store instance.
 * Call once per editor page mount, or use a singleton.
 */
// Re-export the caption model (imported at the top) so modules that import it
// from `editor-store` keep working.
export { CAPTION_PRESETS, DEFAULT_CAPTION_ANIMATION, DEFAULT_CAPTION_STYLE };

/** What to do with generated captions on export. Independent choices: you can
 *  burn captions into the pixels AND keep a sidecar file. The sidecar is also
 *  what Cloud uploads as a selectable caption track. */
export interface CaptionExportOptions {
	/** Burn the captions into the video (overlay). Ignored for GIF. */
	burnIn: boolean;
	/** Write a separate subtitle file next to the export ('none' to skip). */
	sidecar: "none" | "vtt" | "srt";
}

export const DEFAULT_CAPTION_EXPORT: CaptionExportOptions = {
	burnIn: false,
	sidecar: "vtt",
};
