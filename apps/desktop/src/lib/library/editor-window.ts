/**
 * Opening a recording/export in the editor (current window or a dedicated
 * webview). Shared by the home and recasts pages so the window label/sizing
 * and path encoding stay in one place.
 */

import { goto } from "$app/navigation";
import type { RecordingEntry } from "@recast/editor/lib/wire-types";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";

/** Encode a filesystem path for use as an `/editor/[file]` route segment. */
export function encodeEditorPath(path: string): string {
	return encodeURIComponent(btoa(encodeURIComponent(path)));
}

/**
 * Open the editor for `entry` in a separate webview window, focusing an
 * existing one for the same file instead of spawning a duplicate.
 */
export async function openInNewWindow(entry: RecordingEntry): Promise<void> {
	const route = `/editor/${encodeEditorPath(entry.path)}`;
	const label = `editor-${encodeEditorPath(entry.path)
		.replace(/[^a-zA-Z0-9]/g, "")
		.slice(0, 48)}`;
	const existing = await WebviewWindow.getByLabel(label);
	if (existing) {
		await existing.setFocus();
		return;
	}
	new WebviewWindow(label, {
		url: route,
		title: `Editor - ${entry.filename}`,
		width: 1440,
		height: 960,
		center: true,
		decorations: false,
	});
}

/**
 * Open the editor for `entry`, honouring the user's window preference: navigate
 * the current window, or spawn/focus a dedicated editor window.
 */
export async function openInEditor(
	entry: RecordingEntry,
	mode: "navigate" | "new-window",
): Promise<void> {
	if (mode === "new-window") {
		await openInNewWindow(entry);
	} else {
		goto(`/editor/${encodeEditorPath(entry.path)}`);
	}
}
