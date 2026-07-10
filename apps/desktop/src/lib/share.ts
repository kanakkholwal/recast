/**
 * Native OS share-sheet helpers, backed by `tauri-plugin-sharekit`.
 *
 * Replaces `navigator.share`: Web Share Level 2 in WebView2 rejects `video/*`
 * file payloads via `canShare`. Sharekit bridges to the real native share sheets,
 * which accept any file. Errors never throw; they return via `ShareResult` so
 * callers can branch on cancelled/unsupported/error without try/catch.
 */

import {
  shareFile as sharekitShareFile,
  shareText as sharekitShareText,
} from "@choochmeque/tauri-plugin-sharekit-api";

export type ShareTextPayload = {
  title?: string;
  text?: string;
  url?: string;
};

export type ShareFilePayload = ShareTextPayload & {
  path: string;
  fileName?: string;
  mimeType?: string;
};

export type ShareResult =
  | { ok: true }
  | {
      ok: false;
      reason: "cancelled" | "unsupported" | "error";
      message?: string;
    };

function deriveFileName(path: string): string {
  return path.split(/[\\/]/).pop() ?? "recording";
}

function deriveMimeType(fileName: string): string {
  const ext = fileName.split(".").pop()?.toLowerCase();
  switch (ext) {
    case "mp4":
      return "video/mp4";
    case "webm":
      return "video/webm";
    case "mkv":
      return "video/x-matroska";
    case "mov":
      return "video/quicktime";
    case "gif":
      return "image/gif";
    case "png":
      return "image/png";
    case "jpg":
    case "jpeg":
      return "image/jpeg";
    default:
      return "application/octet-stream";
  }
}

// Sharekit needs `file://` URIs: Windows `C:\…` → `file:///C:/…`, POSIX → `file://…`.
function toFileUri(path: string): string {
  if (/^file:\/\//i.test(path)) return path;
  const normalized = path.replace(/\\/g, "/");
  if (/^[a-zA-Z]:\//.test(normalized)) {
    return `file:///${normalized}`;
  }
  if (normalized.startsWith("/")) {
    return `file://${normalized}`;
  }
  return normalized;
}

// Distinguish cancellation vs. real-unsupported vs. opaque failure for the toast.
function classify(e: unknown): ShareResult {
  const message = e instanceof Error ? e.message : String(e ?? "");
  const lower = message.toLowerCase();
  if (lower.includes("cancel")) {
    return { ok: false, reason: "cancelled" };
  }
  if (
    lower.includes("not supported") ||
    lower.includes("unsupported") ||
    lower.includes("not implemented")
  ) {
    return { ok: false, reason: "unsupported", message };
  }
  return { ok: false, reason: "error", message };
}

// Static true, kept so UI-gating call sites don't change.
export function isShareSupported(): boolean {
  return true;
}

export function isFileShareSupported(): boolean {
  return true;
}

export async function shareLink(payload: ShareTextPayload): Promise<ShareResult> {
  // `shareText` takes a single string, so compose title/text/url into one.
  const parts = [payload.title, payload.text, payload.url].filter(
    (s): s is string => typeof s === "string" && s.length > 0,
  );
  const text = parts.join("\n");
  if (!text) {
    return {
      ok: false,
      reason: "error",
      message: "Nothing to share (empty payload).",
    };
  }
  try {
    await sharekitShareText(text);
    return { ok: true };
  } catch (e) {
    console.error("[share] shareLink failed", e);
    return classify(e);
  }
}

export async function shareFile(payload: ShareFilePayload): Promise<ShareResult> {
  const fileName = payload.fileName ?? deriveFileName(payload.path);
  const mimeType = payload.mimeType ?? deriveMimeType(fileName);
  const uri = toFileUri(payload.path);
  try {
    await sharekitShareFile(uri, {
      mimeType,
      title: payload.title ?? fileName,
    });
    return { ok: true };
  } catch (e) {
    console.error("[share] shareFile failed", e, { path: payload.path });
    return classify(e);
  }
}

/**
 * Share a recording: try the file first, then fall back to a link (e.g. a Drive
 * webViewLink) if the runtime can't share files.
 */
export async function shareRecording(opts: {
  path: string;
  fileName: string;
  title?: string;
  text?: string;
  fallbackLink?: string;
}): Promise<ShareResult> {
  const fileResult = await shareFile({
    path: opts.path,
    fileName: opts.fileName,
    title: opts.title ?? opts.fileName,
    text: opts.text,
  });

  if (fileResult.ok || fileResult.reason === "cancelled") return fileResult;

  if (fileResult.reason === "unsupported" && opts.fallbackLink) {
    return shareLink({
      title: opts.title ?? opts.fileName,
      text: opts.text,
      url: opts.fallbackLink,
    });
  }

  return fileResult;
}
