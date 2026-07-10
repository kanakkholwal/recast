/**
 * Native OS notifications for long jobs (export, upload). Only fires when the
 * current window is unfocused, so it complements the in-app toast rather than
 * doubling it. Permission is requested once, lazily.
 */
import { getCurrentWindow } from "@tauri-apps/api/window";

let permission: "granted" | "denied" | "unknown" = "unknown";

async function ensurePermission(): Promise<boolean> {
  if (permission === "granted") return true;
  if (permission === "denied") return false;
  const { isPermissionGranted, requestPermission } = await import(
    "@tauri-apps/plugin-notification"
  );
  let granted = await isPermissionGranted();
  if (!granted) granted = (await requestPermission()) === "granted";
  permission = granted ? "granted" : "denied";
  return granted;
}

/**
 * Send a notification for a finished job. No-op when the window is focused (the
 * user is already looking) or permission is denied.
 */
export async function notifyJobDone(title: string, body: string): Promise<void> {
  try {
    if (await getCurrentWindow().isFocused()) return;
    if (!(await ensurePermission())) return;
    const { sendNotification } = await import(
      "@tauri-apps/plugin-notification"
    );
    sendNotification({ title, body });
  } catch (e) {
    console.warn("[notify] failed", e);
  }
}
