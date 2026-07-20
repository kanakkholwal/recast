/**
 * Types mirroring the Rust IPC structs. Split out of `ipc.ts` so consumers
 * that only need the shapes don't pull in `invoke` and the Tauri runtime.
 */

import type { CaptionAnimation } from "@recast/captions";
import type { RecordingProfile } from "$lib/profiles";
import type { EditorRenderState, VideoMetadata } from "$lib/stores/editor-store.svelte";

export interface DisplayInfo {
	id: number;
	name: string;
	x: number;
	y: number;
	width: number;
	height: number;
	isPrimary: boolean;
	thumbnail: string | null;
	/** Monitor refresh rate in Hz (rounded); 0 if the OS couldn't report it. */
	refreshHz: number;
}

export interface WindowInfo {
	id: number;
	pid: number;
	appName: string;
	title: string;
	x: number;
	y: number;
	width: number;
	height: number;
	isMinimized: boolean;
	thumbnail: string | null;
}

export interface RecordingEntry {
	filename: string;
	path: string;
	sizeBytes: number;
	/** Birth time (file creation), epoch seconds. Falls back to `modified` on
	 * filesystems where birth time isn't reported. Labels the recording date. */
	created: number;
	/** Last-modified time, epoch seconds. Drives "what was I last editing"
	 * surfaces like the library Continue card. */
	modified: number;
	/** `.recast` only: a legacy bundle that must be migrated before editing. */
	needsMigration: boolean;
}

export interface EditorDocument {
	projectPath: string;
	mediaPath: string;
	cursorPath?: string | null;
	editsPath?: string | null;
	audioPath?: string | null;
	microphonePath?: string | null;
	cameraPath?: string | null;
	metadata: VideoMetadata;
	renderState: EditorRenderState;
	/** True for a legacy bundle: migrate before loading the editor. */
	needsMigration: boolean;
}

export interface AutosaveState {
	projectPath: string;
	savedAtUnixMs: number;
	editsJson: string;
}

/** One encoder candidate (H.264 or HEVC) and whether it really initializes
 *  here. Mirrors the Rust `EncoderAvailability` struct (`probe_video_encoders`). */
export interface EncoderAvailability {
	name: string;
	label: string;
	vendor: string;
	/** Codec family ("H.264" or "HEVC"), used to group the matrix. */
	family: string;
	hardware: boolean;
	available: boolean;
	active: boolean;
}

/** ffmpeg/ffprobe resolution + codec diagnostics. Mirrors the Rust
 *  `FfmpegDiagnostics` struct (`diagnose_ffmpeg`). */
export interface FfmpegDiagnostics {
	ffmpeg_path: string;
	ffprobe_path: string;
	version: string | null;
	h264_encoder: string;
	encoders_present: string[];
	encoders_missing: string[];
}

/** One capture-input capability and whether this device's native API supports
 *  it. Mirrors the Rust `CaptureCapability` struct (`capture_capabilities`). */
/** Refines the `supported: false` case so the UI can say the right thing:
 *  `unsupported` → the OS can't do it; `planned` → we haven't shipped it yet. */
export type CapabilityStatus = "supported" | "unsupported" | "planned";

export interface CaptureCapability {
	/** "screen" | "window" | "region" | "systemAudio" | "microphone" |
	 *  "camera" | "cursor". */
	key: string;
	label: string;
	supported: boolean;
	/** Tri-state refinement of `supported`; see `CapabilityStatus`. */
	status: CapabilityStatus;
	/** Native API in use, e.g. "DXGI Desktop Duplication", "FFmpeg AVFoundation". */
	backend: string;
	note: string | null;
}

/** Capture-support matrix for the current OS. Mirrors the Rust
 *  `CaptureCapabilities` struct (`capture_capabilities`). */
export interface CaptureCapabilities {
	platform: string;
	screenBackend: string;
	capabilities: CaptureCapability[];
}

/** Backend-owned staged selection for the next recording. Mirrors the Rust
 *  `CaptureIntent` struct (`commands/intent.rs`). The CLI mutates it and the
 *  panel renders it; `capture-intent:changed` fires on every edit. */
export interface CaptureIntentState {
	/** "display" | "window" | "region"; absent until a source is chosen. */
	targetType?: string | null;
	targetId: number;
	region?: RegionRect | null;
	options: RecordingOptions;
	countdown?: number | null;
	activeProfileId?: string | null;
}

/** Backend-owned recording profiles snapshot. Mirrors the Rust `ProfilesSnapshot`
 *  (`commands/profiles.rs`). `initialized` is false while the backend holds only
 *  the in-memory seed, which the store uses to migrate `localStorage` once. */
export interface ProfilesSnapshot {
	profiles: RecordingProfile[];
	enabled: boolean;
	initialized: boolean;
}

/** Whether the `recast` CLI resolves as a bare terminal command. Mirrors the
 *  Rust `InstallStatus` (`cli_install_status`). */
export interface CliInstallStatus {
	onPath: boolean;
	binDir: string;
	detail: string;
	modifiedRcFiles?: string[];
}

export interface RecordingOptions {
	systemAudio?: boolean;
	microphone?: boolean;
	microphoneDeviceId?: string | null;
	camera?: boolean;
	cameraDeviceId?: string | null;
	/** Capture frame rate. Omitted/out-of-range (24–240) → backend default 60. */
	fps?: number | null;
	/** Capture quality tier: "auto" (default: backend picks high on a hardware
	 *  encoder, balanced on software), or explicit "balanced"/"high"/"pristine". */
	quality?: "auto" | "balanced" | "high" | "pristine" | null;
}

export interface AudioDeviceInfo {
	id: string;
	name: string;
	isDefault: boolean;
}

export interface CameraDeviceInfo {
	id: string;
	name: string;
	status?: "ready" | "warning" | "error" | "unknown";
	statusMessage?: string | null;
}

export interface CameraValidationResult {
	id: string;
	name: string;
	status: "ready" | "warning" | "error" | "unknown";
	statusMessage?: string | null;
	probedAtUnixMs: number;
}

export interface CameraPreviewState {
	mirror: boolean;
	shape: "square" | "rectangle" | "rounded" | "circle";
	cornerRadius: number;
	animationPreset: "none" | "soft" | "lively";
	windowX: number;
	windowY: number;
	windowWidth: number;
	windowHeight: number;
}

export interface RecordingStartResult {
	warnings: string[];
}

export interface RegionRect {
	x: number;
	y: number;
	width: number;
	height: number;
}

export interface LastSource {
	kind: "monitor" | "window" | "region";
	id: number;
	label: string;
	regionX?: number | null;
	regionY?: number | null;
	regionWidth?: number | null;
	regionHeight?: number | null;
}

/** Result of a successful cloud upload + share-link creation. */
export interface CloudShareResult {
	recastId: string;
	slug: string;
	shareUrl: string;
}

/** Local manifest entry: a local export that has a cloud copy. */
export interface CloudUploadRecord {
	recastId: string;
	slug: string;
	shareUrl: string;
	uploadedAt: number;
}

/** Live progress for an in-flight cloud upload, streamed on the request-scoped
 *  channel. Terminal states aren't here: success is the resolved
 *  {@link CloudShareResult}, failure the rejection. */
export type CloudUploadEvent =
	| { kind: "phase"; phase: CloudPhase }
	| { kind: "progress"; bytesSent: number; totalBytes: number };

/** Phase strings the upload streams, in order. */
export type CloudPhase = "preparing" | "uploading" | "finalizing" | "sharing";

/** A single share link for a recast, as returned by `recast_cloud_list_shares`.
 *  The Rust command passes the server's JSON through verbatim (`serde_json::Value`),
 *  so this types the subset the manage UI actually consumes rather than the full
 *  server payload. `visibility` stays a `string` (not a union) because the server
 *  is the source of truth; the UI normalizes it with its own `toVisibility`. */
export interface CloudShareLink {
	slug: string;
	visibility: string;
	hasPassword: boolean;
	expiresAt: string | null;
	viewsCount: number;
}

/** Response of `recast_cloud_list_shares`. `shares` is absent on a recast that
 *  has no links yet. */
export interface CloudShareList {
	shares?: CloudShareLink[];
}

export type ExportStateEvent =
	| { exportId: string; status: "started" }
	| { exportId: string; status: "preparing"; detail?: string }
	| { exportId: string; status: "progress"; progress: number }
	| { exportId: string; status: "finalizing" }
	| { exportId: string; status: "success"; path: string }
	| { exportId: string; status: "cancelled" }
	| { exportId: string; status: "error"; message: string };

export interface ExportGifSettings {
	fps: number | null;
	quality: 'low' | 'medium' | 'high';
	loop: 'infinite' | 'once' | number;
	dither: 'bayer' | 'sierra2' | 'none';
}

/** Encoder effort axis, orthogonal to `quality` (resolution). "balanced"
 *  reproduces the historical encoder settings exactly. */
export type ExportSpeed = "fast" | "balanced" | "quality";

/** Everything the backend queue needs to run one export. Built in the browser
 *  (render state is rasterized there); handed off via {@link enqueueExport}. */
export interface EnqueueExportRequest {
	inputPath: string;
	format: string;
	quality: string;
	renderState: EditorRenderState;
	exportId: string;
	gifSettings?: ExportGifSettings;
	speed?: ExportSpeed;
	/** Output frame rate for MP4/WebM. `null`/omitted keeps the source rate. */
	fps?: number | null;
	/** Burn the generated captions into the video. No-op without a transcript. */
	burnCaptions?: boolean;
	/** Subtitle sidecar to write next to the export on success, or null. */
	captionSidecar?: { format: "vtt" | "srt"; transcript: Transcript } | null;
}

/** A queue row as the backend reports it (source of truth for the activity UI). */
export interface ExportJobDto {
	id: string;
	filename: string;
	/** Source project path. */
	filePath: string;
	status: "queued" | "running" | "success" | "error" | "cancelled" | "interrupted";
	phase: "preparing" | "encoding" | "finalizing" | "cancelling";
	progress: number;
	/** Output path once it succeeds. */
	path?: string | null;
	error?: string | null;
	createdAt: number;
	startedAt?: number | null;
	finishedAt?: number | null;
}

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
export interface CaptionModelPackFile {
	relPath: string;
	url: string;
	sha256: string;
}

/**
 * A `contributes.captionModels[]` entry in an `asset-pack` extension manifest.
 * `runtime` + `engine` are closed allowlists validated in Rust; `engine` must
 * belong to `runtime`. Weight files download into `models/<id>/`, so a pack can
 * only reuse an existing backend, never introduce one.
 */
export interface CaptionModelContribution {
	id: string;
	displayName: string;
	runtime: CaptionRuntime;
	engine: CaptionEngine;
	family: string;
	languages: string[];
	approxSizeBytes?: number | null;
	files: CaptionModelPackFile[];
	requiresGpu?: boolean;
	prefersGpu?: boolean;
	minRamBytes?: number | null;
}

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
export interface RemoteAsrEndpoint {
	/** Stable slug; doubles as the keyring entry suffix and catalog id. */
	id: string;
	displayName: string;
	/** Base URL up to (not including) `/audio/transcriptions`. */
	baseUrl: string;
	/** Model name the endpoint expects, e.g. `whisper-large-v3`. */
	model: string;
	languages: string[];
}

/** A configured endpoint plus whether its API key is stored. */
export interface RemoteAsrEndpointInfo extends RemoteAsrEndpoint {
	hasKey: boolean;
}

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
export type CaptureIntent = "screen" | "window" | "region" | "camera";

export interface AuthPlan {
	id: string;
	name: string;
	status: string;
	currentPeriodEnd: string | null;
	cancelAtPeriodEnd: boolean;
}

export interface AuthUsage {
	recordings: number;
	storageBytes: number;
	activeShares: number;
	sharesLimit: number | null;
}

/** A workspace the signed-in user can upload into. Mirrors the Rust `Workspace`. */
export interface CloudWorkspace {
	id: string;
	name: string;
	/** "owner" | "admin" | "member". */
	role: string;
	/** "free" | "pro" | "enterprise": the org's plan. */
	plan: string;
	/** Live (non-deleted) recast count in the workspace. */
	recastsCount: number;
}

/** Full sign-in snapshot from `auth_status`. Consumers read the subset they
 *  need (profile card vs. share-flow guard). */
export interface AuthStatus {
	signedIn: boolean;
	email: string | null;
	name: string | null;
	image: string | null;
	memberSince: string | null;
	plan: AuthPlan | null;
	usage: AuthUsage | null;
	workspaces: CloudWorkspace[];
	defaultWorkspaceId: string | null;
}

/** Result of `auth_start` (device-authorization flow kickoff). NOTE: the Rust
 *  `AuthStartResult` is NOT `rename_all = camelCase`, so its fields stay
 *  snake_case on the wire. */
export interface AuthStartResult {
	user_code: string;
	verification_uri: string;
	expires_in: number;
}

/** Self-hosting cloud endpoint config (`get_cloud_api_config` / `set_cloud_api_url`). */
export interface CloudApiConfig {
	effective: string;
	overrideUrl: string | null;
	defaultUrl: string;
	isCustom: boolean;
}

export interface GdriveStatus {
	connected: boolean;
	email?: string | null;
}

/** Result of a successful Drive upload. */
export interface GdriveUploadResult {
	fileId: string;
	name: string;
	webViewLink?: string;
}

/** Persisted record of a prior Drive upload, keyed by local export path.
 *  Mirrors the Rust `UploadRecord` from `commands/gdrive.rs`. */
export interface GdriveUploadRecord {
	fileId: string;
	name: string;
	webViewLink?: string;
	/** Unix seconds. */
	uploadedAt: number;
}

/** Byte progress for an in-flight Drive upload, streamed on the request-scoped
 *  channel (one per upload → no id to correlate). */
export interface GdriveUploadProgress {
	bytesSent: number;
	totalBytes: number;
}
