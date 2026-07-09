//! Recast Cloud upload + share — the desktop side of "Record → Polish →
//! Share". Strictly **additive** and opt-in: recording, editing, and export
//! never touch any of this and never require an account or network. The
//! `.recast` on disk stays the source of truth; the cloud holds a derived
//! MP4 only.
//!
//! Flow (frontend orchestrates the export, Rust does the network):
//!   1. Frontend exports a web-ready MP4 (720p for Free / source for Pro)
//!      via the existing `export_video` command, then calls
//!      `recast_cloud_upload(mp4Path, title, workspaceId)`.
//!   2. POST /api/uploads/init  → reserves a draft recast, returns a signed
//!      PUT URL (files-sdk envelope).
//!   3. PUT the file to that URL.
//!   4. POST /api/uploads/complete → HEAD-verifies + publishes.
//!   5. POST /api/recasts/{id}/share { visibility: "public" } → share link.
//!
//! Auth reuses the device-flow bearer token from `auth.rs` (OS keyring) —
//! the frontend never sees the raw token. Progress is emitted as coarse
//! phase events (`recast-cloud:progress|complete|error`); the long-running
//! granular progress is the export step, which has its own `export-state`
//! events.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use reqwest::header;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use super::auth::{cloud_api_url, current_session_token, user_agent};
use super::error::{AppError, AppResult};

// ──────────────────────────────────────────────────────────────────────────
// HTTP helper (shared base + authed client, reused from the auth module)
// ──────────────────────────────────────────────────────────────────────────

/// Upload-tuned client: a generous connect timeout but NO overall timeout —
/// a 150 MB+ PUT over a slow link can legitimately run for minutes, and
/// auth.rs's 15s client would kill it.
fn cloud_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent(user_agent())
        .connect_timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| format!("http client init failed: {e}"))
}

fn token_or_err() -> Result<String, String> {
    current_session_token().ok_or_else(|| "Not signed in to Recast Cloud".to_string())
}

fn bearer(token: &str) -> String {
    format!("Bearer {token}")
}

/// Resolve the workspace to upload into. Honors an explicit id; otherwise
/// asks `/api/desktop/profile` for the user's `defaultWorkspaceId` (active
/// org, else first membership). Returns `None` only if the profile call
/// fails or the user belongs to no workspace — in which case `init` falls
/// back to the session's active org and surfaces a clear error if unset.
async fn resolve_workspace_id(
    client: &reqwest::Client,
    base: &str,
    token: &str,
    provided: Option<String>,
) -> Option<String> {
    if let Some(ws) = provided.filter(|s| !s.is_empty()) {
        return Some(ws);
    }
    let resp = client
        .get(format!("{base}/api/desktop/profile"))
        .header(header::AUTHORIZATION, bearer(token))
        .send()
        .await
        .ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let body = resp.json::<serde_json::Value>().await.ok()?;
    body.get("defaultWorkspaceId")
        .and_then(|v| v.as_str())
        .map(str::to_string)
}

// ──────────────────────────────────────────────────────────────────────────
// Local manifest — which local exports have a cloud copy, keyed by file path.
// Lets the library swap "Share to Cloud" → "Copy link / Manage" without a
// network round-trip. Independent of the cloud: deleting one never touches
// the other. Mirrors the Google Drive manifest pattern.
// ──────────────────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct CloudUploadRecord {
    pub recast_id: String,
    pub slug: String,
    pub share_url: String,
    /// Unix seconds.
    pub uploaded_at: u64,
}

fn manifest_path(app: &AppHandle) -> Option<PathBuf> {
    app.path()
        .app_data_dir()
        .ok()
        .map(|dir| dir.join("recast-cloud-uploads.json"))
}

fn read_manifest(app: &AppHandle) -> HashMap<String, CloudUploadRecord> {
    let Some(path) = manifest_path(app) else {
        return HashMap::new();
    };
    let Ok(data) = std::fs::read_to_string(&path) else {
        return HashMap::new();
    };
    serde_json::from_str(&data).unwrap_or_default()
}

fn write_manifest(app: &AppHandle, manifest: &HashMap<String, CloudUploadRecord>) {
    let Some(path) = manifest_path(app) else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(data) = serde_json::to_string_pretty(manifest) {
        let _ = std::fs::write(path, data);
    }
}

fn record_upload(app: &AppHandle, local_path: &str, record: CloudUploadRecord) {
    let mut manifest = read_manifest(app);
    manifest.insert(local_path.to_string(), record);
    write_manifest(app, &manifest);
}

fn forget_path(app: &AppHandle, local_path: &str) {
    let mut manifest = read_manifest(app);
    if manifest.remove(local_path).is_some() {
        write_manifest(app, &manifest);
    }
}

fn forget_by_recast_id(app: &AppHandle, recast_id: &str) {
    let mut manifest = read_manifest(app);
    let before = manifest.len();
    manifest.retain(|_, r| r.recast_id != recast_id);
    if manifest.len() != before {
        write_manifest(app, &manifest);
    }
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

// ──────────────────────────────────────────────────────────────────────────
// Events
// ──────────────────────────────────────────────────────────────────────────

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct CloudProgress<'a> {
    path: &'a str,
    /// "preparing" | "uploading" | "finalizing" | "sharing"
    phase: &'a str,
}

fn emit_progress(app: &AppHandle, path: &str, phase: &str) {
    let _ = app.emit("recast-cloud:progress", CloudProgress { path, phase });
}

/// Byte-level progress for the long-running PUT, so the share card can render a
/// determinate bar instead of an indeterminate pulse (mirrors Google Drive's
/// `gdrive:progress`). Keyed by the local file path, like every other
/// `recast-cloud:*` event.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct CloudUploadProgress<'a> {
    path: &'a str,
    bytes_sent: u64,
    total_bytes: u64,
}

/// Emit a failure event AND return the message, so the awaiting promise and
/// any event listener (corner notifications) both learn about it.
fn fail(app: &AppHandle, path: &str, message: String) -> String {
    let _ = app.emit(
        "recast-cloud:error",
        serde_json::json!({ "path": path, "message": message }),
    );
    message
}

// ──────────────────────────────────────────────────────────────────────────
// Wire types
// ──────────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct UploadEnvelope {
    method: String,
    url: String,
    headers: Option<HashMap<String, String>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct InitResp {
    recast_id: String,
    upload: UploadEnvelope,
    /// Optional PUT envelope for a poster WebP. Absent if the server couldn't
    /// sign one; the uploader then skips the poster.
    poster_upload: Option<UploadEnvelope>,
    /// Optional PUT envelope for a captions VTT track. Present only when the
    /// init request signalled `hasCaptions`.
    captions_upload: Option<UploadEnvelope>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ShareResp {
    slug: String,
    share_url: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CloudShareResult {
    pub recast_id: String,
    pub slug: String,
    pub share_url: String,
}

// ──────────────────────────────────────────────────────────────────────────
// Commands
// ──────────────────────────────────────────────────────────────────────────

/// Upload an already-exported MP4 to Recast Cloud and create a public share
/// link. `path` is the exported file (the caller runs `export_video` first);
/// `workspace_id` comes from `/api/desktop/profile`'s `defaultWorkspaceId`.
///
/// Returns the recast id + slug + share URL, and records the result in the
/// local manifest so the library row can switch to a "manage" affordance.
#[tauri::command]
pub async fn recast_cloud_upload(
    app: AppHandle,
    path: String,
    title: String,
    workspace_id: Option<String>,
    // Output-time transcript to publish as a selectable caption track. None /
    // empty → no track uploaded. Serialized to VTT here.
    captions_transcript: Option<crate::transcription::Transcript>,
) -> AppResult<CloudShareResult> {
    let token = token_or_err().map_err(|e| fail(&app, &path, e))?;
    let client = cloud_client().map_err(|e| fail(&app, &path, e))?;
    let base = cloud_api_url();

    emit_progress(&app, &path, "preparing");

    // Probe the exported MP4 for the real dimensions / duration / size. This
    // is authoritative; we don't trust caller-supplied numbers.
    let meta = super::editor::get_video_metadata(path.clone())
        .await
        .map_err(|e| fail(&app, &path, format!("Couldn't read video metadata: {e}")))?;
    let width = meta.width;
    let height = meta.height;
    let fps = (meta.fps.round() as i64).max(1) as u32;
    let duration_sec = meta.duration.round().max(0.0) as u64;
    let size_bytes = meta.size_bytes;

    // Best-effort poster (a single WebP frame). Generated off the main thread;
    // a failure here never blocks the upload — the recast just keeps no poster.
    let poster_src = path.clone();
    let poster_bytes = tauri::async_runtime::spawn_blocking(move || {
        super::editor::poster_webp_for_export(&poster_src)
    })
    .await
    .ok()
    .flatten();

    let resolved_workspace = resolve_workspace_id(&client, &base, &token, workspace_id).await;

    // ── init ──────────────────────────────────────────────────────────
    let mut init_body = serde_json::Map::new();
    init_body.insert("title".into(), title.trim().into());
    init_body.insert("durationSec".into(), duration_sec.into());
    init_body.insert("sizeBytes".into(), size_bytes.into());
    init_body.insert("width".into(), width.into());
    init_body.insert("height".into(), height.into());
    init_body.insert("fps".into(), fps.into());
    // zod treats workspaceId as optional-string — omit (not null) when absent.
    if let Some(ws) = resolved_workspace.as_ref().filter(|s| !s.is_empty()) {
        init_body.insert("workspaceId".into(), ws.clone().into());
    }
    // Serialize the caption track up front so we can both tell init to sign a
    // captions PUT URL and reuse the body for the upload below.
    let captions_vtt = captions_transcript
        .as_ref()
        .filter(|t| !t.segments.is_empty())
        .map(crate::transcription::subtitles::to_vtt);
    if captions_vtt.is_some() {
        init_body.insert("hasCaptions".into(), true.into());
    }

    let init_resp = client
        .post(format!("{base}/api/uploads/init"))
        .header(header::AUTHORIZATION, bearer(&token))
        .json(&init_body)
        .send()
        .await
        .map_err(|e| fail(&app, &path, format!("Upload init failed: {e}")))?;

    if !init_resp.status().is_success() {
        let status = init_resp.status();
        let body = init_resp.text().await.unwrap_or_default();
        return Err(fail(&app, &path, humanize_init_error(status.as_u16(), &body)).into());
    }

    let init: InitResp = init_resp
        .json()
        .await
        .map_err(|e| fail(&app, &path, format!("Upload init parse failed: {e}")))?;

    if init.upload.method.to_uppercase() != "PUT" {
        return Err(fail(
            &app,
            &path,
            "This storage provider isn't supported by the desktop uploader yet.".into(),
        )
        .into());
    }

    // ── PUT the file ──────────────────────────────────────────────────
    // Stream the in-memory buffer in chunks so we can emit byte-level progress
    // (mirrors the Google Drive uploader). We MUST still send an explicit
    // Content-Length — S3/R2/Azure reject a chunked PUT, and `wrap_stream`
    // otherwise defaults to `Transfer-Encoding: chunked`. Free uploads are
    // 720p-capped (~150 MB), comfortably in RAM; only one ~1 MiB chunk is
    // copied out of the buffer at a time.
    emit_progress(&app, &path, "uploading");
    let bytes = tokio::fs::read(&path)
        .await
        .map_err(|e| fail(&app, &path, format!("Couldn't read export file: {e}")))?;
    let total_bytes = bytes.len() as u64;

    // Surface the bar at 0% before the first chunk flushes.
    let _ = app.emit(
        "recast-cloud:upload-progress",
        CloudUploadProgress {
            path: &path,
            bytes_sent: 0,
            total_bytes,
        },
    );

    let envelope_headers = init.upload.headers.unwrap_or_default();
    let has_content_type = envelope_headers
        .keys()
        .any(|k| k.eq_ignore_ascii_case("content-type"));

    const PUT_CHUNK_SIZE: usize = 1024 * 1024; // 1 MiB → smooth bar, bounded event count
    let progress_app = app.clone();
    let progress_path = path.clone();
    let body_stream = futures_util::stream::unfold((bytes, 0usize), move |(buf, offset)| {
        let progress_app = progress_app.clone();
        let progress_path = progress_path.clone();
        async move {
            if offset >= buf.len() {
                return None;
            }
            let end = (offset + PUT_CHUNK_SIZE).min(buf.len());
            let chunk = buf[offset..end].to_vec();
            // Cumulative bytes handed to the transport, keyed by local path
            // to match the store's `uploads` map.
            let _ = progress_app.emit(
                "recast-cloud:upload-progress",
                CloudUploadProgress {
                    path: &progress_path,
                    bytes_sent: end as u64,
                    total_bytes,
                },
            );
            Some((Ok::<Vec<u8>, std::io::Error>(chunk), (buf, end)))
        }
    });

    let mut put = client
        .put(&init.upload.url)
        .header(header::CONTENT_LENGTH, total_bytes)
        .body(reqwest::Body::wrap_stream(body_stream));
    for (k, v) in &envelope_headers {
        put = put.header(k.as_str(), v.as_str());
    }
    if !has_content_type {
        put = put.header(header::CONTENT_TYPE, "video/mp4");
    }

    let put_resp = put
        .send()
        .await
        .map_err(|e| fail(&app, &path, format!("Upload failed: {e}")))?;
    if !put_resp.status().is_success() {
        let status = put_resp.status();
        return Err(fail(&app, &path, format!("Upload rejected ({status}).")).into());
    }

    // ── PUT the poster (best-effort) ────────────────────────────────────
    // Never fails the upload: if the WebP wasn't generated, the server didn't
    // sign a poster URL, or the PUT errors, we just report `hasPoster: false`.
    let mut has_poster = false;
    if let (Some(poster), Some(penv)) = (poster_bytes.as_ref(), init.poster_upload.as_ref()) {
        if penv.method.eq_ignore_ascii_case("PUT") {
            let pheaders = penv.headers.clone().unwrap_or_default();
            let pheader_has_ct = pheaders
                .keys()
                .any(|k| k.eq_ignore_ascii_case("content-type"));
            let mut preq = client.put(&penv.url).body(poster.clone());
            for (k, v) in &pheaders {
                preq = preq.header(k.as_str(), v.as_str());
            }
            if !pheader_has_ct {
                preq = preq.header(header::CONTENT_TYPE, "image/webp");
            }
            has_poster = preq
                .send()
                .await
                .map(|r| r.status().is_success())
                .unwrap_or(false);
        }
    }

    // ── PUT the captions VTT (best-effort) ──────────────────────────────
    // Like the poster: a failure here never fails the upload; the recast just
    // ships without a selectable caption track.
    let mut has_captions = false;
    if let (Some(vtt), Some(cenv)) = (captions_vtt.as_ref(), init.captions_upload.as_ref()) {
        if cenv.method.eq_ignore_ascii_case("PUT") {
            let cheaders = cenv.headers.clone().unwrap_or_default();
            let cheader_has_ct = cheaders
                .keys()
                .any(|k| k.eq_ignore_ascii_case("content-type"));
            let mut creq = client.put(&cenv.url).body(vtt.clone().into_bytes());
            for (k, v) in &cheaders {
                creq = creq.header(k.as_str(), v.as_str());
            }
            if !cheader_has_ct {
                creq = creq.header(header::CONTENT_TYPE, "text/vtt");
            }
            has_captions = creq
                .send()
                .await
                .map(|r| r.status().is_success())
                .unwrap_or(false);
        }
    }

    // ── complete ──────────────────────────────────────────────────────
    emit_progress(&app, &path, "finalizing");
    let complete_resp = client
        .post(format!("{base}/api/uploads/complete"))
        .header(header::AUTHORIZATION, bearer(&token))
        .json(&serde_json::json!({
            "recastId": init.recast_id,
            "width": width,
            "height": height,
            "fps": fps,
            "durationSec": duration_sec,
            "hasPoster": has_poster,
            "hasCaptions": has_captions,
        }))
        .send()
        .await
        .map_err(|e| fail(&app, &path, format!("Finalize failed: {e}")))?;

    if !complete_resp.status().is_success() {
        let status = complete_resp.status();
        let body = complete_resp.text().await.unwrap_or_default();
        return Err(fail(&app, &path, humanize_complete_error(status.as_u16(), &body)).into());
    }

    // ── share (public link) ───────────────────────────────────────────
    emit_progress(&app, &path, "sharing");
    let share_resp = client
        .post(format!("{base}/api/recasts/{}/share", init.recast_id))
        .header(header::AUTHORIZATION, bearer(&token))
        .json(&serde_json::json!({ "visibility": "public" }))
        .send()
        .await
        .map_err(|e| fail(&app, &path, format!("Creating share link failed: {e}")))?;

    if !share_resp.status().is_success() {
        let status = share_resp.status();
        let body = share_resp.text().await.unwrap_or_default();
        return Err(fail(
            &app,
            &path,
            format!("Creating share link failed ({status}): {body}"),
        )
        .into());
    }

    let share: ShareResp = share_resp
        .json()
        .await
        .map_err(|e| fail(&app, &path, format!("Share response parse failed: {e}")))?;

    let result = CloudShareResult {
        recast_id: init.recast_id,
        slug: share.slug,
        share_url: share.share_url,
    };

    record_upload(
        &app,
        &path,
        CloudUploadRecord {
            recast_id: result.recast_id.clone(),
            slug: result.slug.clone(),
            share_url: result.share_url.clone(),
            uploaded_at: now_unix(),
        },
    );

    let _ = app.emit(
        "recast-cloud:complete",
        serde_json::json!({
            "path": path,
            "recastId": result.recast_id,
            "slug": result.slug,
            "shareUrl": result.share_url,
        }),
    );

    Ok(result)
}

/// Update an existing share's settings. All knobs optional:
///   - `visibility`: "public" | "workspace" | "private" (None = unchanged)
///   - `password`:   None = unchanged; "" = remove; else set (≥4 chars)
///   - `expires_at`: None = unchanged; "" = clear; else ISO-8601 future date
#[tauri::command]
pub async fn recast_cloud_update_share(
    slug: String,
    visibility: Option<String>,
    password: Option<String>,
    expires_at: Option<String>,
) -> AppResult<()> {
    let token = token_or_err()?;
    let client = cloud_client()?;
    let base = cloud_api_url();

    // Visibility lives in /access, which speaks the legacy {public,team,
    // private} triplet — map "workspace" → "team".
    if let Some(v) = visibility.as_ref() {
        let mapped = match v.as_str() {
            "public" => "public",
            "workspace" | "team" => "team",
            "private" => "private",
            other => return Err(AppError::msg(format!("Unknown visibility: {other}"))),
        };
        let resp = client
            .patch(format!("{base}/api/share/{slug}/access"))
            .header(header::AUTHORIZATION, bearer(&token))
            .json(&serde_json::json!({ "visibility": mapped }))
            .send()
            .await
            .map_err(|e| AppError::msg(format!("Updating visibility failed: {e}")))?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(AppError::msg(format!(
                "Updating visibility failed ({status}): {body}"
            )));
        }
    }

    // Password + expiry go through /settings. Only send the keys provided so
    // we never clobber an unrelated field.
    let mut settings = serde_json::Map::new();
    if let Some(pw) = password {
        settings.insert(
            "password".into(),
            if pw.is_empty() {
                serde_json::Value::Null
            } else {
                pw.into()
            },
        );
    }
    if let Some(exp) = expires_at {
        settings.insert(
            "expiresAt".into(),
            if exp.is_empty() {
                serde_json::Value::Null
            } else {
                exp.into()
            },
        );
    }
    if !settings.is_empty() {
        let resp = client
            .patch(format!("{base}/api/share/{slug}/settings"))
            .header(header::AUTHORIZATION, bearer(&token))
            .json(&settings)
            .send()
            .await
            .map_err(|e| AppError::msg(format!("Updating share settings failed: {e}")))?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(AppError::msg(format!(
                "Updating share settings failed ({status}): {body}"
            )));
        }
    }

    Ok(())
}

/// Delete the cloud copy of a recast (blob + row + shares + usage). Never
/// touches the local `.recast`. `path`, when given, is forgotten from the
/// manifest so the library row reverts to "Share to Cloud".
#[tauri::command]
pub async fn recast_cloud_delete(
    app: AppHandle,
    recast_id: String,
    path: Option<String>,
) -> AppResult<()> {
    let token = token_or_err()?;
    let client = cloud_client()?;
    let base = cloud_api_url();

    let resp = client
        .delete(format!("{base}/api/recasts/{recast_id}"))
        .header(header::AUTHORIZATION, bearer(&token))
        .send()
        .await
        .map_err(|e| AppError::msg(format!("Deleting cloud copy failed: {e}")))?;

    // 404 = already gone; treat as success so the local manifest can heal.
    if !resp.status().is_success() && resp.status().as_u16() != 404 {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(AppError::msg(format!(
            "Deleting cloud copy failed ({status}): {body}"
        )));
    }

    if let Some(p) = path {
        forget_path(&app, &p);
    } else {
        forget_by_recast_id(&app, &recast_id);
    }
    Ok(())
}

/// List the shares for a recast (owner-only). Returned verbatim as JSON so
/// the manage UI can render whatever the server provides.
#[tauri::command]
pub async fn recast_cloud_list_shares(recast_id: String) -> AppResult<serde_json::Value> {
    let token = token_or_err()?;
    let client = cloud_client()?;
    let base = cloud_api_url();

    let resp = client
        .get(format!("{base}/api/recasts/{recast_id}/share"))
        .header(header::AUTHORIZATION, bearer(&token))
        .send()
        .await
        .map_err(|e| AppError::msg(format!("Listing shares failed: {e}")))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(AppError::msg(format!(
            "Listing shares failed ({status}): {body}"
        )));
    }
    resp.json()
        .await
        .map_err(|e| AppError::msg(format!("Share list parse failed: {e}")))
}

/// All locally-recorded cloud uploads, keyed by local export path.
///
/// Async + `spawn_blocking`: a plain `fn` Tauri command runs on the main
/// (UI) thread, so its `std::fs` manifest read would block the webview — the
/// macOS WKWebView freeze this project has been bitten by. The frontend
/// already awaits this, so moving the read onto a blocking worker is
/// transparent to call sites.
#[tauri::command]
pub async fn recast_cloud_list_uploads(app: AppHandle) -> HashMap<String, CloudUploadRecord> {
    tauri::async_runtime::spawn_blocking(move || read_manifest(&app))
        .await
        .unwrap_or_default()
}

/// Drop a manifest entry without any network call — for when the user removed
/// the cloud copy elsewhere, or the local file moved. Async + `spawn_blocking`
/// for the same reason as `recast_cloud_list_uploads`: keep the manifest
/// read-modify-write off the UI thread.
#[tauri::command]
pub async fn recast_cloud_forget_upload(app: AppHandle, path: String) -> AppResult<()> {
    tauri::async_runtime::spawn_blocking(move || forget_path(&app, &path))
        .await
        .map_err(|e| AppError::msg(format!("Forgetting upload failed: {e}")))
}

// ──────────────────────────────────────────────────────────────────────────
// Error humanization — turn the API's machine reasons into one-liners the
// corner-notification / toast can show directly.
// ──────────────────────────────────────────────────────────────────────────

fn reason_of(body: &str) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|v| {
            v.get("denial")
                .and_then(|d| d.get("reason"))
                .or_else(|| v.get("reason"))
                .and_then(|r| r.as_str())
                .map(str::to_string)
        })
}

fn humanize_init_error(status: u16, body: &str) -> String {
    match reason_of(body).as_deref() {
        Some("storage_over_cap") => "You're out of cloud storage. Upgrade or free up space.".into(),
        Some("active_recasts_over_cap") => {
            "You've hit your active share-link limit. Delete one or upgrade.".into()
        }
        Some("duration_over_cap") => {
            "This recording is longer than your plan allows for cloud sharing.".into()
        }
        Some("resolution_over_cap") => {
            "Your plan caps cloud sharing at 720p. Export at 720p, or upgrade for HD.".into()
        }
        _ if status == 401 => "Your Recast Cloud session expired. Sign in again.".into(),
        _ if status == 403 => "You don't have access to that workspace.".into(),
        _ => format!("Upload init failed ({status})."),
    }
}

fn humanize_complete_error(status: u16, body: &str) -> String {
    match reason_of(body).as_deref() {
        Some("upload_missing") => "The upload didn't arrive — please try again.".into(),
        Some("empty_upload") => "The uploaded file was empty — please try again.".into(),
        Some("storage_over_cap") => "You're out of cloud storage. Upgrade or free up space.".into(),
        Some("resolution_over_cap") => {
            "Your plan caps cloud sharing at 720p. Export at 720p, or upgrade for HD.".into()
        }
        _ => format!("Finalize failed ({status})."),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bearer_prefixes_the_token() {
        assert_eq!(bearer("abc123"), "Bearer abc123");
    }

    #[test]
    fn reason_of_reads_top_level_reason() {
        assert_eq!(
            reason_of(r#"{"reason":"storage_over_cap"}"#).as_deref(),
            Some("storage_over_cap"),
        );
    }

    #[test]
    fn reason_of_reads_nested_denial_reason() {
        assert_eq!(
            reason_of(r#"{"denial":{"reason":"duration_over_cap"}}"#).as_deref(),
            Some("duration_over_cap"),
        );
    }

    #[test]
    fn reason_of_returns_none_for_non_json_or_missing_reason() {
        assert_eq!(reason_of("not json at all"), None);
        assert_eq!(reason_of(r#"{"foo":"bar"}"#), None);
    }

    #[test]
    fn humanize_init_error_prefers_reason_over_status() {
        assert_eq!(
            humanize_init_error(500, r#"{"reason":"storage_over_cap"}"#),
            "You're out of cloud storage. Upgrade or free up space.",
        );
    }

    #[test]
    fn humanize_init_error_falls_back_to_status() {
        assert_eq!(
            humanize_init_error(401, "{}"),
            "Your Recast Cloud session expired. Sign in again.",
        );
        assert_eq!(
            humanize_init_error(403, "{}"),
            "You don't have access to that workspace.",
        );
        assert_eq!(humanize_init_error(500, "{}"), "Upload init failed (500).");
    }

    #[test]
    fn humanize_complete_error_maps_reason_and_status() {
        assert_eq!(
            humanize_complete_error(200, r#"{"reason":"upload_missing"}"#),
            "The upload didn't arrive — please try again.",
        );
        assert_eq!(humanize_complete_error(500, "{}"), "Finalize failed (500).");
    }
}
