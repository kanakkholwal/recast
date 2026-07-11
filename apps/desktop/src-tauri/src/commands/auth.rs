//! Cloud sign-in via OAuth 2.0 Device Authorization Grant (RFC 8628).
//!
//! Flow from the desktop's point of view:
//!
//!   1. `auth_start` posts to `/api/auth/device/code`, gets a `device_code`,
//!      `user_code`, `verification_uri_complete`, and polling `interval`.
//!      Opens the user's default browser to `verification_uri_complete`
//!      (the verification page with the user code already in the URL).
//!      Returns the `user_code` immediately so the UI can surface it as a
//!      fallback if the browser launch silently failed.
//!   2. A background poller hits `/api/auth/device/token` every `interval`
//!      seconds until it sees `access_token` (approved), `access_denied`,
//!      or the token expires. On success, Better Auth's plugin has already
//!      created a real session row in the same request — so the row's
//!      `ipAddress` and `userAgent` are this desktop's, not the browser's.
//!      We just persist the returned token to the OS keyring.
//!   3. The frontend listens for `auth:signed-in`, `auth:denied`,
//!      `auth:expired`, and `auth:error` events to update its state.
//!
//! Cancellation: `auth_start` stores the spawned poller's `JoinHandle` in
//! `AppState.auth_poller`. `auth_cancel` aborts it, preventing the
//! "user clicks Cancel, then approves in the browser tab" race where the UI
//! would lurch from signed-out to signed-in without further user action.
//! Only one poller may be live at a time; calling `auth_start` while one is
//! already running returns an error.
//!
//! Token storage uses `keyring` — DPAPI on Windows, Keychain on macOS,
//! SecretService on Linux. Service name is the bundle identifier.

use parking_lot::RwLock;
use std::time::{Duration, Instant};

use keyring::Entry;
use reqwest::header;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

use super::error::{AppError, AppResult};
use super::system::save_config;
use super::types::AppState;

const KEYRING_SERVICE: &str = "com.nexonauts.recast";
const KEYRING_ENTRY: &str = "cloud-session-token";
const CLIENT_ID: &str = "recast-desktop";
const DEFAULT_CLOUD_API_URL: &str = "https://recast.li";

/// Self-hosting override for the cloud API base URL, cached in-process so the
/// no-arg `cloud_api_url()` resolver (called from many sites that don't carry
/// `AppState`) can read it without threading state everywhere. Mirrors
/// `AppConfig.cloud_api_url`: seeded from disk on startup via
/// `init_cloud_api_override`, updated by the `set_cloud_api_url` command.
static CLOUD_API_OVERRIDE: RwLock<Option<String>> = RwLock::new(None);

/// Validate + normalize a user-supplied self-host URL. Accepts only an
/// absolute `http`/`https` URL with a non-empty host; returns the
/// trailing-slash-stripped form. `None` for anything malformed — the caller
/// treats that as "no usable override" (setter rejects it; resolver ignores
/// it and falls back to the default endpoint).
pub(crate) fn normalize_api_url(raw: &str) -> Option<String> {
    let trimmed = raw.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return None;
    }
    let parsed = reqwest::Url::parse(trimmed).ok()?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return None;
    }
    if parsed.host_str().map(str::is_empty).unwrap_or(true) {
        return None;
    }
    Some(trimmed.to_string())
}

/// Seed the in-process override from persisted config at startup. Called once
/// during app setup after the config is loaded.
pub(crate) fn init_cloud_api_override(value: Option<String>) {
    *CLOUD_API_OVERRIDE.write() = value;
}

/// Resolves the cloud API base URL. Resolution order:
///   1. The self-host override the user set in Settings → Cloud, if present
///      and still valid. This is deliberate user action, so unlike an
///      injected env var it's honored in release builds too — it's how
///      self-hosters point the desktop at their own server.
///   2. In debug builds only, the `CLOUD_API_URL` env var (dev convenience —
///      lets `pnpm tauri dev` target a local SvelteKit server). Release
///      builds skip this so a stray/injected env can't silently redirect
///      auth traffic to an attacker host.
///   3. The bundled default endpoint.
/// Trailing slashes are stripped at every layer.
pub(crate) fn cloud_api_url() -> String {
    if let Some(raw) = CLOUD_API_OVERRIDE.read().clone() {
        if let Some(valid) = normalize_api_url(&raw) {
            return valid;
        }
    }
    #[cfg(debug_assertions)]
    let raw = std::env::var("CLOUD_API_URL").unwrap_or_else(|_| DEFAULT_CLOUD_API_URL.to_string());
    #[cfg(not(debug_assertions))]
    let raw = DEFAULT_CLOUD_API_URL.to_string();
    raw.trim_end_matches('/').to_string()
}

pub(crate) fn user_agent() -> String {
    // Better Auth captures this header into `session.userAgent`. Putting the
    // OS + hostname here gives the user a recognizable label in the future
    // Settings → Devices list ("Kanak-Desktop on Windows") without needing
    // a custom field on the deviceCode body schema.
    //
    // PRIVACY NOTE: `hostname::get()` returns the OS hostname which is often
    // a real first name or device name (e.g. "Kanak-MacBook-Pro"). It lands
    // in the `session.user_agent` Postgres column. This is by design — it's
    // only ever shown back to the same user in their own Devices list — and
    // is covered by the privacy policy's "Web server access logs (... user
    // agent ...)" disclosure. If we ever expose `session.user_agent` to
    // OTHER users (e.g. team-admin views), redact this field there.
    let host = hostname::get()
        .ok()
        .and_then(|h| h.into_string().ok())
        .unwrap_or_else(|| "Unknown".to_string());
    format!(
        "Recast/{} ({}; {})",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        host
    )
}

fn http_client() -> reqwest::Result<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent(user_agent())
        .timeout(Duration::from_secs(15))
        .build()
}

fn keyring_entry() -> keyring::Result<Entry> {
    Entry::new(KEYRING_SERVICE, KEYRING_ENTRY)
}

fn store_session_token(token: &str) -> Result<(), String> {
    keyring_entry()
        .and_then(|e| e.set_password(token))
        .map_err(|e| format!("keyring write failed: {e}"))
}

fn read_session_token() -> Option<String> {
    keyring_entry().ok().and_then(|e| e.get_password().ok())
}

/// Best-effort local token removal. Returns `Ok(())` even if the keyring
/// entry can't be opened — the goal of this function is "make sure no token
/// is usable from this process," not "successfully interact with the OS
/// secrets store." If we can't open the entry we couldn't have read a token
/// either, so the user is effectively signed out from the desktop's POV
/// and surfacing an error to the UI ("Couldn't sign out") would be
/// misleading.
fn delete_session_token() -> Result<(), String> {
    let Ok(entry) = keyring_entry() else {
        return Ok(());
    };
    match entry.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(format!("keyring delete failed: {e}")),
    }
}

#[derive(Deserialize)]
struct DeviceCodeResp {
    device_code: String,
    user_code: String,
    verification_uri_complete: Option<String>,
    verification_uri: String,
    interval: u64,
    expires_in: u64,
}

#[derive(Deserialize)]
struct DeviceTokenSuccess {
    access_token: String,
}

#[derive(Deserialize)]
struct DeviceTokenError {
    error: String,
}

#[derive(Serialize)]
pub struct AuthStartResult {
    user_code: String,
    verification_uri: String,
    expires_in: u64,
}

#[derive(Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct AuthPlan {
    /// "free" | "pro" — plan id from the subscription row.
    id: String,
    /// Display name ("Free", "Pro").
    name: String,
    /// Subscription status — "active" | "canceled" | "past_due" | "trialing" | …
    status: String,
    /// ISO-8601 string or `null` for free plans.
    current_period_end: Option<String>,
    cancel_at_period_end: bool,
}

#[derive(Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct AuthUsage {
    recordings: u64,
    storage_bytes: u64,
    active_shares: u64,
    /// `None` means the plan has no cap.
    shares_limit: Option<u64>,
}

/// One workspace (Better Auth `organization`) the signed-in user belongs to.
/// Mirrors the `workspaces[]` entries `/api/desktop/profile` returns so the
/// desktop can offer a target picker at share time. Uploads carry an explicit
/// `workspaceId` and the server re-validates membership on
/// `/api/uploads/init`, so a stale id fails closed rather than silently
/// uploading into a team the user was removed from.
#[derive(Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct Workspace {
    id: String,
    name: String,
    /// "owner" | "admin" | "member" — surfaced as a badge in the picker.
    role: String,
    /// "free" | "pro" | "enterprise" — the workspace's (org's) plan, shown as
    /// a badge so the user can tell which team a share lands on.
    plan: String,
    /// Live (non-deleted) recast count in this workspace.
    recasts_count: u64,
}

#[derive(Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct AuthStatus {
    signed_in: bool,
    email: Option<String>,
    name: Option<String>,
    /// Avatar URL — Better Auth's `user.image`, populated by OAuth providers
    /// or admin-set. `None` for users who signed up via email/password and
    /// never uploaded an avatar; UI falls back to initials.
    image: Option<String>,
    /// ISO-8601 timestamp of the `user.createdAt`. Used for "Member since
    /// May 2026" line in the Settings → Cloud card.
    member_since: Option<String>,
    /// Plan + usage are fetched from /api/desktop/profile and ONLY present
    /// when that call succeeds. The minimal /api/auth/get-session fallback
    /// (used on transport errors mid-recompute) leaves these `None`, so the
    /// UI should be defensive.
    plan: Option<AuthPlan>,
    usage: Option<AuthUsage>,
    /// Workspaces the user belongs to (active-org-first ordering from the
    /// server). Empty on the get-session fallback path — the picker simply
    /// won't offer a choice until a profile fetch succeeds.
    workspaces: Vec<Workspace>,
    /// The server's preferred upload target: the session's active org if the
    /// user is still a member, else their first workspace. `None` when the
    /// user belongs to no workspace. The desktop's local preference overrides
    /// this when set and still valid.
    default_workspace_id: Option<String>,
}

/// Parses Better Auth's `/api/auth/get-session` response body into our
/// `AuthStatus` (minimal — email/name/image only). Used as a fallback when
/// the desktop profile endpoint isn't reachable. Returns `signed_in: true`
/// with empty identity fields when the body is shaped unexpectedly — we
/// already know the token was accepted (the caller checked status), so the
/// user IS signed in even if we can't surface their display name.
fn parse_session_body(body: &serde_json::Value) -> AuthStatus {
    let user = body.get("user");
    AuthStatus {
        signed_in: true,
        email: user
            .and_then(|u| u.get("email"))
            .and_then(|v| v.as_str())
            .map(str::to_string),
        name: user
            .and_then(|u| u.get("name"))
            .and_then(|v| v.as_str())
            .map(str::to_string),
        image: user
            .and_then(|u| u.get("image"))
            .and_then(|v| v.as_str())
            .map(str::to_string),
        member_since: None,
        plan: None,
        usage: None,
        workspaces: Vec::new(),
        default_workspace_id: None,
    }
}

/// Parses the response from /api/desktop/profile into a fully-populated
/// `AuthStatus` (identity + plan + usage). The endpoint authenticates via
/// the same bearer token and returns everything in one round-trip, so this
/// is the preferred path when we want to render the rich Settings card.
fn parse_profile_body(body: &serde_json::Value) -> AuthStatus {
    let user = body.get("user");
    let plan = body.get("plan");
    let usage = body.get("usage");

    AuthStatus {
        signed_in: true,
        email: user
            .and_then(|u| u.get("email"))
            .and_then(|v| v.as_str())
            .map(str::to_string),
        name: user
            .and_then(|u| u.get("name"))
            .and_then(|v| v.as_str())
            .map(str::to_string),
        image: user
            .and_then(|u| u.get("image"))
            .and_then(|v| v.as_str())
            .map(str::to_string),
        member_since: user
            .and_then(|u| u.get("memberSince"))
            .and_then(|v| v.as_str())
            .map(str::to_string),
        plan: plan.map(|p| AuthPlan {
            id: p
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("free")
                .to_string(),
            name: p
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("Free")
                .to_string(),
            status: p
                .get("status")
                .and_then(|v| v.as_str())
                .unwrap_or("active")
                .to_string(),
            current_period_end: p
                .get("currentPeriodEnd")
                .and_then(|v| v.as_str())
                .map(str::to_string),
            cancel_at_period_end: p
                .get("cancelAtPeriodEnd")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
        }),
        usage: usage.map(|u| AuthUsage {
            recordings: u.get("recordings").and_then(|v| v.as_u64()).unwrap_or(0),
            storage_bytes: u.get("storageBytes").and_then(|v| v.as_u64()).unwrap_or(0),
            active_shares: u.get("activeShares").and_then(|v| v.as_u64()).unwrap_or(0),
            shares_limit: u.get("sharesLimit").and_then(|v| v.as_u64()),
        }),
        workspaces: body
            .get("workspaces")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|w| {
                        let id = w.get("id").and_then(|v| v.as_str())?.to_string();
                        Some(Workspace {
                            id,
                            name: w
                                .get("name")
                                .and_then(|v| v.as_str())
                                .unwrap_or("Workspace")
                                .to_string(),
                            role: w
                                .get("role")
                                .and_then(|v| v.as_str())
                                .unwrap_or("member")
                                .to_string(),
                            plan: w
                                .get("plan")
                                .and_then(|v| v.as_str())
                                .unwrap_or("free")
                                .to_string(),
                            recasts_count: w
                                .get("recastsCount")
                                .and_then(|v| v.as_u64())
                                .unwrap_or(0),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default(),
        default_workspace_id: body
            .get("defaultWorkspaceId")
            .and_then(|v| v.as_str())
            .map(str::to_string),
    }
}

/// Kicks off a device-authorization sign-in. Returns immediately with the
/// user code (for UI fallback) and spawns a background poller that emits
/// `auth:signed-in` / `auth:denied` / `auth:expired` / `auth:error` events
/// when the flow terminates.
///
/// Returns an error if a sign-in flow is already in progress, or if the
/// user is already signed in (the caller should `auth_sign_out` first).
#[tauri::command]
pub async fn auth_start(app: AppHandle, state: State<'_, AppState>) -> AppResult<AuthStartResult> {
    // Abort any previous poller first. There's a sub-instant race in the
    // poller between "/device/token returned approved → token written to
    // keyring" and "abort fires at the next await" — `store_session_token`
    // is synchronous so an abort during that window still leaves a token
    // behind. Killing the poller first narrows the window.
    if let Some(prev) = state.auth_poller.lock().take() {
        prev.abort();
    }

    // Clear any stale token (UI desync, lost-race-with-cancel, leftover
    // from a previous install, etc.) before starting a new flow. The user
    // clicking "Sign in" is unambiguous intent; if the keyring still holds
    // an old token the right answer is to throw it away — not to refuse
    // the sign-in and leave the user stuck. Server-side revoke is
    // best-effort: offline or 404 doesn't block the new flow.
    if let Some(stale) = read_session_token() {
        log::info!("auth_start: clearing stale token before fresh sign-in");
        if let Ok(client) = http_client() {
            let base = cloud_api_url();
            let _ = client
                .post(format!("{base}/api/auth/sign-out"))
                .header(header::AUTHORIZATION, format!("Bearer {stale}"))
                .send()
                .await;
        }
        let _ = delete_session_token();
    }

    let client =
        http_client().map_err(|e| AppError::msg(format!("http client init failed: {e}")))?;
    let base = cloud_api_url();

    let resp = client
        .post(format!("{base}/api/auth/device/code"))
        .json(&serde_json::json!({
            "client_id": CLIENT_ID,
            "scope": "sync",
        }))
        .send()
        .await
        .map_err(|e| AppError::msg(format!("device/code request failed: {e}")))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(AppError::msg(format!(
            "device/code returned {status}: {body}"
        )));
    }

    let code: DeviceCodeResp = resp
        .json()
        .await
        .map_err(|e| AppError::msg(format!("device/code response parse failed: {e}")))?;

    let open_url = code
        .verification_uri_complete
        .clone()
        .unwrap_or_else(|| code.verification_uri.clone());
    if let Err(e) = tauri_plugin_opener::open_url(&open_url, None::<&str>) {
        // Non-fatal — the UI still surfaces the user code so the user can
        // navigate manually. Just log.
        log::warn!("auth: failed to open browser to {open_url}: {e}");
    }

    let result = AuthStartResult {
        user_code: code.user_code.clone(),
        verification_uri: code.verification_uri.clone(),
        expires_in: code.expires_in,
    };

    let poll_app = app.clone();
    let device_code = code.device_code.clone();
    let interval = code.interval.max(1);
    let expires_in = code.expires_in;
    let client_for_poll = client.clone();
    let base_for_poll = base.clone();
    let handle = tauri::async_runtime::spawn(async move {
        if let Err(e) = poll_for_token(
            &poll_app,
            &client_for_poll,
            &base_for_poll,
            &device_code,
            interval,
            expires_in,
        )
        .await
        {
            let _ = poll_app.emit("auth:error", e);
        }
    });

    *state.auth_poller.lock() = Some(handle);

    Ok(result)
}

/// Aborts the in-flight device-authorization poller (if any). Used when
/// the user clicks Cancel in the Settings → Cloud panel — without this,
/// the poller keeps running until the device code expires, and an
/// approval in the browser tab would silently sign the desktop in even
/// though the user has moved on from the flow.
///
/// Returns `Ok(())` whether or not a poller was running — Cancel is
/// idempotent.
#[tauri::command]
pub fn auth_cancel(state: State<'_, AppState>) -> AppResult<()> {
    if let Some(handle) = state.auth_poller.lock().take() {
        handle.abort();
    }
    Ok(())
}

async fn poll_for_token(
    app: &AppHandle,
    client: &reqwest::Client,
    base: &str,
    device_code: &str,
    initial_interval: u64,
    expires_in: u64,
) -> Result<(), String> {
    let deadline = Instant::now() + Duration::from_secs(expires_in);
    let mut interval = initial_interval;

    loop {
        tokio::time::sleep(Duration::from_secs(interval)).await;
        if Instant::now() > deadline {
            let _ = app.emit("auth:expired", ());
            return Ok(());
        }

        let resp = client
            .post(format!("{base}/api/auth/device/token"))
            .json(&serde_json::json!({
                "grant_type": "urn:ietf:params:oauth:grant-type:device_code",
                "device_code": device_code,
                "client_id": CLIENT_ID,
            }))
            .send()
            .await
            .map_err(|e| format!("device/token request failed: {e}"))?;

        let status = resp.status();
        if status.is_success() {
            let body: DeviceTokenSuccess = resp
                .json()
                .await
                .map_err(|e| format!("device/token success parse failed: {e}"))?;
            store_session_token(&body.access_token)?;
            // PRIVACY NOTE: this event payload carries the signed-in user's
            // email + name. Tauri events fan out to every listener in the
            // WebView, so any future bundled third-party JS would be able to
            // subscribe via `listen('auth:signed-in', ...)`. We currently
            // ship only first-party code, but if that ever changes, switch
            // this to an empty payload and have the frontend re-call
            // `auth_status` to fetch the identity over IPC instead.
            let status = fetch_status(client, base, &body.access_token).await;
            let _ = app.emit("auth:signed-in", status);
            return Ok(());
        }

        // Non-2xx — try to parse the OAuth error envelope. RFC 8628 reserves
        // a specific set of error codes for the polling path.
        let err: DeviceTokenError = resp
            .json()
            .await
            .map_err(|e| format!("device/token error parse failed (status {status}): {e}"))?;
        match err.error.as_str() {
            "authorization_pending" => continue,
            "slow_down" => {
                interval = interval.saturating_add(5);
                continue;
            }
            "access_denied" => {
                let _ = app.emit("auth:denied", ());
                return Ok(());
            }
            "expired_token" => {
                let _ = app.emit("auth:expired", ());
                return Ok(());
            }
            other => {
                let _ = app.emit("auth:error", format!("server error: {other}"));
                return Ok(());
            }
        }
    }
}

/// Fetches the rich profile (user + plan + usage) from
/// `/api/desktop/profile`. Falls back to the bare get-session response if
/// the desktop endpoint isn't reachable or returns non-2xx — keeps the UI
/// usable even when the new endpoint is missing (e.g. older server build).
async fn fetch_status(client: &reqwest::Client, base: &str, token: &str) -> AuthStatus {
    let profile_resp = client
        .get(format!("{base}/api/desktop/profile"))
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .send()
        .await;

    if let Ok(resp) = profile_resp {
        if resp.status().is_success() {
            if let Ok(body) = resp.json::<serde_json::Value>().await {
                return parse_profile_body(&body);
            }
        }
    }

    // Fallback: minimal get-session shape so the UI at least shows email.
    let resp = client
        .get(format!("{base}/api/auth/get-session"))
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .send()
        .await;

    let Ok(resp) = resp else {
        return AuthStatus {
            signed_in: true,
            ..Default::default()
        };
    };
    if !resp.status().is_success() {
        return AuthStatus {
            signed_in: true,
            ..Default::default()
        };
    }
    match resp.json::<serde_json::Value>().await {
        Ok(body) => parse_session_body(&body),
        Err(_) => AuthStatus {
            signed_in: true,
            ..Default::default()
        },
    }
}

/// Returns the current sign-in state. Hits the server to validate the
/// stored token — a revoked/expired token reports as signed-out and is
/// cleared from the keyring so the next `auth_start` is clean.
///
/// OFFLINE BEHAVIOR: if we have a stored token but the request fails at the
/// transport layer (offline, DNS error, server unreachable), we report
/// `signed_in: true` with no identity. Recast is offline-first ([1]) — flipping
/// to "Not signed in" on every network blip would defeat the whole point.
/// A 401/403 from the server is still treated as signed-out, since that's
/// an authoritative "your token is no longer valid."
///
/// [1]: see project memory `project_overview.md`.
#[tauri::command]
pub async fn auth_status() -> AppResult<AuthStatus> {
    let Some(token) = read_session_token() else {
        return Ok(AuthStatus {
            signed_in: false,
            ..Default::default()
        });
    };
    let client =
        http_client().map_err(|e| AppError::msg(format!("http client init failed: {e}")))?;
    let base = cloud_api_url();

    // Hit the desktop profile endpoint first — it both validates the token
    // (returns 401 if invalid) AND returns the rich plan + usage data the
    // Settings card needs. Falling through to /api/auth/get-session only if
    // /api/desktop/profile is missing on the server (older build).
    let resp = match client
        .get(format!("{base}/api/desktop/profile"))
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .send()
        .await
    {
        Ok(resp) => resp,
        // Transport failure (no network, DNS, TLS, timeout). Trust the local
        // token and assume signed-in — offline-first.
        Err(e) => {
            log::warn!("auth_status: transport failure, assuming signed-in: {e}");
            return Ok(AuthStatus {
                signed_in: true,
                ..Default::default()
            });
        }
    };

    let status = resp.status();
    if status.as_u16() == 401 || status.as_u16() == 403 {
        // Authoritative server response: token is no longer valid — purge it
        // locally so we don't stay in a "signed in" UI state forever.
        let _ = delete_session_token();
        return Ok(AuthStatus {
            signed_in: false,
            ..Default::default()
        });
    }

    if status.is_success() {
        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| AppError::msg(format!("desktop/profile parse failed: {e}")))?;
        if body.is_null() || body.get("user").is_none() {
            let _ = delete_session_token();
            return Ok(AuthStatus {
                signed_in: false,
                ..Default::default()
            });
        }
        return Ok(parse_profile_body(&body));
    }

    // Non-401 non-success (404 if endpoint missing, 5xx, etc.) — fall back to
    // get-session for at least the minimal identity.
    Ok(fetch_status(&client, &base, &token).await)
}

/// Server-side revoke + local delete. Best-effort on the server side — if
/// the request fails (offline, server down) we still drop the local token,
/// because a stale UI state is worse than a still-valid server session
/// (which can be revoked from the dashboard's Devices list later).
#[tauri::command]
pub async fn auth_sign_out() -> AppResult<()> {
    let token = read_session_token();
    if let Some(token) = token {
        if let Ok(client) = http_client() {
            let base = cloud_api_url();
            let _ = client
                .post(format!("{base}/api/auth/sign-out"))
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .send()
                .await;
        }
    }
    delete_session_token().map_err(Into::into)
}

/// Current cloud-endpoint configuration, surfaced to Settings → Cloud so a
/// self-hoster can see what's in effect and what the built-in default is.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudApiConfig {
    /// The base URL actually used for cloud requests right now.
    effective: String,
    /// The user's saved self-host override, or `None` when using the default.
    override_url: Option<String>,
    /// The bundled default endpoint — shown as the fallback / placeholder.
    default_url: String,
    /// Whether `effective` differs from the bundled default (i.e. a custom
    /// self-host endpoint is in effect).
    is_custom: bool,
}

/// Read the current cloud endpoint configuration for the Settings UI.
#[tauri::command]
pub fn get_cloud_api_config(state: State<'_, AppState>) -> CloudApiConfig {
    let override_url = state.config.read().cloud_api_url.clone();
    let effective = cloud_api_url();
    CloudApiConfig {
        is_custom: effective != DEFAULT_CLOUD_API_URL,
        effective,
        override_url,
        default_url: DEFAULT_CLOUD_API_URL.to_string(),
    }
}

/// Set (or clear) the self-host cloud API override.
///
/// Passing an empty/whitespace string or `null` clears the override and
/// reverts to the bundled default. A non-empty value must be a valid absolute
/// http(s) URL — otherwise this returns an error and nothing is persisted (the
/// "fall back to the default" behavior the resolver guarantees only kicks in
/// for already-stored values; we reject bad input up front so the user gets a
/// clear message instead of a silent revert).
///
/// Because a session token is only valid against the server that issued it,
/// changing the effective endpoint clears the locally-stored token so the UI
/// shows signed-out and the user re-authenticates against the new server
/// rather than firing a stale token at it.
#[tauri::command]
pub fn set_cloud_api_url(
    app: AppHandle,
    state: State<'_, AppState>,
    url: Option<String>,
) -> AppResult<CloudApiConfig> {
    let normalized = match url {
        Some(raw) if !raw.trim().is_empty() => Some(normalize_api_url(&raw).ok_or_else(|| {
            AppError::from("Enter a valid http(s) URL, e.g. https://recast.example.com")
        })?),
        _ => None,
    };

    let previous_effective = cloud_api_url();

    let snapshot = {
        let mut config = state.config.write();
        config.cloud_api_url = normalized.clone();
        config.clone()
    };
    save_config(&app, &snapshot);
    init_cloud_api_override(normalized.clone());

    // If the endpoint actually moved, drop the old server's token locally so
    // we don't carry a now-invalid session across servers.
    if cloud_api_url() != previous_effective {
        let _ = delete_session_token();
    }

    let override_url = normalized;
    let effective = cloud_api_url();
    Ok(CloudApiConfig {
        is_custom: effective != DEFAULT_CLOUD_API_URL,
        effective,
        override_url,
        default_url: DEFAULT_CLOUD_API_URL.to_string(),
    })
}

/// Returns the stored bearer token for downstream cloud-API calls (sync,
/// upload, share). Intentionally not exposed via Tauri command — the
/// frontend should never see the raw token. Sync features should call
/// dedicated Rust commands that fetch + use this internally.
#[allow(dead_code)]
pub fn current_session_token() -> Option<String> {
    read_session_token()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_accepts_http_and_https_and_strips_trailing_slash() {
        assert_eq!(
            normalize_api_url("https://recast.example.com/"),
            Some("https://recast.example.com".to_string())
        );
        assert_eq!(
            normalize_api_url("  http://localhost:3000  "),
            Some("http://localhost:3000".to_string())
        );
    }

    #[test]
    fn normalize_rejects_empty_bad_scheme_and_hostless() {
        assert_eq!(normalize_api_url(""), None);
        assert_eq!(normalize_api_url("   "), None);
        assert_eq!(normalize_api_url("ftp://example.com"), None);
        assert_eq!(normalize_api_url("not a url"), None);
    }

    #[test]
    fn session_body_extracts_identity_and_marks_signed_in() {
        let body = serde_json::json!({
            "user": { "email": "a@b.com", "name": "Ada", "image": "http://img/x.png" }
        });
        let status = parse_session_body(&body);
        assert!(status.signed_in);
        assert_eq!(status.email.as_deref(), Some("a@b.com"));
        assert_eq!(status.name.as_deref(), Some("Ada"));
        assert_eq!(status.image.as_deref(), Some("http://img/x.png"));
        assert!(status.plan.is_none());
        assert!(status.workspaces.is_empty());
    }

    #[test]
    fn session_body_stays_signed_in_when_shape_unexpected() {
        let status = parse_session_body(&serde_json::json!({}));
        assert!(status.signed_in);
        assert!(status.email.is_none());
    }

    #[test]
    fn profile_body_populates_plan_usage_and_workspaces() {
        let body = serde_json::json!({
            "user": { "email": "a@b.com", "name": "Ada" },
            "plan": { "id": "pro", "name": "Pro", "status": "active", "cancelAtPeriodEnd": true },
            "usage": { "recordings": 3, "storageBytes": 1024, "activeShares": 1, "sharesLimit": 10 },
            "workspaces": [ { "id": "w1", "name": "Team", "role": "owner", "plan": "pro", "recastsCount": 5 } ],
            "defaultWorkspaceId": "w1"
        });
        let status = parse_profile_body(&body);
        let plan = status.plan.expect("plan present");
        assert_eq!(plan.id, "pro");
        assert!(plan.cancel_at_period_end);
        let usage = status.usage.expect("usage present");
        assert_eq!(usage.recordings, 3);
        assert_eq!(usage.shares_limit, Some(10));
        assert_eq!(status.workspaces.len(), 1);
        assert_eq!(status.workspaces[0].id, "w1");
        assert_eq!(status.default_workspace_id.as_deref(), Some("w1"));
    }

    #[test]
    fn profile_body_defaults_plan_fields_when_absent() {
        let body = serde_json::json!({
            "user": { "email": "a@b.com" },
            "plan": {},
            "usage": {}
        });
        let status = parse_profile_body(&body);
        let plan = status.plan.expect("plan present");
        assert_eq!(plan.id, "free");
        assert_eq!(plan.name, "Free");
        assert_eq!(plan.status, "active");
        assert!(!plan.cancel_at_period_end);
        let usage = status.usage.expect("usage present");
        assert_eq!(usage.recordings, 0);
        assert_eq!(usage.shares_limit, None);
    }
}
