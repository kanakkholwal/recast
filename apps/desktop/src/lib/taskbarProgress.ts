/**
 * Taskbar (Windows) / dock (macOS) progress for the current window during a
 * long job like export. No-ops on failure and on Linux where unsupported.
 */
import { getCurrentWindow, ProgressBarStatus } from "@tauri-apps/api/window";

export async function setJobProgress(pct: number): Promise<void> {
  try {
    await getCurrentWindow().setProgressBar({
      status: ProgressBarStatus.Normal,
      progress: Math.round(Math.min(Math.max(pct, 0), 100)),
    });
  } catch {
    // Unsupported platform or window gone; ignore.
  }
}

export async function setJobProgressIndeterminate(): Promise<void> {
  try {
    await getCurrentWindow().setProgressBar({
      status: ProgressBarStatus.Indeterminate,
    });
  } catch {
    // ignore
  }
}

export async function clearJobProgress(): Promise<void> {
  try {
    await getCurrentWindow().setProgressBar({ status: ProgressBarStatus.None });
  } catch {
    // ignore
  }
}
