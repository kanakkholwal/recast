/**
 * The backend contract the editor speaks: the desktop app's IPC struct shapes.
 * Lives in the package so nothing here imports apps/desktop;
 * `apps/desktop/src/lib/ipc-types.ts` re-exports it, keeping one definition.
 */

import type { CaptionAnimation } from "@recast/captions";
import type { EditorRenderState, Transcript, VideoMetadata } from "./editor/render-state";
import type { RecordingProfile } from "./profiles";

export type {
	EditorRenderState,
	Transcript,
	TranscriptSegment,
	TranscriptWord,
	VideoMetadata,
} from "./editor/render-state";

export type CameraCapture = "separate" | "off" | "failed" | "legacy";

export interface AudioDeviceInfo {
	id: string;
	name: string;
	isDefault: boolean;
}

export interface ExportGifSettings {
	fps: number | null;
	quality: "low" | "medium" | "high";
	loop: "infinite" | "once" | number;
	dither: "bayer" | "sierra2" | "none";
}

/** Encoder effort axis, orthogonal to `quality` (resolution). "balanced"
 *  reproduces the historical encoder settings exactly. */
export type ExportSpeed = "fast" | "balanced" | "quality";

/** Everything the backend queue needs to run one export. Built in the browser
 *  (render state is rasterized there); handed off via {@link enqueueExport}. */
export type ZoomSuggestionReason = "click" | "settleAfterMotion";

export interface ZoomSuggestion {
	timestampUs: number;
	x: number;
	y: number;
	reason: ZoomSuggestionReason;
	/** Confidence in [0,1]: how strongly this moment warrants a zoom. */
	score?: number;
}

/** Tunable thresholds for `detectSilence`; omit any field to use the default. */
export interface SilenceDetectOptions {
	/**
	 * Speech-probability threshold in [0,1]: a frame counts as speech once the
	 * voice-activity model scores at or above it, so everything below is
	 * silence. Higher = more aggressive (more gets called silence).
	 */
	threshold?: number;
	/** Minimum continuous non-speech run (seconds). */
	minAudioSilence?: number;
	/** Minimum length of a returned silence segment (seconds). */
	minSegment?: number;
}

/** A detected silence range, in original-recording seconds. */
export interface SilenceSegment {
	start: number;
	end: number;
	/** 0..1: how strongly this range warrants a cut. */
	confidence: number;
	micSilent: boolean;
	systemSilent: boolean;
	/** Cursor track was present and confirmed idle over the range. */
	cursorIdle: boolean;
}

/** How a model runs. `ggml` is the on-device engine (transcribe.cpp, any GGUF
 *  model family); `remote` posts to an OpenAI-compatible endpoint the server owns.
 *  The GGUF file decides the architecture, so the family (Parakeet / Whisper) is
 *  display metadata (`family`), not a separate engine. */
export type CaptionEngine = "ggml" | "remote";

/** Inference backend (the availability axis). Mirrors `CaptionEngine`: `ggml`
 *  ships in a default build; `remote` posts to an OpenAI-compatible endpoint. */
export type CaptionRuntime = "ggml" | "remote";

/** Where a catalog entry came from: the built-in list, an installed pack, or a
 *  user-configured remote endpoint. */
export type CaptionModelSource = "builtin" | "extension" | "remote";

export interface CaptionModelInfo {
	id: string;
	displayName: string;
	engine: CaptionEngine;
	/** Inference backend (derived from `engine`); the availability axis. */
	runtime: CaptionRuntime;
	/** Built-in vs. contributed by an installed extension (provenance badge). */
	source: CaptionModelSource;
	/** Display group for the picker, e.g. "Parakeet" / "Whisper". */
	family: string;
	languages: string[];
	approxSizeBytes: number | null;
	isDefault: boolean;
	installed: boolean;
	/** False for a model with no files defined (e.g. a remote endpoint). */
	downloadable: boolean;
	requiresGpu: boolean;
	prefersGpu: boolean;
	minRamBytes: number | null;
	/** False → this device can't run the model (hard-disabled in the UI). */
	runnable: boolean;
	/** False → this model's runtime isn't usable in this build (on-device engine
	 *  not compiled in, or a remote endpoint has no key). Download stays allowed;
	 *  only Generate is gated on this. */
	runtimeAvailable: boolean;
	/** Non-blocking device caveat (slow on CPU, low RAM, …), or the reason the
	 *  runtime is unavailable when that's the blocker. */
	warning: string | null;
	/** What the model can do beyond plain transcription. Presentation only —
	 *  nothing here changes how the engine is invoked. */
	capabilities: CaptionModelCapabilities;
	/** How many languages the model covers. `languages` carries `["multi"]`
	 *  rather than 99 entries, so the count is separate. Null when unknown
	 *  (extension packs, remote endpoints). */
	languageCount: number | null;
	/** Relative speed / accuracy, 0-100, for the picker's comparison bars.
	 *  Editorial values for ranking models against each other, not benchmarks.
	 *  Null when unknown. */
	speedScore: number | null;
	accuracyScore: number | null;
	/** Surfaced with a "Recommended" tag in the picker. */
	recommended: boolean;
}

/** Model abilities beyond plain same-language transcription. Mirrors
 *  `ModelCapabilities` in `transcription/models.rs`. */
export interface CaptionModelCapabilities {
	/** Emits partial results as audio arrives (vs. one result at the end). */
	streaming: boolean;
	/** Can transcribe speech into a different language. */
	translate: boolean;
	/** Detects the spoken language rather than needing it declared. */
	langDetect: boolean;
	/** How precisely the model locates its text in time. `"none"` cannot drive
	 *  captions at all — every built-in is checked against this in Rust. */
	timestamps: CaptionTimestampGranularity;
}

/** How precisely a model reports WHEN each piece of text was said. Mirrors
 *  `TimestampGranularity` in `transcription/models.rs`. */
export type CaptionTimestampGranularity = "none" | "segment" | "token" | "word";

/** One weight file of a pack-contributed caption model. Third-party weights
 *  MUST pin a sha256 (built-ins may leave it null until a revision is locked). */
export interface GpuInfo {
	available: boolean;
	/** "metal" | "cuda" | null (CPU mode). */
	backend: string | null;
	name: string | null;
}

export interface DeviceCapabilities {
	os: string;
	arch: string;
	totalRamBytes: number | null;
	gpu: GpuInfo;
	/** Whether the on-device caption engine (ggml / transcribe.cpp) is compiled
	 *  into this build. False in a `--no-default-features` build, where only remote
	 *  endpoints can transcribe. */
	captionsAvailable: boolean;
}

/** Progress tick for a caption-model download. `total` is 0 when the server
 *  didn't report a content length; `file` is empty on the terminal tick. */
export interface CaptionDownloadProgress {
	modelId: string;
	file: string;
	downloaded: number;
	total: number;
}

/** Coarse phase of a transcription run: "extracting" | "transcribing" | "done". */
export interface TranscribeProgress {
	phase: string;
}

/** One recognized text line, with a resolution-independent box. */
export interface ScreenElement {
	/** Stable within a span, for "element 7" style reference. */
	id: number;
	/** "text" today; "icon" once a detector is added. */
	kind: string;
	/** Normalized [x0, y0, x1, y1] in 0..1 of the frame. */
	bbox: [number, number, number, number];
	content: string;
	/** Engine that read it, e.g. "ocrs". */
	source: string;
}

/** A stretch of time over which the screen text stayed effectively the same. */
export interface ScreenStateSpan {
	/** Seconds on the video clock. */
	start: number;
	end: number;
	elements: ScreenElement[];
	/** Small JPEG data URI, only when previews were requested. */
	preview?: string | null;
}

/** Counters and per-stage timings for one read, so a human reviewing the output can
 *  see the work behind it rather than being handed spans with no provenance. */
export interface OcrStats {
	/** Video length in seconds, per ffprobe. */
	durationSecs: number;
	/** Coarse frames the decode pass walked. */
	framesScanned: number;
	/** Frames that survived the change gate and were actually OCR'd. */
	framesRead: number;
	/** Total recognized elements across every span. */
	elements: number;
	/** Decode + change-gate pass. */
	sampleMs: number;
	/** One-time model load. */
	modelLoadMs: number;
	/** The OCR pass itself, which dominates the rest by a wide margin. */
	ocrMs: number;
}

export interface VideoTextTimeline {
	engine: string;
	spans: ScreenStateSpan[];
	stats: OcrStats;
}

/** Phase of an OCR run: "downloading" | "sampling" | "reading" | "done". */
export type OcrPhase = "downloading" | "sampling" | "reading" | "done";

/**
 * Counted progress of a read. The units of `done`/`total` are whatever the phase
 * counts: bytes while downloading, coarse frames while sampling, OCR'd frames while
 * reading. A `total` of 0 means the phase cannot be counted yet, so show an
 * indeterminate bar rather than dividing by it.
 */
export interface OcrProgress {
	phase: OcrPhase;
	done: number;
	total: number;
	/** The result so far: frames kept while sampling, screen states found while reading. */
	found: number;
}

/** A user-configured remote endpoint. Non-secret: the API key is NOT here (it
 *  lives in the OS keyring and never crosses IPC). */
export interface AssetInstallFailure {
	id: string;
	reason: string;
}

export interface HydratedAsset {
	id: string;
	path: string | null;
	thumbPath: string | null;
}

export interface AssetInstallResult {
	installed: string[];
	skipped: string[];
	failed: AssetInstallFailure[];
	cacheDir: string;
	hydrated: HydratedAsset[];
}

/** A manifest-local asset (downloaded + sha256-verified by the installer). */
export interface ExtensionAssetEntry {
	id: string;
	filename: string;
	url: string;
	sha256: string;
	size?: number | null;
	version?: string | null;
	thumbFilename?: string | null;
	thumbUrl?: string | null;
	thumbSha256?: string | null;
}

export interface ExtCursorContribution {
	id: string;
	label: string;
	description?: string;
	/** Manifest-local asset id of the rest-state SVG. */
	rest: string;
	/** Manifest-local asset id of the optional pressed-state (left-click) SVG. */
	press?: string;
	/** Manifest-local asset id of the optional right-click SVG. */
	rightPress?: string;
	/** Manifest-local asset id of the optional drag SVG. */
	drag?: string;
	hotspot: { x: number; y: number };
	pressedHotspot?: { x: number; y: number };
	rightPressedHotspot?: { x: number; y: number };
	dragHotspot?: { x: number; y: number };
}

export interface ExtBackgroundContribution {
	id: string;
	label: string;
	/** Manifest-local asset id of the full-resolution image. */
	asset: string;
	/** Optional manifest-local asset id of a thumbnail. */
	thumb?: string;
}

export interface ExtGradientContribution {
	id: string;
	label: string;
	/** CSS `linear-gradient(...)` string. */
	value: string;
}

export interface ExtColorContribution {
	id: string;
	label: string;
	/** Hex colour. */
	value: string;
}

export interface ExtEasingContribution {
	id: string;
	label: string;
	value: { x1: number; y1: number; x2: number; y2: number };
}

export interface ExtSmoothingContribution {
	id: string;
	label: string;
	smoothing: number;
	snapToClicks: boolean;
	snapWindowMs: number;
}

/** A caption theme contributed by a pack: the visual fields of a caption
 *  style. Mirrors the built-in `CaptionPreset.style` shape. */
export interface ExtCaptionPresetContribution {
	id: string;
	label: string;
	description?: string;
	fontFamily: string;
	fontWeight: number;
	fontSizePct: number;
	position: "top" | "center" | "bottom";
	align: "left" | "center" | "right";
	offsetPct: number;
	color: string;
	uppercase: boolean;
	letterSpacing: number;
	background: "none" | "soft" | "box";
	backgroundColor: string;
	backgroundOpacity: number;
	outlineWidth: number;
	outlineColor: string;
	maxLines: number;
	// New pill/highlight fields, optional so packs authored before them still
	// load (the registry mapping fills defaults from DEFAULT_CAPTION_STYLE).
	mutedColor?: string;
	boxPaddingXEm?: number;
	boxPaddingYEm?: number;
	boxRadiusEm?: number;
	lineHeight?: number;
	maxCharsPerLine?: number;
	/** Optional word-by-word animation. */
	animation?: CaptionAnimation;
}

export interface ExtensionContributions {
	cursors?: ExtCursorContribution[];
	backgrounds?: ExtBackgroundContribution[];
	gradients?: ExtGradientContribution[];
	colors?: ExtColorContribution[];
	easings?: ExtEasingContribution[];
	smoothings?: ExtSmoothingContribution[];
	captionPresets?: ExtCaptionPresetContribution[];
}

export interface ExtensionManifest {
	id: string;
	name: string;
	version: string;
	author?: string | null;
	kind: string;
	permissions: string[];
	signature?: string | null;
	contributes: ExtensionContributions;
	assets: ExtensionAssetEntry[];
}

/** Resolved on-disk location for one manifest-local asset id. */
export interface ExtAssetPath {
	id: string;
	path: string | null;
	thumbPath: string | null;
}

export interface InstalledExtension {
	manifest: ExtensionManifest;
	enabled: boolean;
	dir: string;
	assets: ExtAssetPath[];
}

/** What the panel should preselect on open. Sent from the home mode tiles. */
