//! Native crash + error reporting to PostHog.
//!
//! Covers the parts of the app that never touch JS — Rust panics and command
//! errors — so a crash that kills the process (or happens before a webview is
//! live) still produces a report. Mirrors the JS analytics abstraction's rules:
//!
//!   * Gated on the `telemetry_errors` consent flag (default opt-in / ON),
//!     read from `AppConfig` (mirrored there by `set_telemetry_consent`).
//!   * Suppressed entirely in debug builds (`tauri dev`) unless
//!     `PUBLIC_POSTHOG_ALLOW_DEV=1`, mirroring the JS clients' `!import.meta.env.DEV`
//!     gate so local development never pollutes the PostHog project.
//!   * PII-scrubbed before anything leaves the machine.
//!   * Uses the same anonymous `install_id` as JS events, so a Rust-side crash
//!     and a later JS event attribute to the same person.
//!
//! Sends go directly over HTTP (PostHog capture API) rather than forwarding to
//! JS, because a panic may occur with no live webview. The `reqwest` / EU-host
//! / release-key-baking conventions mirror `commands/auth.rs`.

use std::panic::PanicHookInfo;
use std::time::Duration;

use tauri::{AppHandle, Manager};

use crate::commands::types::AppState;

const DEFAULT_HOST: &str = "https://eu.i.posthog.com";

/// PostHog project key. Honored from the environment in debug builds (so dev
/// can point at a test project without recompiling); baked at compile time for
/// release, deliberately ignoring the runtime env — same stance as
/// `auth::cloud_api_url`, so an injected env can't redirect telemetry.
///
/// The `PUBLIC_` prefix is shared with the Svelte frontend (which reads the
/// same `PUBLIC_POSTHOG_KEY` via Vite's `import.meta.env`) and the web app, so
/// one injected value configures analytics on both sides of the app.
fn posthog_key() -> Option<String> {
    #[cfg(debug_assertions)]
    {
        std::env::var("PUBLIC_POSTHOG_KEY")
            .ok()
            .filter(|s| !s.is_empty())
    }
    #[cfg(not(debug_assertions))]
    {
        option_env!("PUBLIC_POSTHOG_KEY")
            .map(|s| s.to_string())
            .filter(|s| !s.is_empty())
    }
}

fn posthog_host() -> String {
    #[cfg(debug_assertions)]
    let v = std::env::var("PUBLIC_POSTHOG_HOST").ok();
    #[cfg(not(debug_assertions))]
    let v = option_env!("PUBLIC_POSTHOG_HOST").map(|s| s.to_string());
    v.filter(|s| !s.is_empty())
        .unwrap_or_else(|| DEFAULT_HOST.to_string())
}

/// Suppress all telemetry in debug builds so `tauri dev` never pollutes the
/// PostHog project — the native parity of the JS clients' `!import.meta.env.DEV`
/// gate. Opt back in with `PUBLIC_POSTHOG_ALLOW_DEV=1` to deliberately exercise
/// the crash path against a test project (the same intent the debug env-read in
/// `posthog_key` serves). Release builds always send, subject to consent.
fn dev_telemetry_suppressed() -> bool {
    #[cfg(debug_assertions)]
    {
        !std::env::var("PUBLIC_POSTHOG_ALLOW_DEV")
            .map(|v| matches!(v.as_str(), "1" | "true" | "TRUE"))
            .unwrap_or(false)
    }
    #[cfg(not(debug_assertions))]
    {
        false
    }
}

/// Redact filesystem paths and emails from free text before it leaves the
/// machine. The JS side (`packages/analytics/src/scrub.ts`) is the canonical,
/// fuller scrubber; this is the Rust parity pass for native payloads.
fn scrub(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    // Tokenize on whitespace and redact any token that looks like a path or
    // an email. Coarser than the JS regex set but covers the high-risk cases
    // (home directories, absolute paths, emails) without pulling in `regex`.
    for (i, token) in input.split_whitespace().enumerate() {
        if i > 0 {
            out.push(' ');
        }
        out.push_str(&redact_token(token));
    }
    out
}

fn redact_token(token: &str) -> String {
    // Email
    if token.contains('@') && token.contains('.') {
        return "<email>".to_string();
    }
    // Windows user home: C:\Users\<name>\... -> C:\Users\<user>
    if let Some(idx) = token.to_ascii_lowercase().find(r"\users\") {
        let prefix = &token[..idx + r"\users\".len()];
        return format!("{prefix}<user>");
    }
    // Unix home: /Users/<name> or /home/<name>
    for marker in ["/Users/", "/home/"] {
        if let Some(idx) = token.find(marker) {
            let prefix = &token[..idx + marker.len()];
            // Drop everything after the user segment.
            return format!("{prefix}<user>");
        }
    }
    // Bare absolute Windows path (C:\...) or extended path (\\?\...)
    if token.starts_with(r"\\?\") || (token.len() > 2 && token.as_bytes()[1] == b':') {
        return "<path>".to_string();
    }
    token.to_string()
}

/// Stable, low-cardinality fingerprint (djb2 over normalized text) so the same
/// crash groups together regardless of volatile numbers.
fn fingerprint(name: &str, message: &str) -> String {
    let normalized: String = format!("{name}:{message}")
        .chars()
        .map(|c| if c.is_ascii_digit() { '#' } else { c })
        .collect();
    let mut hash: u64 = 5381;
    for b in normalized.bytes() {
        hash = (hash.wrapping_mul(33)) ^ b as u64;
    }
    format!("{hash:x}")
}

/// Read the error-consent flag + anonymous install id from `AppConfig`. Falls
/// back to (true, anonymous) when state isn't managed yet — a very early panic
/// before `app.manage(AppState)`. Defaulting to ON matches the default-opt-in
/// stance; the anonymous id keeps it ungrouped rather than wrong.
fn read_consent(app: &AppHandle) -> (bool, String) {
    if let Some(state) = app.try_state::<AppState>() {
        let config = state.config.read();
        let id = config
            .install_id
            .clone()
            .unwrap_or_else(|| "anonymous-desktop".to_string());
        (config.telemetry_errors, id)
    } else {
        (true, "anonymous-desktop".to_string())
    }
}

/// Capture a scrubbed exception. Always fire-and-forget: the HTTP send happens
/// on a detached thread with its own short-lived runtime, so telemetry can never
/// block or stall the app — not even on the crashing thread during a panic. If
/// the process exits before the send finishes, the report is simply dropped
/// (best-effort delivery; reliability is never traded for blocking the app).
pub fn capture_exception(app: &AppHandle, name: &str, message: &str, stack: Option<String>) {
    if dev_telemetry_suppressed() {
        return;
    }
    let (errors_on, distinct_id) = read_consent(app);
    if !errors_on {
        return;
    }
    let Some(key) = posthog_key() else {
        return;
    };

    let host = posthog_host();
    let clean_message = scrub(message);
    let clean_stack = stack.map(|s| scrub(&s));
    let fp = fingerprint(name, &clean_message);

    let payload = serde_json::json!({
        "api_key": key,
        "event": "$exception",
        "distinct_id": distinct_id,
        "properties": {
            "$exception_type": name,
            "$exception_message": clean_message,
            "$exception_list": [{
                "type": name,
                "value": clean_message,
                "stacktrace": clean_stack.as_ref().map(|s| serde_json::json!({ "type": "raw", "raw": s })),
            }],
            "error_fingerprint": fp,
            "source": "desktop",
            "os": std::env::consts::OS,
            "app_version": env!("CARGO_PKG_VERSION"),
            "$lib": "recast-desktop-rust",
        }
    });

    let url = format!("{}/i/v0/e/", host.trim_end_matches('/'));
    // Detached — never joined. A panic may fire on a thread with no tokio
    // context, so we stand up a tiny current-thread runtime just for this send.
    // `Builder::spawn` returns a Result (vs `thread::spawn`, which panics if the
    // OS can't create a thread) so this can't double-panic inside the panic hook.
    let _ = std::thread::Builder::new().spawn(move || {
        let Ok(rt) = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        else {
            return;
        };
        rt.block_on(async {
            let Ok(client) = reqwest::Client::builder()
                .timeout(Duration::from_secs(5))
                .build()
            else {
                return;
            };
            let _ = client.post(&url).json(&payload).send().await;
        });
    });
}

/// Install the global panic hook. Always logs through the existing
/// `tauri-plugin-log` pipeline; additionally sends a scrubbed `$exception` to
/// PostHog when the user consents. Chains to the previous hook so default
/// behaviour (e.g. abort under `panic=abort`) is preserved.
pub fn install_panic_hook(app: AppHandle) {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info: &PanicHookInfo| {
        let location = info
            .location()
            .map(|l| format!("{}:{}", l.file(), l.line()))
            .unwrap_or_else(|| "<unknown>".to_string());
        let payload = if let Some(s) = info.payload().downcast_ref::<&str>() {
            (*s).to_string()
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "non-string panic payload".to_string()
        };
        let message = format!("panic at {location}: {payload}");
        log::error!("{message}");
        capture_exception(&app, "RustPanic", &message, Some(location));
        previous(info);
    }));
}
