//! Google Drive integration: connect/disconnect, status, resumable upload.
//!
//! Auth: OAuth 2.0 Authorization Code + PKCE via loopback redirect — the
//! standard "desktop app" flow Google blesses for installed clients. The
//! refresh token is persisted in the OS keyring (DPAPI on Windows, Keychain
//! on macOS, SecretService on Linux); access tokens live only in memory and
//! are refreshed on demand.
//!
//! Upload: files are uploaded into a `/Recast/` folder under the user's My
//! Drive. The folder is created on first upload. Uploads use the resumable
//! protocol so we can stream chunks and emit per-chunk progress without
//! holding the whole file in memory. Files stay **private** — we do not
//! issue `permissions.create`. The returned `webViewLink` is the Drive UI
//! URL the owner can use to view the file or share it manually.
//!
//! Scopes:
//!   * `drive.file` — read/write only the files this app creates. Least
//!     privileged option; avoids the "restricted scope" Google verification
//!     bar that full `drive` access would require.
//!   * `userinfo.email` — populate the connected-account display in
//!     Settings.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::{Duration, Instant};

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use keyring::Entry;
use parking_lot::Mutex;
use rand::Rng;
use reqwest::{header, Client};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;

use super::error::{AppError, AppResult};

const KEYRING_SERVICE: &str = "com.nexonauts.recast";
const KEYRING_ENTRY: &str = "gdrive-refresh-token";
const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const REVOKE_URL: &str = "https://oauth2.googleapis.com/revoke";
const USERINFO_URL: &str = "https://www.googleapis.com/oauth2/v3/userinfo";
const DRIVE_API: &str = "https://www.googleapis.com/drive/v3";
const DRIVE_UPLOAD: &str = "https://www.googleapis.com/upload/drive/v3/files";
const RECAST_FOLDER_NAME: &str = "Recast";
const SCOPES: &str =
    "https://www.googleapis.com/auth/drive.file https://www.googleapis.com/auth/userinfo.email";
/// 8 MB per chunk — Drive's resumable spec requires multiples of 256 KB and
/// recommends 8 MB+ for bandwidth efficiency. Larger chunks pin more memory
/// per upload and worsen retry granularity, smaller waste round-trips.
const UPLOAD_CHUNK_SIZE: usize = 8 * 1024 * 1024;
/// How long the loopback listener waits for the browser callback before
/// giving up. Google's consent page is fast; 5 minutes covers slow networks
/// + a user who walks away briefly to grab a different account.
const OAUTH_CALLBACK_TIMEOUT: Duration = Duration::from_secs(300);

/// Compile-time OAuth client credentials. Release builds bake in whatever
/// `cargo build` sees in `GOOGLE_OAUTH_CLIENT_ID`/`_SECRET` (CI injects these
/// from repo secrets); dev `tauri dev` falls back to the runtime env (loaded
/// from `apps/desktop/.env` via dotenvy in `lib.rs`).
fn client_id() -> Option<String> {
    option_env!("GOOGLE_OAUTH_CLIENT_ID")
        .map(str::to_string)
        .or_else(|| std::env::var("GOOGLE_OAUTH_CLIENT_ID").ok())
        .filter(|s| !s.is_empty())
}

fn client_secret() -> Option<String> {
    option_env!("GOOGLE_OAUTH_CLIENT_SECRET")
        .map(str::to_string)
        .or_else(|| std::env::var("GOOGLE_OAUTH_CLIENT_SECRET").ok())
        .filter(|s| !s.is_empty())
}

fn require_credentials() -> Result<(String, String), String> {
    match (client_id(), client_secret()) {
        (Some(id), Some(secret)) => Ok((id, secret)),
        _ => Err(
            "Google OAuth client not configured. Set GOOGLE_OAUTH_CLIENT_ID and \
             GOOGLE_OAUTH_CLIENT_SECRET in apps/desktop/.env (or as build-time env \
             vars) and rebuild."
                .into(),
        ),
    }
}

// ──────────────────────────────────────────────────────────────────────────
// In-memory state
// ──────────────────────────────────────────────────────────────────────────

#[derive(Clone)]
struct CachedAccessToken {
    token: String,
    /// Wall-clock instant the token is no longer trusted. We refresh
    /// 60 seconds before this so a request mid-flight doesn't 401.
    expires_at: Instant,
}

static ACCESS_TOKEN: Mutex<Option<CachedAccessToken>> = Mutex::new(None);
static ACTIVE_UPLOADS: Mutex<Option<HashMap<String, Arc<AtomicBool>>>> = Mutex::new(None);

/// Persistent record of which local exports have been uploaded to Drive,
/// indexed by the local file path so the exports list can switch its menu
/// from "Upload to Drive" to "Copy link / Re-upload" without hitting the
/// network. Stored on disk as JSON in the app data dir — no database.
/// Re-uploads overwrite the previous entry so users always see the latest
/// Drive link.
#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct UploadRecord {
    pub file_id: String,
    pub name: String,
    pub web_view_link: Option<String>,
    /// Unix seconds. Lets the UI sort or stamp "uploaded 2 minutes ago"
    /// if we ever want that. Cheap to record now, expensive to backfill.
    pub uploaded_at: u64,
}

fn manifest_path(app: &AppHandle) -> Option<PathBuf> {
    app.path()
        .app_data_dir()
        .ok()
        .map(|dir| dir.join("drive-uploads.json"))
}

fn read_manifest(app: &AppHandle) -> HashMap<String, UploadRecord> {
    let Some(path) = manifest_path(app) else {
        return HashMap::new();
    };
    let Ok(data) = std::fs::read_to_string(&path) else {
        return HashMap::new();
    };
    serde_json::from_str(&data).unwrap_or_default()
}

fn write_manifest(app: &AppHandle, manifest: &HashMap<String, UploadRecord>) {
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

fn record_upload(app: &AppHandle, local_path: &str, record: UploadRecord) {
    let mut manifest = read_manifest(app);
    manifest.insert(local_path.to_string(), record);
    write_manifest(app, &manifest);
}

fn upload_cancel_flag(upload_id: &str) -> Arc<AtomicBool> {
    let mut guard = ACTIVE_UPLOADS.lock();
    let map = guard.get_or_insert_with(HashMap::new);
    map.entry(upload_id.to_string())
        .or_insert_with(|| Arc::new(AtomicBool::new(false)))
        .clone()
}

fn drop_upload_cancel_flag(upload_id: &str) {
    if let Some(map) = ACTIVE_UPLOADS.lock().as_mut() {
        map.remove(upload_id);
    }
}

fn signal_upload_cancel(upload_id: &str) {
    if let Some(map) = ACTIVE_UPLOADS.lock().as_ref() {
        if let Some(flag) = map.get(upload_id) {
            flag.store(true, Ordering::Relaxed);
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────
// Keyring (refresh token) helpers
// ──────────────────────────────────────────────────────────────────────────

fn keyring_entry() -> keyring::Result<Entry> {
    Entry::new(KEYRING_SERVICE, KEYRING_ENTRY)
}

fn read_refresh_token() -> Option<String> {
    keyring_entry().ok().and_then(|e| e.get_password().ok())
}

fn store_refresh_token(token: &str) -> Result<(), String> {
    keyring_entry()
        .and_then(|e| e.set_password(token))
        .map_err(|e| format!("keyring write failed: {e}"))
}

fn delete_refresh_token() -> Result<(), String> {
    match keyring_entry() {
        Ok(entry) => match entry.delete_credential() {
            Ok(()) => Ok(()),
            Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(format!("keyring delete failed: {e}")),
        },
        Err(e) => Err(format!("keyring open failed: {e}")),
    }
}

// ──────────────────────────────────────────────────────────────────────────
// PKCE + state generation
// ──────────────────────────────────────────────────────────────────────────

fn random_url_safe_string(len: usize) -> String {
    // RFC 7636 §4.1: code_verifier = high-entropy cryptographic random string
    // using the URL/filename-safe alphabet [A-Z][a-z][0-9]-._~. We sample from
    // that exact charset to avoid needing further encoding.
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~";
    let mut rng = rand::thread_rng();
    (0..len)
        .map(|_| {
            let idx = rng.gen_range(0..ALPHABET.len());
            ALPHABET[idx] as char
        })
        .collect()
}

fn pkce_challenge(verifier: &str) -> String {
    let digest = Sha256::digest(verifier.as_bytes());
    URL_SAFE_NO_PAD.encode(digest)
}

// ──────────────────────────────────────────────────────────────────────────
// HTTP client
// ──────────────────────────────────────────────────────────────────────────

fn http_client() -> Result<Client, String> {
    Client::builder()
        .user_agent(format!("Recast/{} (gdrive)", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| format!("http client init failed: {e}"))
}

// ──────────────────────────────────────────────────────────────────────────
// Token exchange + refresh
// ──────────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    expires_in: u64,
    #[serde(default)]
    refresh_token: Option<String>,
}

async fn exchange_code_for_tokens(
    client: &Client,
    code: &str,
    verifier: &str,
    redirect_uri: &str,
) -> Result<TokenResponse, String> {
    let (id, secret) = require_credentials()?;
    let params = [
        ("code", code),
        ("client_id", id.as_str()),
        ("client_secret", secret.as_str()),
        ("redirect_uri", redirect_uri),
        ("grant_type", "authorization_code"),
        ("code_verifier", verifier),
    ];
    let resp = client
        .post(TOKEN_URL)
        .form(&params)
        .send()
        .await
        .map_err(|e| format!("token exchange request failed: {e}"))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("token exchange returned {status}: {body}"));
    }
    resp.json::<TokenResponse>()
        .await
        .map_err(|e| format!("token exchange parse failed: {e}"))
}

async fn refresh_access_token(
    client: &Client,
    refresh_token: &str,
) -> Result<TokenResponse, String> {
    let (id, secret) = require_credentials()?;
    let params = [
        ("client_id", id.as_str()),
        ("client_secret", secret.as_str()),
        ("refresh_token", refresh_token),
        ("grant_type", "refresh_token"),
    ];
    let resp = client
        .post(TOKEN_URL)
        .form(&params)
        .send()
        .await
        .map_err(|e| format!("refresh request failed: {e}"))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("refresh returned {status}: {body}"));
    }
    resp.json::<TokenResponse>()
        .await
        .map_err(|e| format!("refresh parse failed: {e}"))
}

/// Returns a valid access token, refreshing if cached one is stale (or
/// absent). Errors out if the user isn't connected at all.
async fn ensure_access_token(client: &Client) -> Result<String, String> {
    // Fast path: cached, fresh.
    if let Some(cached) = ACCESS_TOKEN.lock().clone() {
        if cached.expires_at > Instant::now() + Duration::from_secs(60) {
            return Ok(cached.token);
        }
    }
    let refresh = read_refresh_token()
        .ok_or_else(|| "Not connected to Google Drive. Connect from Settings.".to_string())?;
    let resp = refresh_access_token(client, &refresh).await?;
    let token = resp.access_token.clone();
    let cached = CachedAccessToken {
        token: token.clone(),
        expires_at: Instant::now() + Duration::from_secs(resp.expires_in),
    };
    *ACCESS_TOKEN.lock() = Some(cached);
    // Google sometimes rotates the refresh token. Persist if present.
    if let Some(new_refresh) = resp.refresh_token {
        let _ = store_refresh_token(&new_refresh);
    }
    Ok(token)
}

// ──────────────────────────────────────────────────────────────────────────
// Loopback OAuth flow
// ──────────────────────────────────────────────────────────────────────────

/// HTML escape for the few characters that could break out of attributes
/// or open tags when interpolating an `error` string Google handed us into
/// the callback page. The error values Google returns are short codes
/// (`access_denied`, `invalid_grant`, …) but we don't fully trust them.
fn html_escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(ch),
        }
    }
    out
}

/// Renders the loopback callback page shown in the user's browser after
/// the OAuth redirect lands. Styling is fully self-contained (no external
/// CSS) and mirrors Recast's in-app design language: dark canvas, glass
/// card with subtle border + shadow, primary-green accent, system font
/// stack with deliberate letter-spacing. The page picks light or dark
/// mode from the OS via `prefers-color-scheme` so it doesn't jar a user
/// whose browser is in light mode.
///
/// Two states: success (consent approved, code captured) and error (user
/// denied, or Google returned an `error=…` query param).
fn render_callback_page(error: Option<&str>) -> (String, &'static str) {
    // Recast brand: primary is a vivid lime-green (`oklch(76% 0.21 125.904)`
    // in light mode, `oklch(92% 0.23 125.904)` in dark). The page uses
    // CSS custom properties + a `prefers-color-scheme` swap so users see
    // the right palette without us shipping two pages.
    let success_icon = "M4.5 12.75l6 6 9-13.5";
    let error_icon = "M12 9v3.75m9-.75a9 9 0 11-18 0 9 9 0 0118 0zm-9 3.75h.008v.008H12v-.008z";
    // Recast brand mark — three vertical rounded bars on a rounded square,
    // mirroring `apps/desktop/src/components/logo.svelte`. Kept as raw
    // <rect> markup so the silhouette is pixel-identical to the in-app
    // and installer icon. `--brand-fill` / `--brand-bars` flip with the
    // OS color scheme below.
    let recast_logo = r#"<svg class="brand-mark" viewBox="0 0 512 512" xmlns="http://www.w3.org/2000/svg" aria-hidden="true">
      <rect width="512" height="512" rx="256" fill="var(--brand-fill)"/>
      <rect x="111" y="166" width="60" height="180" rx="30" fill="var(--brand-bars)"/>
      <rect x="230" y="166" width="60" height="180" rx="30" fill="var(--brand-bars)"/>
      <rect x="349" y="166" width="60" height="180" rx="30" fill="var(--brand-bars)"/>
    </svg>"#;
    let (title, heading, sub, icon_path, accent_class, http_status) = match error {
        Some(err) => (
            "Sign-in failed - Recast",
            "Google Drive sign-in failed".to_string(),
            format!(
                "Google reported <code>{}</code>. You can close this tab and try again from Recast.",
                html_escape(err)
            ),
            error_icon,
            "icon icon--error",
            "400 Bad Request",
        ),
        None => (
            "Connected - Recast",
            "Recast is connected to Google Drive".to_string(),
            "You can close this tab and return to the app. New exports will be \
             uploadable straight to your private <strong>Recast</strong> folder."
                .to_string(),
            success_icon,
            "icon icon--success",
            "200 OK",
        ),
    };

    let body = format!(
        r##"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>{title}</title>
<style>
  :root {{
    color-scheme: dark light;
    --bg: #0b0c0e;
    --bg-grad-a: #0c1213;
    --bg-grad-b: #07090a;
    --card: rgba(255,255,255,0.04);
    --card-border: rgba(255,255,255,0.08);
    --card-ring: rgba(255,255,255,0.04);
    --fg: #ededed;
    --fg-muted: rgba(237,237,237,0.62);
    --fg-subtle: rgba(237,237,237,0.42);
    --primary: oklch(92% 0.23 125.904);
    --primary-soft: oklch(92% 0.23 125.904 / 0.15);
    --primary-ring: oklch(92% 0.23 125.904 / 0.3);
    --error: oklch(70% 0.2 25);
    --error-soft: oklch(70% 0.2 25 / 0.14);
    --error-ring: oklch(70% 0.2 25 / 0.28);
    --code-bg: rgba(255,255,255,0.06);
    /* Brand mark colors — mirror `logo.svelte`'s dark-mode mapping:
       white rounded square (the "background") with black bars on top. */
    --brand-fill: #ffffff;
    --brand-bars: #000000;
  }}
  @media (prefers-color-scheme: light) {{
    :root {{
      --bg: #f5f5f4;
      --bg-grad-a: #fafafa;
      --bg-grad-b: #ececec;
      --card: rgba(255,255,255,0.92);
      --card-border: rgba(0,0,0,0.06);
      --card-ring: rgba(0,0,0,0.04);
      --fg: #0b0b0c;
      --fg-muted: rgba(11,11,12,0.62);
      --fg-subtle: rgba(11,11,12,0.42);
      --primary: oklch(76% 0.21 125.904);
      --primary-soft: oklch(76% 0.21 125.904 / 0.12);
      --primary-ring: oklch(76% 0.21 125.904 / 0.3);
      --error: oklch(63% 0.21 25);
      --error-soft: oklch(63% 0.21 25 / 0.12);
      --error-ring: oklch(63% 0.21 25 / 0.28);
      --code-bg: rgba(0,0,0,0.05);
      /* Light-mode mark: black rounded square, white bars. */
      --brand-fill: #000000;
      --brand-bars: #ffffff;
    }}
  }}
  * {{ box-sizing: border-box; }}
  html, body {{ height: 100%; }}
  body {{
    margin: 0;
    font-family: -apple-system, BlinkMacSystemFont, "Inter", "Segoe UI",
                 system-ui, sans-serif;
    color: var(--fg);
    background:
      radial-gradient(60rem 40rem at 50% -10%, var(--primary-soft), transparent 60%),
      linear-gradient(180deg, var(--bg-grad-a), var(--bg-grad-b));
    background-color: var(--bg);
    display: grid;
    place-items: center;
    padding: 1.5rem;
    -webkit-font-smoothing: antialiased;
    -moz-osx-font-smoothing: grayscale;
  }}
  .brand {{
    position: fixed;
    top: 1.25rem;
    left: 1.25rem;
    display: inline-flex;
    align-items: center;
    gap: 0.5rem;
    font-size: 0.78rem;
    font-weight: 600;
    letter-spacing: 0.06em;
    color: var(--fg-muted);
    text-transform: uppercase;
  }}
  .brand .brand-mark {{
    width: 1.15rem;
    height: 1.15rem;
    display: block;
    border-radius: 50%;
    box-shadow: 0 0 0 1px var(--card-border);
  }}
  .card {{
    width: min(28rem, 100%);
    border-radius: 1rem;
    background: var(--card);
    border: 1px solid var(--card-border);
    box-shadow:
      0 1px 0 var(--card-ring) inset,
      0 20px 50px -20px rgba(0,0,0,0.55),
      0 8px 16px -8px rgba(0,0,0,0.35);
    padding: 1.75rem 1.75rem 1.5rem;
    text-align: left;
    backdrop-filter: blur(14px);
    -webkit-backdrop-filter: blur(14px);
  }}
  .icon {{
    width: 2.5rem;
    height: 2.5rem;
    border-radius: 0.75rem;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    margin-bottom: 1.25rem;
    border: 1px solid;
  }}
  .icon svg {{
    width: 1.25rem;
    height: 1.25rem;
  }}
  .icon--success {{
    background: var(--primary-soft);
    color: var(--primary);
    border-color: var(--primary-ring);
  }}
  .icon--error {{
    background: var(--error-soft);
    color: var(--error);
    border-color: var(--error-ring);
  }}
  h1 {{
    margin: 0 0 0.5rem;
    font-size: 1.25rem;
    font-weight: 600;
    letter-spacing: -0.01em;
    line-height: 1.25;
  }}
  p {{
    margin: 0 0 1.25rem;
    color: var(--fg-muted);
    font-size: 0.92rem;
    line-height: 1.55;
  }}
  p strong {{ color: var(--fg); font-weight: 600; }}
  code {{
    font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
    font-size: 0.85em;
    background: var(--code-bg);
    padding: 0.1em 0.4em;
    border-radius: 0.375rem;
  }}
  .hint {{
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding-top: 1rem;
    border-top: 1px solid var(--card-border);
    color: var(--fg-subtle);
    font-size: 0.78rem;
    letter-spacing: 0.01em;
  }}
  .hint kbd {{
    font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
    font-size: 0.7rem;
    padding: 0.15rem 0.4rem;
    border-radius: 0.3rem;
    background: var(--code-bg);
    border: 1px solid var(--card-border);
    color: var(--fg-muted);
    line-height: 1;
  }}
  .dot {{
    width: 0.35rem;
    height: 0.35rem;
    border-radius: 999px;
    background: var(--primary);
    box-shadow: 0 0 0 4px var(--primary-soft);
    animation: pulse 1.6s ease-in-out infinite;
  }}
  @keyframes pulse {{
    0%, 100% {{ opacity: 1; }}
    50% {{ opacity: 0.4; }}
  }}
  @media (prefers-reduced-motion: reduce) {{
    .dot {{ animation: none; }}
  }}
</style>
</head>
<body>
  <div class="brand" aria-label="Recast">
    {recast_logo}
    Recast
  </div>
  <main class="card" role="status" aria-live="polite">
    <span class="{accent_class}" aria-hidden="true">
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"
           stroke-linecap="round" stroke-linejoin="round">
        <path d="{icon_path}"/>
      </svg>
    </span>
    <h1>{heading}</h1>
    <p>{sub}</p>
    <div class="hint">
      <span class="dot" aria-hidden="true"></span>
      <span>Return to Recast — you can close this tab anytime.</span>
    </div>
  </main>
</body>
</html>"##,
        title = title,
        recast_logo = recast_logo,
        accent_class = accent_class,
        icon_path = icon_path,
        heading = heading,
        sub = sub,
    );

    (body, http_status)
}

/// Bind a TCP listener to a kernel-chosen port on 127.0.0.1, await one
/// HTTP GET callback, parse `code` and `state` from the query string,
/// respond with a friendly success page, and return the parsed query.
async fn await_oauth_callback(
    listener: TcpListener,
    expected_state: &str,
) -> Result<String, String> {
    let accept = tokio::time::timeout(OAUTH_CALLBACK_TIMEOUT, listener.accept())
        .await
        .map_err(|_| "Timed out waiting for Google sign-in. Try again.".to_string())?
        .map_err(|e| format!("accept failed: {e}"))?;
    let (mut socket, _) = accept;

    let mut reader = BufReader::new(&mut socket);
    let mut request_line = String::new();
    reader
        .read_line(&mut request_line)
        .await
        .map_err(|e| format!("read failed: {e}"))?;
    // Request line: `GET /callback?code=...&state=... HTTP/1.1\r\n`
    // We don't care about headers/body; close right after responding.
    let path_query = request_line
        .split_whitespace()
        .nth(1)
        .ok_or_else(|| "malformed HTTP request".to_string())?;
    let query = path_query.split_once('?').map(|(_, q)| q).unwrap_or("");

    let mut code: Option<String> = None;
    let mut state: Option<String> = None;
    let mut error: Option<String> = None;
    for pair in query.split('&') {
        let Some((key, value)) = pair.split_once('=') else {
            continue;
        };
        let decoded = urlencoding::decode(value).map(|s| s.into_owned()).ok();
        match key {
            "code" => code = decoded,
            "state" => state = decoded,
            "error" => error = decoded,
            _ => {}
        }
    }

    let (body, status) = render_callback_page(error.as_deref());
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\n\
         Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = socket.write_all(response.as_bytes()).await;
    let _ = socket.shutdown().await;

    if let Some(err) = error {
        return Err(format!("Google returned error: {err}"));
    }
    let code = code.ok_or_else(|| "no code in callback".to_string())?;
    let returned_state = state.ok_or_else(|| "no state in callback".to_string())?;
    if returned_state != expected_state {
        return Err("state mismatch — possible cross-site request forgery".into());
    }
    Ok(code)
}

#[derive(Serialize, Clone)]
pub struct GdriveStatus {
    pub connected: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
}

#[derive(Deserialize)]
struct UserInfo {
    #[serde(default)]
    email: Option<String>,
}

async fn fetch_email(client: &Client, access_token: &str) -> Option<String> {
    let resp = client
        .get(USERINFO_URL)
        .header(header::AUTHORIZATION, format!("Bearer {access_token}"))
        .send()
        .await
        .ok()?;
    if !resp.status().is_success() {
        return None;
    }
    resp.json::<UserInfo>().await.ok().and_then(|u| u.email)
}

/// Kicks off the loopback OAuth flow. Opens the browser to Google's consent
/// page, awaits the redirect, exchanges the code for tokens, persists the
/// refresh token, and emits `gdrive:connected` on success.
#[tauri::command]
pub async fn gdrive_connect(app: AppHandle) -> AppResult<()> {
    let (client_id, _) = require_credentials()?;
    let client = http_client()?;

    // Bind first so we know what redirect_uri to send to Google. Port 0
    // lets the kernel pick; Google's "Desktop app" client treats any
    // 127.0.0.1 port as valid.
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|e| AppError::msg(format!("loopback bind failed: {e}")))?;
    let port = listener
        .local_addr()
        .map_err(|e| AppError::msg(format!("loopback local_addr failed: {e}")))?
        .port();
    let redirect_uri = format!("http://127.0.0.1:{port}/callback");

    let verifier = random_url_safe_string(64);
    let challenge = pkce_challenge(&verifier);
    let state = random_url_safe_string(32);

    let auth_url = format!(
        "{AUTH_URL}?response_type=code&client_id={cid}&redirect_uri={ruri}\
         &scope={scope}&access_type=offline&prompt=consent\
         &code_challenge={challenge}&code_challenge_method=S256&state={state}",
        cid = urlencoding::encode(&client_id),
        ruri = urlencoding::encode(&redirect_uri),
        scope = urlencoding::encode(SCOPES),
        challenge = urlencoding::encode(&challenge),
        state = urlencoding::encode(&state),
    );

    if let Err(e) = tauri_plugin_opener::open_url(&auth_url, None::<&str>) {
        log::warn!("gdrive: failed to open browser: {e}");
    }

    let code = await_oauth_callback(listener, &state).await?;
    let tokens = exchange_code_for_tokens(&client, &code, &verifier, &redirect_uri).await?;

    let refresh = tokens.refresh_token.clone().ok_or_else(|| {
        AppError::from(
            "Google did not return a refresh token. Try disconnecting and reconnecting; \
             the consent prompt must request offline access.",
        )
    })?;
    store_refresh_token(&refresh)?;
    *ACCESS_TOKEN.lock() = Some(CachedAccessToken {
        token: tokens.access_token.clone(),
        expires_at: Instant::now() + Duration::from_secs(tokens.expires_in),
    });

    let email = fetch_email(&client, &tokens.access_token).await;
    let _ = app.emit(
        "gdrive:connected",
        GdriveStatus {
            connected: true,
            email,
        },
    );
    Ok(())
}

#[tauri::command]
pub async fn gdrive_status() -> AppResult<GdriveStatus> {
    if read_refresh_token().is_none() {
        return Ok(GdriveStatus {
            connected: false,
            email: None,
        });
    }
    let client = http_client()?;
    let access = match ensure_access_token(&client).await {
        Ok(t) => t,
        Err(_) => {
            // Refresh failed — token was revoked or credentials missing.
            // Clear local state so the UI stops claiming we're connected.
            let _ = delete_refresh_token();
            *ACCESS_TOKEN.lock() = None;
            return Ok(GdriveStatus {
                connected: false,
                email: None,
            });
        }
    };
    let email = fetch_email(&client, &access).await;
    Ok(GdriveStatus {
        connected: true,
        email,
    })
}

#[tauri::command]
pub async fn gdrive_disconnect() -> AppResult<()> {
    let token = read_refresh_token();
    if let Some(token) = token {
        if let Ok(client) = http_client() {
            // Best-effort revoke. If this fails (offline, network glitch)
            // we still purge the local entry — a stale UI is worse than a
            // server-side session that can be revoked from the Google
            // account dashboard later.
            let _ = client
                .post(REVOKE_URL)
                .query(&[("token", token.as_str())])
                .send()
                .await;
        }
    }
    *ACCESS_TOKEN.lock() = None;
    delete_refresh_token().map_err(Into::into)
}

// ──────────────────────────────────────────────────────────────────────────
// Drive upload
// ──────────────────────────────────────────────────────────────────────────

async fn find_or_create_recast_folder(
    client: &Client,
    access_token: &str,
) -> Result<String, String> {
    // Search `My Drive` (default corpus) for an existing top-level folder
    // we own called `Recast`. The query escapes the literal name since
    // Drive's q syntax treats single quotes as string delimiters.
    let q = format!(
        "name='{}' and mimeType='application/vnd.google-apps.folder' and trashed=false",
        RECAST_FOLDER_NAME
    );
    let resp = client
        .get(format!("{DRIVE_API}/files"))
        .header(header::AUTHORIZATION, format!("Bearer {access_token}"))
        .query(&[
            ("q", q.as_str()),
            ("fields", "files(id)"),
            ("pageSize", "1"),
        ])
        .send()
        .await
        .map_err(|e| format!("folder lookup request failed: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("folder lookup returned {status}: {body}"));
    }

    #[derive(Deserialize)]
    struct FilesList {
        files: Vec<FileId>,
    }
    #[derive(Deserialize)]
    struct FileId {
        id: String,
    }

    let body: FilesList = resp
        .json()
        .await
        .map_err(|e| format!("folder lookup parse failed: {e}"))?;
    if let Some(existing) = body.files.into_iter().next() {
        return Ok(existing.id);
    }

    // Create. Plain JSON metadata POST — no media body.
    let resp = client
        .post(format!("{DRIVE_API}/files"))
        .header(header::AUTHORIZATION, format!("Bearer {access_token}"))
        .json(&serde_json::json!({
            "name": RECAST_FOLDER_NAME,
            "mimeType": "application/vnd.google-apps.folder",
        }))
        .send()
        .await
        .map_err(|e| format!("folder create request failed: {e}"))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("folder create returned {status}: {body}"));
    }
    let created: FileId = resp
        .json()
        .await
        .map_err(|e| format!("folder create parse failed: {e}"))?;
    Ok(created.id)
}

fn guess_mime_type(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_ascii_lowercase())
        .as_deref()
    {
        Some("mp4") => "video/mp4",
        Some("webm") => "video/webm",
        Some("gif") => "image/gif",
        Some("mov") => "video/quicktime",
        Some("mkv") => "video/x-matroska",
        _ => "application/octet-stream",
    }
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GdriveUploadResult {
    pub file_id: String,
    pub name: String,
    pub web_view_link: Option<String>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct UploadCompleteEvent<'a> {
    upload_id: &'a str,
    /// Source path on this machine. The frontend uses this to index the
    /// upload-history map so the exports list can flip its menu state
    /// based on whether this exact file was previously uploaded.
    source_path: &'a str,
    #[serde(flatten)]
    result: &'a GdriveUploadResult,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct UploadProgressEvent<'a> {
    upload_id: &'a str,
    bytes_sent: u64,
    total_bytes: u64,
}

/// Resumable upload of `path` into the `/Recast/` folder. Streams the file
/// in chunks, emits `gdrive:progress` events between chunks, and honors a
/// cancel flag the frontend can flip via `gdrive_cancel_upload`.
#[tauri::command]
pub async fn gdrive_upload(
    app: AppHandle,
    path: String,
    upload_id: String,
) -> AppResult<GdriveUploadResult> {
    let cancel = upload_cancel_flag(&upload_id);
    let result = gdrive_upload_inner(&app, &path, &upload_id, cancel.clone()).await;
    drop_upload_cancel_flag(&upload_id);
    match &result {
        Ok(payload) => {
            // Persist a record of this upload keyed by the local file path
            // so the exports list can switch its action from "Upload" to
            // "Copy link / Re-upload" without re-querying Google. The
            // write is best-effort — if the disk is full or the path is
            // unwriteable, the in-app upload still succeeded.
            let uploaded_at = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            record_upload(
                &app,
                &path,
                UploadRecord {
                    file_id: payload.file_id.clone(),
                    name: payload.name.clone(),
                    web_view_link: payload.web_view_link.clone(),
                    uploaded_at,
                },
            );

            let _ = app.emit(
                "gdrive:upload-complete",
                UploadCompleteEvent {
                    upload_id: upload_id.as_str(),
                    source_path: path.as_str(),
                    result: payload,
                },
            );
        }
        Err(err) => {
            let cancelled = cancel.load(Ordering::Relaxed);
            let _ = app.emit(
                "gdrive:upload-error",
                serde_json::json!({
                    "uploadId": upload_id,
                    "message": err,
                    "cancelled": cancelled,
                }),
            );
        }
    }
    result.map_err(Into::into)
}

async fn gdrive_upload_inner(
    app: &AppHandle,
    path: &str,
    upload_id: &str,
    cancel: Arc<AtomicBool>,
) -> Result<GdriveUploadResult, String> {
    let file_path = PathBuf::from(path);
    if !file_path.is_file() {
        return Err(format!("file not found: {path}"));
    }
    let file_name = file_path
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| "invalid file name".to_string())?
        .to_string();
    let total_bytes = std::fs::metadata(&file_path)
        .map_err(|e| format!("stat failed: {e}"))?
        .len();
    let mime = guess_mime_type(&file_path);

    let client = http_client()?;
    let access = ensure_access_token(&client).await?;
    let folder_id = find_or_create_recast_folder(&client, &access).await?;

    // Initiate resumable upload — POST metadata, get Location header.
    let metadata = serde_json::json!({
        "name": file_name,
        "parents": [folder_id],
        "mimeType": mime,
    });
    let init = client
        .post(format!(
            "{DRIVE_UPLOAD}?uploadType=resumable&fields=id,name,webViewLink"
        ))
        .header(header::AUTHORIZATION, format!("Bearer {access}"))
        .header("X-Upload-Content-Type", mime)
        .header("X-Upload-Content-Length", total_bytes.to_string())
        .json(&metadata)
        .send()
        .await
        .map_err(|e| format!("upload init failed: {e}"))?;
    if !init.status().is_success() {
        let status = init.status();
        let body = init.text().await.unwrap_or_default();
        return Err(format!("upload init returned {status}: {body}"));
    }
    let session_url = init
        .headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| "upload init missing Location header".to_string())?
        .to_string();

    // Stream chunks. For files smaller than one chunk, this is a single
    // PUT covering the whole range. For larger files, repeated PUTs with
    // 308 Resume Incomplete in between.
    let file = tokio::fs::File::open(&file_path)
        .await
        .map_err(|e| format!("open failed: {e}"))?;
    let mut reader = tokio::io::BufReader::new(file);
    let mut bytes_sent: u64 = 0;
    let mut buf = vec![0u8; UPLOAD_CHUNK_SIZE];

    loop {
        if cancel.load(Ordering::Relaxed) {
            // Best-effort DELETE on the session URL to free Drive's
            // partial upload bookkeeping. The error path doesn't propagate
            // this; we still want to bail out with a cancel message.
            let _ = client.delete(&session_url).send().await;
            return Err("upload cancelled".into());
        }
        use tokio::io::AsyncReadExt;
        let n = reader
            .read(&mut buf)
            .await
            .map_err(|e| format!("read failed: {e}"))?;
        if n == 0 {
            return Err("upload ended early — file shorter than declared size".into());
        }
        let chunk = &buf[..n];
        let range_end = bytes_sent + n as u64 - 1;
        let content_range = format!("bytes {bytes_sent}-{range_end}/{total_bytes}");

        let resp = client
            .put(&session_url)
            .header(header::CONTENT_LENGTH, n.to_string())
            .header(header::CONTENT_RANGE, content_range)
            .body(chunk.to_vec())
            .send()
            .await
            .map_err(|e| format!("chunk PUT failed: {e}"))?;

        let status = resp.status();
        if status == reqwest::StatusCode::OK || status == reqwest::StatusCode::CREATED {
            // Final chunk — Drive returns the file resource.
            let body: serde_json::Value = resp
                .json()
                .await
                .map_err(|e| format!("final response parse failed: {e}"))?;
            bytes_sent += n as u64;
            let _ = app.emit(
                "gdrive:progress",
                UploadProgressEvent {
                    upload_id,
                    bytes_sent,
                    total_bytes,
                },
            );
            let file_id = body
                .get("id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "final response missing file id".to_string())?
                .to_string();
            let name = body
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or(&file_name)
                .to_string();
            let web_view_link = body
                .get("webViewLink")
                .and_then(|v| v.as_str())
                .map(str::to_string);
            return Ok(GdriveUploadResult {
                file_id,
                name,
                web_view_link,
            });
        }
        if status.as_u16() == 308 {
            // Resume Incomplete. Advance our pointer past what Drive
            // confirms received; on a fresh start that's exactly the
            // chunk we just sent, so this is a sanity-check no-op.
            bytes_sent += n as u64;
            let _ = app.emit(
                "gdrive:progress",
                UploadProgressEvent {
                    upload_id,
                    bytes_sent,
                    total_bytes,
                },
            );
            continue;
        }
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("chunk PUT returned {status}: {body}"));
    }
}

#[tauri::command]
pub fn gdrive_cancel_upload(upload_id: String) {
    signal_upload_cancel(&upload_id);
}

/// Returns the local upload history — a map of `localPath -> UploadRecord`
/// for every export the user has uploaded from this machine. Cheap (single
/// JSON file read); the frontend caches the result in a Svelte store and
/// merges in new entries as `gdrive:upload-complete` events fire.
#[tauri::command]
pub fn gdrive_list_uploads(app: AppHandle) -> HashMap<String, UploadRecord> {
    read_manifest(&app)
}

/// Drop a single entry from the upload history. Used when a user moves an
/// export to trash — keeping the stale "uploaded" badge on a non-existent
/// file would be confusing. Best-effort: no error on missing keys.
#[tauri::command]
pub fn gdrive_forget_upload(app: AppHandle, local_path: String) {
    let mut manifest = read_manifest(&app);
    if manifest.remove(&local_path).is_some() {
        write_manifest(&app, &manifest);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_escape_replaces_all_markup_sensitive_chars() {
        assert_eq!(
            html_escape(r#"<a href="x&y">'q'</a>"#),
            "&lt;a href=&quot;x&amp;y&quot;&gt;&#39;q&#39;&lt;/a&gt;",
        );
    }

    #[test]
    fn html_escape_leaves_plain_text_untouched() {
        assert_eq!(html_escape("access_denied"), "access_denied");
    }

    #[test]
    fn guess_mime_type_maps_known_video_extensions() {
        assert_eq!(guess_mime_type(Path::new("clip.mp4")), "video/mp4");
        assert_eq!(guess_mime_type(Path::new("clip.webm")), "video/webm");
        assert_eq!(guess_mime_type(Path::new("clip.gif")), "image/gif");
        assert_eq!(guess_mime_type(Path::new("clip.mov")), "video/quicktime");
        assert_eq!(guess_mime_type(Path::new("clip.mkv")), "video/x-matroska");
    }

    #[test]
    fn guess_mime_type_is_case_insensitive() {
        assert_eq!(guess_mime_type(Path::new("CLIP.MP4")), "video/mp4");
    }

    #[test]
    fn guess_mime_type_falls_back_to_octet_stream() {
        assert_eq!(
            guess_mime_type(Path::new("data.bin")),
            "application/octet-stream",
        );
        assert_eq!(
            guess_mime_type(Path::new("noext")),
            "application/octet-stream",
        );
    }

    #[test]
    fn pkce_challenge_matches_rfc_7636_test_vector() {
        // RFC 7636 Appendix B.
        assert_eq!(
            pkce_challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM",
        );
    }

    #[test]
    fn random_url_safe_string_has_requested_length_and_charset() {
        const ALPHABET: &[u8] =
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~";
        let s = random_url_safe_string(64);
        assert_eq!(s.len(), 64);
        assert!(s.bytes().all(|b| ALPHABET.contains(&b)));
    }
}
