/**
 * Typed IPC wrappers for Tauri backend commands.
 *
 * All invoke() calls should go through these functions instead of using
 * raw invoke() strings. This gives us:
 * - Type safety for arguments and return values
 * - Single place to update if command signatures change
 * - Better IDE autocomplete
 */

import { invoke } from "@tauri-apps/api/core";
import type { EditorRenderState, VideoMetadata } from "$lib/stores/editor-store.svelte";

// ── Types matching Rust structs ─────────────────────────────────────────

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
	metadata: VideoMetadata;
	renderState: EditorRenderState;
}

export interface AutosaveState {
	projectPath: string;
	savedAtUnixMs: number;
	editsJson: string;
}

// ── System commands ─────────────────────────────────────────────────────

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

// ── Recording commands ──────────────────────────────────────────────────

export function startRecording(targetType: string, targetId: number): Promise<void> {
	return invoke("start_recording", { targetType, targetId });
}

export function stopRecording(): Promise<string> {
	return invoke<string>("stop_recording");
}

export function listRecordings(): Promise<RecordingEntry[]> {
	return invoke<RecordingEntry[]>("list_recordings");
}

// ── Editor commands ─────────────────────────────────────────────────────

export function loadEditorDocument(path: string): Promise<EditorDocument> {
	return invoke<EditorDocument>("load_editor_document", { path });
}

export function renderPreviewFrame(
	inputPath: string,
	time: number,
	renderState: EditorRenderState,
): Promise<string> {
	return invoke<string>("render_preview_frame", {
		request: { inputPath, time, renderState },
	});
}

export function generateThumbnails(path: string, count: number): Promise<string[]> {
	return invoke<string[]>("generate_thumbnails", { path, count });
}

export function getVideoMetadata(path: string): Promise<VideoMetadata> {
	return invoke<VideoMetadata>("get_video_metadata", { path });
}

export function exportVideo(
	inputPath: string,
	format: string,
	quality: string,
	renderState: EditorRenderState,
): Promise<string> {
	return invoke<string>("export_video", {
		request: { inputPath, format, quality, renderState },
	});
}

// ── Autosave / Recovery commands ────────────────────────────────────────

export function autosaveProject(projectPath: string, editsJson: string): Promise<void> {
	return invoke("autosave_project", { projectPath, editsJson });
}

export function clearAutosave(projectPath: string): Promise<void> {
	return invoke("clear_autosave", { projectPath });
}

export function getRecoverableSessions(): Promise<AutosaveState[]> {
	return invoke<AutosaveState[]>("get_recoverable_sessions");
}
