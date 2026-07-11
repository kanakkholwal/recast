/**
 * Typed IPC wrappers for Tauri backend commands. All `invoke()` calls route
 * through here so argument/return types live in one place.
 */

import type { EditorRenderState, VideoMetadata } from "$lib/stores/editor-store.svelte";
import type { CaptionAnimation } from "$lib/captions/animation";
// Type-only: erased at runtime, so no ESM cycle with `$lib/profiles` (which
// imports value bindings from here).
import type { RecordingProfile } from "$lib/profiles";
import { analytics } from "$lib/analytics/client";
import { Channel, invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { platform } from "@tauri-apps/plugin-os";

// Some Linux compositors (KWin/Wayland) let an undecorated transparent
// always-on-top window trap input focus, breaking the main window's controls,
// so drop `alwaysOnTop` on Linux. Lazy, not a top-level `const`: calling
// `platform()` at module-eval time would make this module unsafe to import
// outside the Tauri webview (web/SSR builds import it but guard calls).
const isLinux = () => platform() === "linux";

// Types matching Rust structs

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
	created: number;
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

// System commands

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

/** Probe which video encoders actually work on this device (real init
 *  probe, not just "compiled in"). Each hardware probe spawns ffmpeg, so
 *  this can take up to ~2s cold, so call it off the render path. */
export function probeVideoEncoders(): Promise<EncoderAvailability[]> {
	return invoke<EncoderAvailability[]>("probe_video_encoders");
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

/** Report which capture inputs this device's native APIs support, computed
 *  from the running build's backend plus cheap runtime checks (macOS device
 *  listing, Linux session type). Powers Settings → "Capture support". */
export function captureCapabilities(): Promise<CaptureCapabilities> {
	return invoke<CaptureCapabilities>("capture_capabilities");
}

/** Resolved ffmpeg paths, version, and which export codecs are present. */
export function diagnoseFfmpeg(): Promise<FfmpegDiagnostics> {
	return invoke<FfmpegDiagnostics>("diagnose_ffmpeg");
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

/** Event name broadcast by the backend whenever the capture intent changes. */
export const CAPTURE_INTENT_CHANGED_EVENT = "capture-intent:changed";

/** Read the current staged capture intent. */
export function getCaptureIntent(): Promise<CaptureIntentState> {
	return invoke<CaptureIntentState>("get_capture_intent");
}

/** Replace the staged capture intent (broadcasts `capture-intent:changed`). */
export function setCaptureIntent(
	intent: CaptureIntentState,
): Promise<CaptureIntentState> {
	return invoke<CaptureIntentState>("set_capture_intent", { intent });
}

/** Backend-owned recording profiles snapshot. Mirrors the Rust `ProfilesSnapshot`
 *  (`commands/profiles.rs`). `initialized` is false while the backend holds only
 *  the in-memory seed, which the store uses to migrate `localStorage` once. */
export interface ProfilesSnapshot {
	profiles: RecordingProfile[];
	enabled: boolean;
	initialized: boolean;
}

/** Event broadcast by the backend whenever the saved profile set changes. */
export const RECORDING_PROFILES_CHANGED_EVENT = "recording-profiles:changed";

/** Read the backend-owned profile set. */
export function getProfiles(): Promise<ProfilesSnapshot> {
	return invoke<ProfilesSnapshot>("get_profiles");
}

/** Replace the backend-owned profile set (broadcasts `recording-profiles:changed`). */
export function setProfiles(
	profiles: RecordingProfile[],
	enabled: boolean,
): Promise<ProfilesSnapshot> {
	return invoke<ProfilesSnapshot>("set_profiles", { profiles, enabled });
}

/** Apply a saved profile (by id or name) to the staged capture intent. */
export function useProfile(id: string): Promise<CaptureIntentState> {
	return invoke<CaptureIntentState>("use_profile", { id });
}

/** Whether the `recast` CLI resolves as a bare terminal command. Mirrors the
 *  Rust `InstallStatus` (`cli_install_status`). */
export interface CliInstallStatus {
	onPath: boolean;
	binDir: string;
	detail: string;
}

/** Current PATH state of the `recast` command line tool. */
export function cliInstallStatus(): Promise<CliInstallStatus> {
	return invoke<CliInstallStatus>("cli_install_status");
}

/** Put `recast` on the user's PATH. Returns a human-readable result message. */
export function installCli(): Promise<string> {
	return invoke<string>("install_cli");
}

/** Remove `recast` from the user's PATH. Returns a human-readable result message. */
export function uninstallCli(): Promise<string> {
	return invoke<string>("uninstall_cli");
}

/**
 * Lock a window's resize to a fixed aspect ratio and cap its width at a
 * fraction of its monitor. On Windows this is a real-time WM_SIZING constraint
 * (proportional while dragging); other platforms no-op and rely on the JS
 * snap-to-aspect fallback. Re-call when the ratio changes.
 *
 * @param minWidthPx minimum width in *physical* pixels (the OS drag rect is
 *   physical too); pass `logicalMin * devicePixelRatio`.
 * @param chromePx fixed, non-scaling vertical space (physical px) reserved at
 *   the bottom of the window for a control bar outside the video. The aspect
 *   applies to `height - chromePx`. Pass 0 for a video-only window.
 */
export function setWindowAspectRatio(
	label: string,
	aspectWidth: number,
	aspectHeight: number,
	maxScreenFraction: number,
	minWidthPx: number,
	chromePx: number,
): Promise<void> {
	return invoke("set_window_aspect_ratio", {
		label,
		aspectWidth,
		aspectHeight,
		maxScreenFraction,
		minWidthPx,
		chromePx,
	});
}

export function getOutputDir(): Promise<string> {
	return invoke<string>("get_output_dir");
}

export function setOutputDir(path: string): Promise<void> {
	return invoke("set_output_dir", { path });
}

export function getDisplays(): Promise<DisplayInfo[]> {
	return invoke<DisplayInfo[]>("get_displays");
}

export function getWindows(): Promise<WindowInfo[]> {
	return invoke<WindowInfo[]>("get_windows");
}

export function openFileLocation(path: string): Promise<void> {
	return invoke("open_file_location", { path });
}

/** Move a file to the OS recycle bin / trash. Recoverable via the OS. */
export function deleteFile(path: string): Promise<void> {
	return invoke("delete_file", { path });
}

/**
 * Rename a file in place. If `newName` has no extension, the original extension
 * is preserved. Returns the new absolute path.
 */
export function renameFile(path: string, newName: string): Promise<string> {
	return invoke<string>("rename_file", { path, newName });
}

// Recording commands

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

export function startRecording(
	targetType: string,
	targetId: number,
	options?: RecordingOptions,
	region?: RegionRect | null,
): Promise<RecordingStartResult> {
	// No PII: source kind, capture rate, quality tier only.
	analytics.capture("recording_started", {
		source_kind: targetType,
		fps: options?.fps ?? "default",
		quality: options?.quality ?? "auto",
	});
	return invoke<RecordingStartResult>("start_recording", {
		targetType,
		targetId,
		region: region ?? null,
		options: options ?? null,
	});
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

export function getLastSource(): Promise<LastSource | null> {
	return invoke<LastSource | null>("get_last_source");
}

export function setLastSource(source: LastSource): Promise<void> {
	return invoke("set_last_source", { source });
}

export function getAudioDevices(): Promise<AudioDeviceInfo[]> {
	return invoke<AudioDeviceInfo[]>("get_audio_devices");
}

export function getCameraDevices(): Promise<CameraDeviceInfo[]> {
	return invoke<CameraDeviceInfo[]>("get_camera_devices");
}

export function validateCameraSource(deviceId: string): Promise<CameraValidationResult> {
	return invoke<CameraValidationResult>("validate_camera_source", { deviceId });
}

export function updateCameraPreviewState(state: CameraPreviewState): Promise<void> {
	return invoke("update_camera_preview_state", { state });
}

export function stopRecording(): Promise<string> {
	analytics.capture("recording_stopped", {});
	return invoke<string>("stop_recording");
}

export function pauseRecording(): Promise<void> {
	return invoke<void>("pause_recording");
}

export function resumeRecording(): Promise<void> {
	return invoke<void>("resume_recording");
}

export function isRecordingPaused(): Promise<boolean> {
	return invoke<boolean>("is_recording_paused");
}

export function listRecasts(): Promise<RecordingEntry[]> {
	return invoke<RecordingEntry[]>("list_recasts");
}

export function listExports(): Promise<RecordingEntry[]> {
	return invoke<RecordingEntry[]>("list_exports");
}

// Recast Cloud commands

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

/**
 * Upload an already-exported MP4 to Recast Cloud and create a public share
 * link. The caller exports the file first; `workspaceId` comes from the
 * desktop profile's `defaultWorkspaceId`. Progress (coarse phase + byte
 * counts during the PUT) streams on a request-scoped channel, with no path
 * correlation. Resolves with the share result; rejects on failure (a detached
 * `recast-cloud:error` event still fires for corner notifications).
 */
export function recastCloudUpload(
	path: string,
	title: string,
	workspaceId?: string,
	/** Output-time transcript to publish as a selectable caption track. */
	captionsTranscript?: Transcript | null,
	onEvent?: (e: CloudUploadEvent) => void,
): Promise<CloudShareResult> {
	const channel = new Channel<CloudUploadEvent>();
	if (onEvent) channel.onmessage = onEvent;
	return invoke<CloudShareResult>("recast_cloud_upload", {
		path,
		title,
		workspaceId,
		captionsTranscript: captionsTranscript ?? null,
		onEvent: channel,
	});
}

/**
 * Update an existing share. Omit a field to leave it unchanged; for
 * `password` / `expiresAt`, pass "" to clear.
 */
export function recastCloudUpdateShare(
	slug: string,
	opts: {
		visibility?: "public" | "workspace" | "private";
		password?: string;
		expiresAt?: string;
	},
): Promise<void> {
	return invoke<void>("recast_cloud_update_share", {
		slug,
		visibility: opts.visibility,
		password: opts.password,
		expiresAt: opts.expiresAt,
	});
}

/** Delete the cloud copy (blob + row + shares). Never touches the local file. */
export function recastCloudDelete(recastId: string, path?: string): Promise<void> {
	return invoke<void>("recast_cloud_delete", { recastId, path });
}

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

/** List the shares for a recast (owner-only). */
export function recastCloudListShares(recastId: string): Promise<CloudShareList> {
	return invoke<CloudShareList>("recast_cloud_list_shares", { recastId });
}

/** All locally-recorded cloud uploads, keyed by local export path. */
export function recastCloudListUploads(): Promise<Record<string, CloudUploadRecord>> {
	return invoke<Record<string, CloudUploadRecord>>("recast_cloud_list_uploads");
}

/** Drop a manifest entry (no network), e.g. the local file moved. */
export function recastCloudForgetUpload(path: string): Promise<void> {
	return invoke<void>("recast_cloud_forget_upload", { path });
}

// Editor commands

export function loadEditorDocument(path: string): Promise<EditorDocument> {
	return invoke<EditorDocument>("load_editor_document", { path });
}

/** Re-pack a legacy `.recast` to the current format in place (keeps a `.bak`). */
export function migrateProject(projectPath: string): Promise<void> {
	return invoke<void>("migrate_project", { projectPath });
}

export function generateThumbnails(path: string, count: number): Promise<string[]> {
	return invoke<string[]>("generate_thumbnails", { path, count });
}

export function getVideoMetadata(path: string): Promise<VideoMetadata> {
	return invoke<VideoMetadata>("get_video_metadata", { path });
}

export type ExportStateEvent =
	| { exportId: string; status: "started" }
	| { exportId: string; status: "preparing"; detail?: string }
	| { exportId: string; status: "progress"; progress: number }
	| { exportId: string; status: "finalizing" }
	| { exportId: string; status: "success"; path: string }
	| { exportId: string; status: "cancelled" }
	| { exportId: string; status: "error"; message: string };

const EXPORT_STATE_EVENT = "export-state";

export function createExportId(): string {
	if (typeof crypto !== "undefined" && typeof crypto.randomUUID === "function") {
		return crypto.randomUUID();
	}

	return `export-${Date.now()}-${Math.random().toString(36).slice(2, 10)}`;
}

export function listenToExportState(
	exportId: string,
	onState: (event: ExportStateEvent) => void,
): Promise<() => void> {
	return listen<ExportStateEvent>(EXPORT_STATE_EVENT, (event) => {
		if (event.payload.exportId !== exportId) return;
		onState(event.payload);
	});
}

/** Listen to `export-state` for EVERY export (no id filter). The activity store
 *  uses this to drive live progress across the whole queue. */
export function listenToAllExportState(
	onState: (event: ExportStateEvent) => void,
): Promise<() => void> {
	return listen<ExportStateEvent>(EXPORT_STATE_EVENT, (event) => onState(event.payload));
}

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

/**
 * Queue an export. The backend persists the payload, runs it on the single serial
 * worker (so two exports never fight for CPU/GPU), and drives progress via
 * `export-state` events. Resolves once the job is durably queued; the export runs
 * in the background and survives closing this editor.
 */
export function enqueueExport(req: EnqueueExportRequest): Promise<void> {
	analytics.capture("export_started", {
		format: req.format,
		quality: req.quality,
		speed: req.speed ?? "balanced",
		fps: req.fps ?? "source",
	});
	return invoke("enqueue_export", {
		request: {
			exportId: req.exportId,
			inputPath: req.inputPath,
			format: req.format,
			quality: req.quality,
			speed: req.speed ?? "balanced",
			renderState: req.renderState,
			gifSettings: req.gifSettings,
			fps: req.fps ?? null,
			burnCaptions: req.burnCaptions ?? false,
			captionSidecar: req.captionSidecar ?? null,
		},
	});
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

/** The whole export queue (queued, running, and undismissed results), oldest first. */
export function listExportJobs(): Promise<ExportJobDto[]> {
	return invoke<ExportJobDto[]>("list_export_jobs");
}

/** Cancel a running export or drop a queued one from the queue. */
export function cancelExportJob(id: string): Promise<void> {
	return invoke("cancel_export_job", { id });
}

/** Remove a finished (non-running) job from the queue list. */
export function dismissExportJob(id: string): Promise<void> {
	return invoke("dismiss_export_job", { id });
}

/** Requeue a failed/cancelled/interrupted job (its payload is still on disk). */
export function retryExportJob(id: string): Promise<void> {
	return invoke("retry_export_job", { id });
}

const EXPORT_JOBS_CHANGED_EVENT = "export-jobs-changed";

/** Fires whenever queue membership or a job's status changes; re-fetch the list. */
export function listenToExportJobsChanged(onChange: () => void): Promise<() => void> {
	return listen(EXPORT_JOBS_CHANGED_EVENT, () => onChange());
}

/**
 * Signal a running export to abort by its session id. Prefer
 * {@link cancelExportJob}, which also drops a still-queued job; this lower-level
 * call only flips the running export's cancel flag. Safe when nothing is running.
 */
export function cancelExport(exportId: string): Promise<void> {
	return invoke("cancel_export", { exportId });
}

// Zoom suggestions (auto-focus)

export type ZoomSuggestionReason = "click" | "settleAfterMotion";

export interface ZoomSuggestion {
	timestampUs: number;
	x: number;
	y: number;
	reason: ZoomSuggestionReason;
	/** Confidence in [0,1]: how strongly this moment warrants a zoom. */
	score?: number;
}

/**
 * Analyse a captured cursor track and return candidate auto-focus moments
 * (clicks + settle-after-motion). Backed by `detect_zoom_triggers` in Rust.
 */
export function suggestZoomRegions(cursorPath: string): Promise<ZoomSuggestion[]> {
	return invoke<ZoomSuggestion[]>("suggest_zoom_regions", { cursorPath });
}

// Silence detection

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

/**
 * Analyse a recording for silence: ranges a Silero voice-activity model
 * scores as non-speech. An idle cursor over the range raises confidence but is
 * no longer required. Implementation lives in `silence.rs` (Rust).
 */
export function detectSilence(
	audioPath?: string | null,
	microphonePath?: string | null,
	cursorPath?: string | null,
	options?: SilenceDetectOptions,
): Promise<SilenceSegment[]> {
	return invoke<SilenceSegment[]>("detect_silence", {
		audioPath: audioPath ?? null,
		microphonePath: microphonePath ?? null,
		cursorPath: cursorPath ?? null,
		options: options ?? null,
	});
}

/**
 * Decode a recording's audio (mic + system mixed) into a compact peak
 * envelope (`buckets` normalised values in [0,1]) for drawing a waveform
 * on the timeline. Returns an empty array when the clip has no audio.
 */
export function extractWaveform(
	audioPath?: string | null,
	microphonePath?: string | null,
	buckets?: number,
): Promise<number[]> {
	return invoke<number[]>("extract_waveform", {
		audioPath: audioPath ?? null,
		microphonePath: microphonePath ?? null,
		buckets: buckets ?? null,
	});
}

// Captions / transcription commands (offline ASR, M1 foundation)

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
}

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

/** OS / arch / RAM / GPU probe used to gate which caption models are offered. */
export function captionCapabilities(): Promise<DeviceCapabilities> {
	return invoke<DeviceCapabilities>("caption_capabilities");
}

/** Download (once) + cache a Google Font's woff2 on device; returns its path. */
export function ensureGoogleFont(family: string, weight: number): Promise<string> {
	return invoke<string>("ensure_google_font", { family, weight });
}

/** Write a transcript to a subtitle sidecar at `destPath`. */
export function exportCaptions(
	transcript: Transcript,
	format: "srt" | "vtt",
	destPath: string,
): Promise<void> {
	return invoke("export_captions", { transcript, format, destPath });
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

/** Catalog of caption models with per-model install state. */
export function listCaptionModels(): Promise<CaptionModelInfo[]> {
	return invoke<CaptionModelInfo[]>("list_caption_models");
}

/** Progress tick for a caption-model download. `total` is 0 when the server
 *  didn't report a content length; `file` is empty on the terminal tick. */
export interface CaptionDownloadProgress {
	modelId: string;
	file: string;
	downloaded: number;
	total: number;
}

/**
 * Download a model's files. Progress streams on a request-scoped channel: one
 * channel per download, torn down when the call settles, so the caller never
 * filters ticks by model id (contrast the old global `captions:download-progress`
 * event). Omit `onProgress` if you don't need progress.
 */
export function downloadCaptionModel(
	id: string,
	onProgress?: (p: CaptionDownloadProgress) => void,
): Promise<void> {
	const channel = new Channel<CaptionDownloadProgress>();
	if (onProgress) channel.onmessage = onProgress;
	return invoke("download_caption_model", { id, onProgress: channel });
}

export function deleteCaptionModel(id: string): Promise<void> {
	return invoke("delete_caption_model", { id });
}

/** Coarse phase of a transcription run: "extracting" | "transcribing" | "done". */
export interface TranscribeProgress {
	phase: string;
}

/**
 * Transcribe a recording's audio with the chosen model. Phase updates stream on
 * a request-scoped channel (`onPhase`); pass it to drive UI state, or omit it.
 */
export function transcribeProject(args: {
	audioPath?: string | null;
	microphonePath?: string | null;
	modelId: string;
	language?: string | null;
	onPhase?: (p: TranscribeProgress) => void;
}): Promise<Transcript> {
	const onPhase = new Channel<TranscribeProgress>();
	if (args.onPhase) onPhase.onmessage = args.onPhase;
	return invoke<Transcript>("transcribe_project", {
		audioPath: args.audioPath ?? null,
		microphonePath: args.microphonePath ?? null,
		modelId: args.modelId,
		language: args.language ?? null,
		onPhase,
	});
}

/** True when at least one given media file actually carries an audio stream
 *  (ffprobe). The caption tab gates its Generate UI on this, since a recording can
 *  have a path but no audio track. */
export function hasTranscribableAudio(paths: (string | null | undefined)[]): Promise<boolean> {
	return invoke<boolean>("has_transcribable_audio", {
		paths: paths.filter((p): p is string => !!p),
	});
}

// Remote transcription endpoints (OpenAI-compatible /audio/transcriptions)

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

/** List configured remote endpoints (with key-present flags, not the keys). */
export function listRemoteAsrEndpoints(): Promise<RemoteAsrEndpointInfo[]> {
	return invoke<RemoteAsrEndpointInfo[]>("list_remote_asr_endpoints");
}

/** Add or update a remote endpoint's config. Returns the stored, normalized form. */
export function setRemoteAsrEndpoint(endpoint: RemoteAsrEndpoint): Promise<RemoteAsrEndpoint> {
	return invoke<RemoteAsrEndpoint>("set_remote_asr_endpoint", { endpoint });
}

/** Remove a remote endpoint and its stored key. */
export function deleteRemoteAsrEndpoint(id: string): Promise<void> {
	return invoke("delete_remote_asr_endpoint", { id });
}

/** Store (or, with an empty value, clear) a remote endpoint's API key in the OS
 *  keyring. Write-only: there is no getter. */
export function setRemoteAsrKey(id: string, key: string): Promise<void> {
	return invoke("set_remote_asr_key", { id, key });
}

// Autosave / Recovery commands

export function autosaveProject(projectPath: string, editsJson: string): Promise<void> {
	return invoke("autosave_project", { projectPath, editsJson });
}

/**
 * Persist the current edits back into the `.recast` archive. Returns the
 * save timestamp (unix ms) so the UI can show "Saved at HH:MM".
 */
export function saveProjectEdits(projectPath: string, editsJson: string): Promise<number> {
	return invoke<number>("save_project_edits", { projectPath, editsJson });
}

export function clearAutosave(projectPath: string): Promise<void> {
	return invoke("clear_autosave", { projectPath });
}

export function getRecoverableSessions(): Promise<AutosaveState[]> {
	return invoke<AutosaveState[]>("get_recoverable_sessions");
}

// External asset cache

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

export function ensureAssetsInstalled(manifestUrl: string): Promise<AssetInstallResult> {
	return invoke<AssetInstallResult>("ensure_assets_installed", { manifestUrl });
}

export function getCachedAssetPath(id: string): Promise<string | null> {
	return invoke<string | null>("get_cached_asset_path", { id });
}

/** Read the on-disk manifest lock and return which assets are already cached.
 *  No network traffic, so safe to call on offline launches before `ensure`. */
export function hydrateCachedAssets(): Promise<HydratedAsset[]> {
	return invoke<HydratedAsset[]>("hydrate_cached_assets");
}

// Declarative asset-pack extensions

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

/** Install (or update) a pack from its manifest URL. Validates + sha256-verifies. */
export function installExtension(manifestUrl: string): Promise<InstalledExtension> {
	return invoke<InstalledExtension>("install_extension", { manifestUrl });
}

/** No-network enumeration of installed packs (for startup hydration). */
export function listInstalledExtensions(): Promise<InstalledExtension[]> {
	return invoke<InstalledExtension[]>("list_installed_extensions");
}

/** Toggle a pack's enabled flag without removing its files. */
export function setExtensionEnabled(extId: string, enabled: boolean): Promise<void> {
	return invoke<void>("set_extension_enabled", { extId, enabled });
}

/** Remove a pack and all of its files. */
export function uninstallExtension(extId: string): Promise<void> {
	return invoke<void>("uninstall_extension", { extId });
}

/** Fetch a curated registry *index* (no install) for the gallery. */
export function fetchExtensionRegistry<T = unknown>(indexUrl: string): Promise<T> {
	return invoke<T>("fetch_extension_registry", { indexUrl });
}


/** What the panel should preselect on open. Sent from the home mode tiles. */
export type CaptureIntent = "screen" | "window" | "region" | "camera";

 export async function launchRecordingPanel(intent?: CaptureIntent) {
    const existing = await WebviewWindow.getByLabel("recording-panel");
    if (existing) {
      await existing.setFocus();
      // The window is already mounted, so a query param wouldn't re-trigger;
      // hand the intent over on an event the panel listens for.
      if (intent) {
        const { emit } = await import("@tauri-apps/api/event");
        await emit("panel-capture-intent", { intent });
      }
      return;
    }

    // Window is sized larger than the visible panel so the CSS drop shadow
    // has room to paint without being clipped by the window bounds.
    const panelWidth = 520;
    const panelHeight = 72;
    const panelWin = new WebviewWindow("recording-panel", {
      url: intent ? `/panel?intent=${intent}` : "/panel",
      title: "Recast Panel",
      width: panelWidth,
      height: panelHeight,
      decorations: false,
      transparent: true,
	  shadow: false,
      alwaysOnTop: !isLinux(),
      resizable: false,
      skipTaskbar: true,
      x: Math.round(window.screen.availWidth / 2 - panelWidth / 2),
      y: window.screen.availHeight - panelHeight - 40,
    });

    // Keep Recast's own controls out of the recorded video. Gated on the
    // user setting (default on); exclusion must run on `tauri://created`, once
    // the native window handle exists. No-op on Linux (no OS support).
    panelWin.once("tauri://created", async () => {
      try {
        if (await getHidePanelFromCapture()) {
          await excludeWindowFromCapture("recording-panel");
        }
      } catch (err) {
        // Non-fatal: the panel just won't be hidden from the capture.
        console.warn("recording panel capture-exclusion failed:", err);
      }
    });

    panelWin.once("tauri://error", (e) => console.error(e));
  }

// Floating webcam preview window.
//
// MUST be excluded from screen capture or DXGI Desktop Duplication bakes the
// camera bubble into the recorded screen video. `exclude_window_from_capture`
// (Windows: SetWindowDisplayAffinity WDA_EXCLUDEFROMCAPTURE) runs on
// `tauri://created`; any earlier and the HWND isn't reachable yet.
export async function openCameraPreviewWindow() {
  const existing = await WebviewWindow.getByLabel("camera-preview");
  if (existing) {
    // Re-apply the exclusion in case the window was reused after a crash
    // or stop/restart cycle that dropped the affinity.
    excludeWindowFromCapture("camera-preview").catch(
      (err) => console.warn("camera preview exclusion (existing) failed:", err),
    );
    await existing.setFocus();
    return;
  }

  const previewSize = 320;
  // The window is the square video bubble plus a control strip below it. Keep
  // this strip height in sync with `CONTROL_BAR_HEIGHT` in
  // `routes/camera-preview/+page.svelte` so the window opens at the right size
  // and doesn't visibly resize itself once the aspect lock kicks in on mount.
  const CONTROL_BAR_HEIGHT = 40;
  const previewWin = new WebviewWindow("camera-preview", {
    url: "/camera-preview",
    title: "Camera",
    width: previewSize,
    height: previewSize + CONTROL_BAR_HEIGHT,
    decorations: false,
    transparent: true,
    shadow: false,
    alwaysOnTop: !isLinux(),
    resizable: true,
    skipTaskbar: true,
    x: Math.round(window.screen.availWidth - previewSize - 40),
    y: Math.round(window.screen.availHeight - previewSize - CONTROL_BAR_HEIGHT - 40),
  });

  previewWin.once("tauri://error", (e) => console.error(e));
  previewWin.once("tauri://created", async () => {
    try {
      await excludeWindowFromCapture("camera-preview");
    } catch (err) {
      // Non-fatal, but the preview's pixels will leak into screen captures.
      console.warn(
        "Failed to exclude camera-preview from screen capture:",
        err,
      );
    }
  });
}

// System tray, diagnostics & misc commands.
// These wrappers are thin; web-safe callers guard with `isTauriApp()` themselves.

/** Exclude a window (by Tauri label) from screen capture (Windows
 *  `SetWindowDisplayAffinity`, macOS `NSWindow.sharingType`). No-op on Linux,
 *  which has no per-window exclusion API. */
export function excludeWindowFromCapture(label: string): Promise<void> {
	return invoke<void>("exclude_window_from_capture", { label });
}

/** Whether the floating recording panel is hidden from screen recordings.
 *  Backed by `AppConfig.hide_panel_from_capture` (default on). */
export function getHidePanelFromCapture(): Promise<boolean> {
	return invoke<boolean>("get_hide_panel_from_capture");
}

export function setHidePanelFromCapture(enabled: boolean): Promise<void> {
	return invoke<void>("set_hide_panel_from_capture", { enabled });
}

/** Refresh the system tray menu/icon. `isRecording` overrides the recording
 *  state shown; `null`/omitted lets the backend resolve it. */
export function refreshTray(isRecording?: boolean | null): Promise<void> {
	return invoke<void>("refresh_tray", { isRecording: isRecording ?? null });
}

/** Whether closing the main window hides to tray instead of quitting. */
export function getCloseToTray(): Promise<boolean> {
	return invoke<boolean>("get_close_to_tray");
}

export function setCloseToTray(enabled: boolean): Promise<void> {
	return invoke<void>("set_close_to_tray", { enabled });
}

/** Whether the window uses a translucent OS backdrop (Mica/Acrylic/vibrancy). */
export function getWindowTransparency(): Promise<boolean> {
	return invoke<boolean>("get_window_transparency");
}

export function setWindowTransparency(enabled: boolean): Promise<void> {
	return invoke<void>("set_window_transparency", { enabled });
}

/** Open the app's log directory in the OS file manager; returns the path. */
export function openLogDir(): Promise<string> {
	return invoke<string>("open_log_dir");
}

/** Consume a file path the OS asked us to open (file association / deep link),
 *  if one is pending. Returns `null` when there's nothing queued. */
export function takePendingOpenFile(): Promise<string | null> {
	return invoke<string | null>("take_pending_open_file");
}

/** Whether the app was cold-launched via the jump list "New Recording" task. */
export function takePendingNewRecording(): Promise<boolean> {
	return invoke<boolean>("take_pending_new_recording");
}

/** Whether a capture session is currently active (recording or paused). */
export function isRecordingActive(): Promise<boolean> {
	return invoke<boolean>("is_recording_active");
}

/** Persist the user's telemetry consent. `installId` seeds a fresh anonymous
 *  id when product analytics is first enabled. */
export function setTelemetryConsent(
	product: boolean,
	errors: boolean,
	installId?: string,
): Promise<void> {
	return invoke<void>("set_telemetry_consent", { product, errors, installId });
}

/** Whether verbose diagnostic logging is enabled. */
export function getDiagnosticLogging(): Promise<boolean> {
	return invoke<boolean>("get_diagnostic_logging");
}

export function setDiagnosticLogging(enabled: boolean): Promise<void> {
	return invoke<void>("set_diagnostic_logging", { enabled });
}

// Recast Cloud: account / auth.
// All are `#[serde(rename_all = "camelCase")]` on the Rust side EXCEPT
// `AuthStartResult` (noted inline).

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

export function authStatus(): Promise<AuthStatus> {
	return invoke<AuthStatus>("auth_status");
}

export function authStart(): Promise<AuthStartResult> {
	return invoke<AuthStartResult>("auth_start");
}

export function authSignOut(): Promise<void> {
	return invoke<void>("auth_sign_out");
}

export function authCancel(): Promise<void> {
	return invoke<void>("auth_cancel");
}

export function getCloudApiConfig(): Promise<CloudApiConfig> {
	return invoke<CloudApiConfig>("get_cloud_api_config");
}

/** Set (or clear, with `null`) the self-hosting endpoint override. Returns the
 *  resolved config; the backend validates and falls back to the default. */
export function setCloudApiUrl(url: string | null): Promise<CloudApiConfig> {
	return invoke<CloudApiConfig>("set_cloud_api_url", { url });
}

// Google Drive: `gdrive_*` commands (OAuth + Drive upload). Thin wrappers; the
// gdrive store guards every call with `isTauriApp()`.

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

export function gdriveStatus(): Promise<GdriveStatus> {
	return invoke<GdriveStatus>("gdrive_status");
}

export function gdriveListUploads(): Promise<Record<string, GdriveUploadRecord>> {
	return invoke<Record<string, GdriveUploadRecord>>("gdrive_list_uploads");
}

export function gdriveConnect(): Promise<void> {
	return invoke<void>("gdrive_connect");
}

export function gdriveDisconnect(): Promise<void> {
	return invoke<void>("gdrive_disconnect");
}

/** Byte progress for an in-flight Drive upload, streamed on the request-scoped
 *  channel (one per upload → no id to correlate). */
export interface GdriveUploadProgress {
	bytesSent: number;
	totalBytes: number;
}

/**
 * Upload an exported file to Drive. Byte progress streams on a request-scoped
 * channel (`onProgress`); success is the resolved {@link GdriveUploadResult},
 * failure the rejection (plus a detached `gdrive:upload-error` event carrying
 * the cancelled/failed distinction for the corner card).
 */
export function gdriveUpload(
	path: string,
	uploadId: string,
	onProgress?: (p: GdriveUploadProgress) => void,
): Promise<GdriveUploadResult> {
	const channel = new Channel<GdriveUploadProgress>();
	if (onProgress) channel.onmessage = onProgress;
	return invoke<GdriveUploadResult>("gdrive_upload", { path, uploadId, onProgress: channel });
}

export function gdriveCancelUpload(uploadId: string): Promise<void> {
	return invoke<void>("gdrive_cancel_upload", { uploadId });
}

export function gdriveForgetUpload(localPath: string): Promise<void> {
	return invoke<void>("gdrive_forget_upload", { localPath });
}

/** Validate a `.recast` project file, throwing if it isn't a readable, valid
 *  project. Used purely as a guard before opening; the backend returns the
 *  project metadata, but no caller surfaces it, so it's intentionally not typed
 *  out here (kept as `void`). */
export function peekRecastProject(path: string): Promise<void> {
	return invoke<void>("peek_recast_project", { path });
}
