/**
 * Recorder + host IPC payload types: the wire contract between `$lib/ipc` and
 * the Rust commands the EDITOR never reads — capture sources, recording
 * options, cloud/Drive/auth, the export queue, CLI install.
 *
 * These lived in `@recast/editor` until 2026-08-09, which meant the recorder
 * couldn't change its wire contract without touching the editor package, and
 * `@recast/editor` couldn't serve a non-Tauri host without dragging ~400 lines
 * of Tauri-only types along. Editor-facing payloads still live in
 * `@recast/editor/lib/wire-types`.
 */

import type { TrackOffsetsWire } from "@recast/editor";
import type { RecordingProfile } from "@recast/editor/lib/profiles";
import type { ExportTimeSpan } from "@recast/editor/lib/services/export";
import type {
	CameraCapture,
	CaptionEngine,
	CaptionRuntime,
	EditorRenderState,
	ExportGifSettings,
	ExportSpeed,
	Transcript,
	VideoMetadata,
} from "@recast/editor/lib/wire-types";

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

/**
 * How the face camera was captured. The camera is always recorded to its own
 * track and composited at export, so the overlay stays editable; this says
 * whether that happened and, when it didn't, why.
 *
 * `failed` is "requested but no track arrived" (device busy, permission denied).
 * `legacy` is a bundle written before the backend recorded capture metadata —
 * unknowable. Neither is folded into `off`: both would blame the user for a
 * toggle they either did set or never had.
 */
export interface EditorDocument {
	projectPath: string;
	mediaPath: string;
	cursorPath?: string | null;
	editsPath?: string | null;
	audioPath?: string | null;
	microphonePath?: string | null;
	cameraPath?: string | null;
	/** Why `cameraPath` is or isn't set. Absent on documents from an older
	 *  backend, which the editor reads as `legacy` (unknowable, not "off"). */
	cameraCapture?: CameraCapture;
	/** Measured lag of each companion track behind video frame 0. Absent on
	 *  bundles recorded before offsets were measured. */
	trackOffsets?: TrackOffsetsWire;
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
	/** Path to a browser-rendered, already-composited video (Phase 4). When set,
	 *  the job mux-copies it instead of running the Rust filter_complex compositor. */
	browserVideoPath?: string | null;
	/** The editor's resolved kept-timeline. The backend replays it instead of
	 *  re-deriving one from cuts + splits + speed anchors. */
	timeMap?: ExportTimeSpan[] | null;
}

/** A queue row as the backend reports it (source of truth for the activity UI). */
export interface ExportJobDto {
	id: string;
	filename: string;
	/** Source project path. */
	filePath: string;
	status: "queued" | "running" | "success" | "error" | "cancelled" | "interrupted";
	/** Only what `export_queue.rs` actually writes. "rendering"/"encoding" are
	 *  synthesized frontend-side — see `ExportItemPhase` in exportActivity. */
	phase: "preparing" | "finalizing" | "cancelling";
	progress: number;
	/** Output path once it succeeds. */
	path?: string | null;
	error?: string | null;
	createdAt: number;
	startedAt?: number | null;
	finishedAt?: number | null;
}

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
	/** Rust sends `Option<String>` with no `skip_serializing_if`, so an absent
	 *  link arrives as `null` — same convention as `GdriveStatus.email`. */
	webViewLink?: string | null;
}

/** Persisted record of a prior Drive upload, keyed by local export path.
 *  Mirrors the Rust `UploadRecord` from `commands/gdrive.rs`. */
export interface GdriveUploadRecord {
	fileId: string;
	name: string;
	webViewLink?: string | null;
	/** Unix seconds. */
	uploadedAt: number;
}

/** Byte progress for an in-flight Drive upload, streamed on the request-scoped
 *  channel (one per upload → no id to correlate). */
export interface GdriveUploadProgress {
	bytesSent: number;
	totalBytes: number;
}
