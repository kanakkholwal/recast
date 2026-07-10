/**
 * External-open pipeline for `.recast` files (today: OS file association).
 *
 * External opens always land in a fresh editor window, never navigating the
 * main window, so the library view stays put and unsaved edits elsewhere aren't
 * disturbed. Same path opened twice → focus the existing window (label dedupe).
 */
import { analytics } from "$lib/analytics/client";
import { isRecordingActive, peekRecastProject } from "$lib/ipc";
import { toast } from "@recast/ui/sonner";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";

function basename(path: string): string {
  return path.split(/[\\/]/).pop() ?? path;
}

function describeError(e: unknown): string {
  if (e instanceof Error) return e.message;
  if (typeof e === "string") return e;
  return String(e);
}

/**
 * Match the encoding used by the recasts list so a path that resolves to
 * the same on-disk file produces the same editor route and window label
 * (label-based dedupe in `openProjectInNewWindow` relies on this).
 */
export function encodeEditorPath(path: string): string {
  return encodeURIComponent(btoa(encodeURIComponent(path)));
}

/**
 * Open a path in a new editor webview, or focus the existing one if a
 * window for this path is already up. The label is derived from the
 * encoded path, so re-opening the same file is idempotent.
 */
export async function openProjectInNewWindow(path: string): Promise<void> {
  const encoded = encodeEditorPath(path);
  const route = `/editor/${encoded}`;
  const label = `editor-${encoded.replace(/[^a-zA-Z0-9]/g, "").slice(0, 48)}`;
  const existing = await WebviewWindow.getByLabel(label);
  if (existing) {
    await existing.setFocus();
    return;
  }
  new WebviewWindow(label, {
    url: route,
    title: `Editor - ${basename(path)}`,
    width: 1440,
    height: 960,
    center: true,
    decorations: false,
    transparent: true,
  });
  // No PII: the path never leaves.
  analytics.capture("editor_opened");
}

/**
 * Validate, then open. Toasts and bails on failure modes the editor can't
 * recover from:
 *   - Active recording → editor thumbnail probes would compete with capture
 *     for CPU; refuse rather than degrade the recording.
 *   - File missing/unreadable → toast the OS error verbatim.
 *   - Not a valid project → "Not a valid Recast project".
 */
export async function openProjectFromExternalPath(
  path: string,
): Promise<void> {
  // Best-effort guard: a failed IPC treats it as "not recording" rather than
  // blocking the open.
  let recording = false;
  try {
    recording = await isRecordingActive();
  } catch (e) {
    console.warn("[open-recast] is_recording_active probe failed", e);
  }
  if (recording) {
    toast.warning("Finish recording before opening another project");
    return;
  }

  try {
    await peekRecastProject(path);
  } catch (e) {
    toast.error(
      `Couldn't open "${basename(path)}": ${describeError(e)}`,
    );
    return;
  }

  await openProjectInNewWindow(path);
}
