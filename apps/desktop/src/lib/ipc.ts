/**
 * Typed IPC wrappers for Tauri backend commands.
 *
 * All invoke() calls should go through these functions instead of using
 * raw invoke() strings. This gives us:
 * - Type safety for arguments and return values
 * - Single place to update if command signatures change
 * - Better IDE autocomplete
 */

import type { EditorRenderState, VideoMetadata } from "$lib/stores/editor-store.svelte";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";

//  Types matching Rust structs 

export interface DisplayInfo {
	id: number;
	name: string;
	x: number;
	y: number;
	width: number;
	height: number;
	isPrimary: boolean;
	thumbnail: string | null;
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
}

export interface AutosaveState {
	projectPath: string;
	savedAtUnixMs: number;
	editsJson: string;
}

//  System commands

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

//  Recording commands

export interface RecordingOptions {
	systemAudio?: boolean;
	microphone?: boolean;
	microphoneDeviceId?: string | null;
	camera?: boolean;
	cameraDeviceId?: string | null;
}

export interface AudioDeviceInfo {
	id: string;
	name: string;
	isDefault: boolean;
}

export interface CameraDeviceInfo {
	id: string;
	name: string;
}

export function startRecording(
	targetType: string,
	targetId: number,
	options?: RecordingOptions,
): Promise<void> {
	return invoke("start_recording", { targetType, targetId, options: options ?? null });
}

export function getAudioDevices(): Promise<AudioDeviceInfo[]> {
	return invoke<AudioDeviceInfo[]>("get_audio_devices");
}

export function getCameraDevices(): Promise<CameraDeviceInfo[]> {
	return invoke<CameraDeviceInfo[]>("get_camera_devices");
}

export function stopRecording(): Promise<string> {
	return invoke<string>("stop_recording");
}

export function listRecasts(): Promise<RecordingEntry[]> {
	return invoke<RecordingEntry[]>("list_recasts");
}

export function listExports(): Promise<RecordingEntry[]> {
	return invoke<RecordingEntry[]>("list_exports");
}

//  Editor commands 

export function loadEditorDocument(path: string): Promise<EditorDocument> {
	return invoke<EditorDocument>("load_editor_document", { path });
}

export function generateThumbnails(path: string, count: number): Promise<string[]> {
	return invoke<string[]>("generate_thumbnails", { path, count });
}

export function getVideoMetadata(path: string): Promise<VideoMetadata> {
	return invoke<VideoMetadata>("get_video_metadata", { path });
}

export type ExportStateEvent =
	| { exportId: string; status: "started" }
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

export function exportVideo(
	inputPath: string,
	format: string,
	quality: string,
	renderState: EditorRenderState,
	exportId: string,
): Promise<string> {
	return invoke<string>("export_video", {
		request: { exportId, inputPath, format, quality, renderState },
	});
}

/**
 * Signal any running export to abort. Causes `exportVideo` to reject with
 * `"export cancelled"`. Safe to call when no export is running.
 */
export function cancelExport(exportId: string): Promise<void> {
	return invoke("cancel_export", { exportId });
}

//  Zoom suggestions (auto-focus) 

export type ZoomSuggestionReason = "click" | "settleAfterMotion";

export interface ZoomSuggestion {
	timestampUs: number;
	x: number;
	y: number;
	reason: ZoomSuggestionReason;
}

/**
 * Analyse a captured cursor track and return candidate auto-focus moments
 * (clicks + settle-after-motion). Backed by `detect_zoom_triggers` in Rust.
 */
export function suggestZoomRegions(cursorPath: string): Promise<ZoomSuggestion[]> {
	return invoke<ZoomSuggestion[]>("suggest_zoom_regions", { cursorPath });
}

//  Autosave / Recovery commands 

export function autosaveProject(projectPath: string, editsJson: string): Promise<void> {
	return invoke("autosave_project", { projectPath, editsJson });
}

export function clearAutosave(projectPath: string): Promise<void> {
	return invoke("clear_autosave", { projectPath });
}

export function getRecoverableSessions(): Promise<AutosaveState[]> {
	return invoke<AutosaveState[]>("get_recoverable_sessions");
}

//  External asset cache 

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
 *  No network traffic — safe to call on offline launches before `ensure`. */
export function hydrateCachedAssets(): Promise<HydratedAsset[]> {
	return invoke<HydratedAsset[]>("hydrate_cached_assets");
}


// start recording
 export async function launchRecordingPanel() {
    const existing = await WebviewWindow.getByLabel("recording-panel");
    if (existing) {
      await existing.setFocus();
      return;
    }

    const panelWidth = 460;
    const panelHeight = 44;
    const panelWin = new WebviewWindow("recording-panel", {
      url: "/panel",
      title: "Recast Panel",
      width: panelWidth,
      height: panelHeight,
      decorations: false,
      transparent: true,
      alwaysOnTop: true,
      resizable: false,
      skipTaskbar: true,
      x: Math.round(window.screen.availWidth / 2 - panelWidth / 2),
      y: window.screen.availHeight - panelHeight - 40,
    });

    panelWin.once("tauri://error", (e) => console.error(e));
  }
