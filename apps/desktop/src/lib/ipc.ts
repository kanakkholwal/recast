/**
 * Typed `invoke()` wrappers for the Tauri backend commands. The request and
 * response shapes live in `ipc-types.ts`; import there if you only need types.
 */

// Type-only: erased at runtime, so there is no ESM cycle with `$lib/profiles`.
import type { RecordingProfile } from "@recast/editor/lib/profiles";
import type {
	AssetInstallResult,
	AudioDeviceInfo,
	CaptionDownloadProgress,
	CaptionModelInfo,
	DeviceCapabilities,
	HydratedAsset,
	InstalledExtension,
	OcrProgress,
	SilenceDetectOptions,
	SilenceSegment,
	TranscribeProgress,
	Transcript,
	VideoTextTimeline,
	ZoomSuggestion,
} from "@recast/editor/lib/wire-types";
import type { VideoMetadata } from "@recast/editor/stores/editor-store.svelte";
import { Channel, invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { platform } from "@tauri-apps/plugin-os";
import { analytics } from "$lib/analytics/client";
import type {
	AuthStartResult,
	AuthStatus,
	AutosaveState,
	CameraDeviceInfo,
	CameraPreviewState,
	CameraValidationResult,
	CaptureCapabilities,
	CaptureIntent,
	CaptureIntentState,
	CliInstallStatus,
	CloudApiConfig,
	CloudShareList,
	CloudShareResult,
	CloudUploadEvent,
	CloudUploadRecord,
	DisplayInfo,
	EditorDocument,
	EncoderAvailability,
	EnqueueExportRequest,
	ExportJobDto,
	ExportStateEvent,
	FfmpegDiagnostics,
	GdriveStatus,
	GdriveUploadProgress,
	GdriveUploadRecord,
	GdriveUploadResult,
	LastSource,
	ProfilesSnapshot,
	RecordingEntry,
	RecordingOptions,
	RecordingStartResult,
	RegionRect,
	RemoteAsrEndpoint,
	RemoteAsrEndpointInfo,
	WindowInfo,
} from "$lib/recorder-types";

// KWin on Wayland lets an undecorated always-on-top window trap focus. Lazy, since `platform()` at module eval breaks web and SSR imports.
const isLinux = () => platform() === "linux";

export type {
	AssetInstallFailure,
	AssetInstallResult,
	AudioDeviceInfo,
	CaptionDownloadProgress,
	CaptionEngine,
	CaptionModelInfo,
	CaptionModelSource,
	CaptionRuntime,
	DeviceCapabilities,
	ExportGifSettings,
	ExportSpeed,
	ExtAssetPath,
	ExtBackgroundContribution,
	ExtCaptionPresetContribution,
	ExtColorContribution,
	ExtCursorContribution,
	ExtEasingContribution,
	ExtensionAssetEntry,
	ExtensionContributions,
	ExtensionManifest,
	ExtGradientContribution,
	ExtSmoothingContribution,
	GpuInfo,
	HydratedAsset,
	InstalledExtension,
	OcrPhase,
	OcrProgress,
	OcrStats,
	ScreenElement,
	ScreenStateSpan,
	SilenceDetectOptions,
	SilenceSegment,
	TranscribeProgress,
	Transcript,
	TranscriptSegment,
	TranscriptWord,
	VideoTextTimeline,
	ZoomSuggestion,
	ZoomSuggestionReason,
} from "@recast/editor/lib/wire-types";
export type {
	AuthPlan,
	AuthStartResult,
	AuthStatus,
	AuthUsage,
	AutosaveState,
	CameraDeviceInfo,
	CameraPreviewState,
	CameraValidationResult,
	CapabilityStatus,
	CaptionModelContribution,
	CaptionModelPackFile,
	CaptureCapabilities,
	CaptureCapability,
	CaptureIntent,
	CaptureIntentState,
	CliInstallStatus,
	CloudApiConfig,
	CloudPhase,
	CloudShareLink,
	CloudShareList,
	CloudShareResult,
	CloudUploadEvent,
	CloudUploadRecord,
	CloudWorkspace,
	DisplayInfo,
	EditorDocument,
	EncoderAvailability,
	EnqueueExportRequest,
	ExportJobDto,
	ExportStateEvent,
	FfmpegDiagnostics,
	GdriveStatus,
	GdriveUploadProgress,
	GdriveUploadRecord,
	GdriveUploadResult,
	LastSource,
	ProfilesSnapshot,
	RecordingEntry,
	RecordingOptions,
	RecordingStartResult,
	RegionRect,
	RemoteAsrEndpoint,
	RemoteAsrEndpointInfo,
	WindowInfo,
} from "$lib/recorder-types";

// System commands

/** Probe which video encoders actually work on this device (real init
 *  probe, not just "compiled in"). Each hardware probe spawns ffmpeg, so
 *  this can take up to ~2s cold, so call it off the render path. */
export function probeVideoEncoders(): Promise<EncoderAvailability[]> {
	return invoke<EncoderAvailability[]>("probe_video_encoders");
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

/** Event name broadcast by the backend whenever the capture intent changes. */
export const CAPTURE_INTENT_CHANGED_EVENT = "capture-intent:changed";

/** Read the current staged capture intent. */
export function getCaptureIntent(): Promise<CaptureIntentState> {
	return invoke<CaptureIntentState>("get_capture_intent");
}

/** Replace the staged capture intent (broadcasts `capture-intent:changed`). */
export function setCaptureIntent(intent: CaptureIntentState): Promise<CaptureIntentState> {
	return invoke<CaptureIntentState>("set_capture_intent", { intent });
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

/** Whether `setup()` auto-installs the CLI on first launch. Backed by
 *  `AppConfig.cli_auto_install` (default true). */
export function getCliAutoInstall(): Promise<boolean> {
	return invoke<boolean>("get_cli_auto_install");
}
export function setCliAutoInstall(enabled: boolean): Promise<void> {
	return invoke<void>("set_cli_auto_install", { enabled });
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

export function getLastSource(): Promise<LastSource | null> {
	return invoke<LastSource | null>("get_last_source");
}

export function setLastSource(source: LastSource): Promise<void> {
	return invoke("set_last_source", { source });
}

/** A screenshot written to disk, as the capture reports it. */
export interface Screenshot {
	path: string;
	width: number;
	height: number;
	/** The captured surface: `display`, `window`, `region`, or `app`. */
	kind: string;
	base64?: string;
	/** Absent when no copy was asked for; `false` when the OS refused one. */
	copiedToClipboard?: boolean;
}

/**
 * Capture a region of the screen at native resolution and save it under the
 * output directory. `rect` is in physical virtual-desktop pixels, which is what
 * the region overlay emits.
 */
export function captureRegionShot(rect: RegionRect): Promise<Screenshot> {
	return invoke<Screenshot>("capture_region_shot", { rect });
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

/** Camera geometry plus the token identifying this capture session. */
export type CameraGeometry = { width: number; height: number; session: number };

/** Open the camera and stream preview frames.
 *  Cameras are exclusive, so this also takes the device away from getUserMedia.
 *  Each frame is `width: u32le, height: u32le` then BGRA rows. */
export function startCameraPreview(
	device: string,
	onFrame: Channel<ArrayBuffer>,
): Promise<CameraGeometry> {
	return invoke<CameraGeometry>("start_camera_preview", { device, onFrame });
}

/** Release the camera held by `session`. A stale token is ignored, so a closing
 *  preview window cannot stop the one that replaced it. */
export function stopCameraPreview(session: number): Promise<void> {
	return invoke<void>("stop_camera_preview", { session });
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

/** WebVTT for the caption sidecar next to `mediaPath` (`foo.mp4` → `foo.vtt`/
 *  `foo.srt`), or null when neither exists. Lets the player show a file's
 *  captions with no loaded project. */
export function captionSidecarVtt(mediaPath: string): Promise<string | null> {
	return invoke<string | null>("caption_sidecar_vtt", { mediaPath });
}

// Recast Cloud commands

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

/**
 * Queue an export. The backend persists the payload, runs it on the single serial
 * worker (so two exports never fight for CPU/GPU), and drives progress via
 * `export-state` events. Resolves once the job is durably queued; the export runs
 * in the background and survives closing this editor.
 */
export function enqueueExport(req: EnqueueExportRequest): Promise<string[]> {
	analytics.capture("export_started", {
		format: req.format,
		quality: req.quality,
		speed: req.speed ?? "balanced",
		fps: req.fps ?? "source",
	});
	return invoke<string[]>("enqueue_export", {
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
			browserVideoPath: req.browserVideoPath ?? null,
			timeMap: req.timeMap ?? null,
		},
	});
}

/** Persist a browser-rendered export video (mp4 bytes ride the invoke body as a
 *  raw ArrayBuffer) to a temp file; returns the path to pass as `browserVideoPath`
 *  on the follow-up {@link enqueueExport} so the job mux-copies it. */
export function saveBrowserExportVideo(bytes: ArrayBuffer): Promise<string> {
	return invoke<string>("save_browser_export_video", bytes);
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

/**
 * Analyse a captured cursor track and return candidate auto-focus moments
 * (clicks + settle-after-motion). Backed by `detect_zoom_triggers` in Rust.
 */
export function suggestZoomRegions(cursorPath: string): Promise<ZoomSuggestion[]> {
	return invoke<ZoomSuggestion[]>("suggest_zoom_regions", { cursorPath });
}

// Silence detection

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

/** OS / arch / RAM / GPU probe used to gate which caption models are offered. */
export function captionCapabilities(): Promise<DeviceCapabilities> {
	return invoke<DeviceCapabilities>("caption_capabilities");
}

/** Download (once) + cache a Google Font's woff2 on device; returns its path. */
export function ensureGoogleFont(family: string, weight: number): Promise<string> {
	return invoke<string>("ensure_google_font", { family, weight });
}

/** The same family's TTF. The engine's shaper cannot read the woff2 above. */
export function captionFontFile(family: string, weight: number): Promise<string> {
	return invoke<string>("caption_font_file", { family, weight });
}

/** Write a transcript to a subtitle sidecar at `destPath`. */
export function exportCaptions(
	transcript: Transcript,
	format: "srt" | "vtt",
	destPath: string,
): Promise<void> {
	return invoke("export_captions", { transcript, format, destPath });
}

/** Catalog of caption models with per-model install state. */
export function listCaptionModels(): Promise<CaptionModelInfo[]> {
	return invoke<CaptionModelInfo[]>("list_caption_models");
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

/** Stop the in-flight transcription. `transcribe_project` then rejects with
 *  `TRANSCRIBE_CANCELLED` (see `@recast/editor/services`). */
export function cancelTranscription(): Promise<void> {
	return invoke("cancel_transcription");
}

// --- On-device OCR (experimental): reads a recording into a timestamped text timeline; only the dev-only OCR tab surfaces it.

/**
 * Read a recording into a screen-state timeline. `previews` attaches a small JPEG
 * per span for review UIs; leave it off for machine consumers. The models are
 * fetched on first use, which is the only run that reports a "downloading" phase.
 *
 * `includeRanges` are `[start, end]` pairs in ORIGINAL-recording seconds naming
 * the footage the edit actually keeps (the segments left after trim and cuts).
 * Pass `store.segments` so removed footage is never read; omit it to read the
 * whole file. Timestamps in the result are original-recording seconds, the same
 * clock `store.seek()` takes.
 */
export function readVideoText(args: {
	videoPath: string;
	previews?: boolean;
	includeRanges?: [number, number][];
	onPhase?: (p: OcrProgress) => void;
}): Promise<VideoTextTimeline> {
	const onPhase = new Channel<OcrProgress>();
	if (args.onPhase) onPhase.onmessage = args.onPhase;
	return invoke<VideoTextTimeline>("read_video_text", {
		videoPath: args.videoPath,
		previews: args.previews ?? false,
		includeRanges: args.includeRanges ?? [],
		onPhase,
	});
}

/** Write an already-serialized read (JSON, or the review panel's readable Markdown)
 *  to `destPath`. The timeline lives here as an object, so it serializes on this
 *  side; the backend only owns the disk write. */
export function exportScreenText(body: string, destPath: string): Promise<void> {
	return invoke<void>("export_screen_text", { body, destPath });
}

/** True when at least one given media file actually carries an audio stream
 *  (ffprobe). The caption tab gates its Generate UI on this, since a recording can
 *  have a path but no audio track. */
export function hasTranscribableAudio(paths: (string | null | undefined)[]): Promise<boolean> {
	return invoke<boolean>("has_transcribable_audio", {
		paths: paths.filter((p): p is string => Boolean(p)),
	});
}

// Remote transcription endpoints (OpenAI-compatible /audio/transcriptions)

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

export async function launchRecordingPanel(intent?: CaptureIntent) {
	const existing = await WebviewWindow.getByLabel("recording-panel");
	if (existing) {
		await existing.setFocus();
		// The window is already mounted, so a query param wouldn't re-trigger; hand the intent over on an event.
		if (intent) {
			const { emit } = await import("@tauri-apps/api/event");
			await emit("panel-capture-intent", { intent });
		}
		return;
	}

	// Sized larger than the visible panel so the CSS drop shadow has room and isn't clipped by the window bounds.
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

	// Gated on the user setting; exclusion must run on `tauri://created`, once the native handle exists. No-op on Linux.
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

// MUST be excluded or DXGI Desktop Duplication bakes the bubble into the recording; runs on `tauri://created`, when the HWND exists.
export async function openCameraPreviewWindow() {
	const existing = await WebviewWindow.getByLabel("camera-preview");
	if (existing) {
		// Re-apply after a crash or stop/restart cycle, which can leave a reused window without the affinity.
		excludeWindowFromCapture("camera-preview").catch((err) =>
			console.warn("camera preview exclusion (existing) failed:", err),
		);
		await existing.setFocus();
		return;
	}

	const previewSize = 320;
	// Keep in sync with `CONTROL_BAR_HEIGHT` in routes/camera-preview, so the window doesn't visibly resize on mount.
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
			console.warn("Failed to exclude camera-preview from screen capture:", err);
		}
	});
}

// --- System tray, diagnostics and misc: thin wrappers; web-safe callers guard with `isTauriApp()` themselves.

/** Exclude a window (by Tauri label) from screen capture (Windows
 *  `SetWindowDisplayAffinity`, macOS `NSWindow.sharingType`). No-op on Linux,
 *  which has no per-window exclusion API. */
export function excludeWindowFromCapture(label: string): Promise<void> {
	return invoke<void>("exclude_window_from_capture", { label });
}

/** Whether recordings are written by the FFmpeg-free GPU writer.
 *  Backed by `AppConfig.native_encoder` (default off). */
export function getNativeEncoder(): Promise<boolean> {
	return invoke<boolean>("get_native_encoder");
}

export function setNativeEncoder(enabled: boolean): Promise<void> {
	return invoke<void>("set_native_encoder", { enabled });
}

/** Whether this machine can honour the native writer (Windows + an MF H.264
 *  encoder). False elsewhere, so the toggle is shown disabled rather than
 *  silently doing nothing. */
export function nativeEncoderAvailable(): Promise<boolean> {
	return invoke<boolean>("native_encoder_available");
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

// --- Recast Cloud auth: all camelCase on the Rust side except `AuthStartResult`, noted inline.

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

// --- Google Drive `gdrive_*` commands: thin wrappers; the gdrive store guards every call with `isTauriApp()`.

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
