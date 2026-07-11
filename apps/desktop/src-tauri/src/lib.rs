use std::collections::HashMap;
use std::path::PathBuf;

mod audio;
mod cache;
mod camera;
mod capture;
pub mod cli;
mod commands;
mod control;
mod cursor;
mod db;
mod encoder;
pub mod ffmpeg;
mod fonts;
#[cfg(windows)]
mod jumplist;
mod path_install;
mod permissions;
mod power;
mod project;
mod recording;
mod render;
mod silence;
mod telemetry;
mod transcription;
mod tray;
mod window_aspect;

use commands::system::load_config;
use commands::types::AppState;
use parking_lot::Mutex;
use recording::RecordingManager;
use tauri::{Emitter, Manager};

/// Pull a `.recast` file path out of process argv if the OS launched us
/// with one via the file association (Windows registry shell-open, macOS
/// LaunchServices, Linux xdg-open). Returns `None` for normal launches.
///
/// Defensive rules:
/// * Skip `argv[0]` (executable path).
/// * Skip any arg starting with `-` — covers dev-mode flags (`--port`,
///   etc.) and the macOS `-psn_NNNN_NNNN` process serial number that
///   LaunchServices sometimes prepends.
/// * Match the extension case-insensitively — Windows is case-insensitive
///   and APFS *can* be case-sensitive, so users may have `.Recast` files.
/// * Verify the path exists. If a user double-clicks then deletes the file
///   before we boot, we want to report "no longer exists" instead of
///   navigating to an editor window that immediately errors.
fn parse_open_arg(argv: &[String]) -> Option<PathBuf> {
    argv.iter()
        .skip(1)
        .filter(|a| !a.starts_with('-'))
        .map(PathBuf::from)
        .find(|p| {
            p.extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case("recast"))
                && p.exists()
        })
}

/// Linux (WebKitGTK) only: enable `getUserMedia`/`enumerateDevices` for a
/// webview and grant the media `permission-request` it raises.
///
/// macOS (WKWebView) and Windows (WebView2) expose `navigator.mediaDevices`
/// as soon as the OS-level privacy gates are satisfied (see `Info.plist` for
/// the macOS usage strings). WebKitGTK is the odd one out: it ships with
/// `enable-media-stream` OFF, so `navigator.mediaDevices` is `undefined`
/// until we flip it — and even then every `getUserMedia` call raises a
/// `permission-request` that WebKit DENIES by default unless answered.
///
/// Applied per-webview and deduped by label (a `OnceLock` set) so it also
/// covers the `camera-preview` / `device-picker` windows, which the frontend
/// spawns at runtime via the JS `WebviewWindow` API — they never pass through
/// `setup()`. Wired from `on_page_load`, which fires for every webview
/// regardless of how it was created.
#[cfg(target_os = "linux")]
fn enable_webview_media(webview: &tauri::Webview) {
    use parking_lot::Mutex;
    use std::collections::HashSet;
    use std::sync::OnceLock;

    static CONFIGURED: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    // Connecting the signal twice would stack handlers across reloads, so
    // configure each webview exactly once. `parking_lot::Mutex` can't poison —
    // a panic here would otherwise abort the app rather than just leaving media
    // unconfigured for one webview.
    if !CONFIGURED
        .get_or_init(|| Mutex::new(HashSet::new()))
        .lock()
        .insert(webview.label().to_string())
    {
        return;
    }

    let result = webview.with_webview(|platform| {
        // webkit2gtk 2.0.x has no `prelude` module — pull the extension
        // traits in directly. `WebViewExt` gives `settings()` +
        // `connect_permission_request()`, `SettingsExt` gives
        // `set_enable_media_stream()`, `PermissionRequestExt` gives
        // `allow()`, and glib's `Cast` (via its prelude) gives `.is::<T>()`.
        use webkit2gtk::glib::prelude::*;
        use webkit2gtk::{PermissionRequestExt, SettingsExt, WebViewExt};

        let wv = platform.inner();
        if let Some(settings) = wv.settings() {
            settings.set_enable_media_stream(true);
        }
        wv.connect_permission_request(|_, request| {
            // getUserMedia (camera-preview + device-picker) is the only
            // permission this app ever triggers. Grant it; leave anything
            // else to WebKit's deny-by-default rather than blanket-allowing.
            if request.is::<webkit2gtk::UserMediaPermissionRequest>() {
                request.allow();
            }
            true
        });
    });
    if let Err(e) = result {
        log::warn!("failed to enable webview media on Linux: {e}");
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Load the desktop app's single `.env` at `apps/desktop/.env` (the same
    // file Vite reads), so dev config lives in ONE place for both the Svelte
    // frontend and this Rust backend: PUBLIC_POSTHOG_*, GOOGLE_OAUTH_*,
    // CLOUD_API_URL, TAURI_SIGNING_PRIVATE_KEY, … We load it by an EXPLICIT
    // path rather than `dotenvy::dotenv()`'s walk-up: the cargo CWD is
    // `src-tauri/`, and a stray `.env` there would otherwise shadow the app
    // file. `CARGO_MANIFEST_DIR` is `…/apps/desktop/src-tauri`, so `../.env` is
    // the app root .env. Silent on missing/invalid file — release installs have
    // no .env (and release reads creds via `option_env!` at build time anyway).
    #[cfg(debug_assertions)]
    let _ = dotenvy::from_path(concat!(env!("CARGO_MANIFEST_DIR"), "/../.env"));

    let mut builder = tauri::Builder::default()
        // Single-instance MUST be the first plugin registered. The handler
        // fires inside the second-launched process — by the time it runs,
        // any later plugin would have already initialized in that ghost
        // process. The plugin shuts the ghost down after the handler returns,
        // so we just refocus the existing window and exit.
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
            // Warm-start file-association path: the ghost process's argv is
            // forwarded here. Emit to the main window which always-new-windows
            // it via openProjectFromExternalPath. Close-to-tray keeps main's
            // JS alive even when hidden, so the listener catches this.
            if let Some(path) = parse_open_arg(&argv) {
                let payload = path.to_string_lossy().to_string();
                if let Err(e) = app.emit("app://open-recast", payload) {
                    log::warn!("emit app://open-recast failed: {e}");
                }
            }
            // Jump list "New Recording" task on a running app.
            if argv.iter().any(|a| a == "--new-recording") {
                let _ = app.emit("global-shortcut:launch-panel", ());
            }
        }))
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        // JS-injecting plugin — must be on the Builder before any window,
        // same constraint as dialog/os (see the comment block below).
        .plugin(tauri_plugin_sharekit::init())
        // Deep-link injects JS (onOpenUrl/getCurrent) into the webview, so it
        // sits in the pre-window group like dialog/os/sharekit.
        .plugin(tauri_plugin_deep_link::init())
        // OS-wide recording hotkeys, handled in Rust so they fire when Recast is
        // unfocused. Alt+Shift+R stops (routed to the panel via tray:record-toggle)
        // when recording, else launches the panel; Alt+Shift+P pauses/resumes.
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    use tauri_plugin_global_shortcut::{Code, Modifiers, Shortcut, ShortcutState};
                    if event.state() != ShortcutState::Pressed {
                        return;
                    }
                    let mods = Modifiers::ALT | Modifiers::SHIFT;
                    let recording = crate::tray::is_recording_active();
                    if shortcut == &Shortcut::new(Some(mods), Code::KeyR) {
                        let _ = if recording {
                            app.emit("tray:record-toggle", ())
                        } else {
                            app.emit("global-shortcut:launch-panel", ())
                        };
                    } else if shortcut == &Shortcut::new(Some(mods), Code::KeyP) && recording {
                        let _ = app.emit("global-shortcut:toggle-pause", ());
                    }
                })
                .build(),
        )
        .plugin(tauri_plugin_os::init());

    // JS-injecting plugins (dialog, os) MUST be added on the Builder before
    // any window is created — registering them later via `app.handle().plugin()`
    // inside `setup()` is too late: the WebView has already loaded the bundle
    // without the plugin's init script, so `window.__TAURI_OS_PLUGIN_INTERNALS__`
    // is undefined and synchronous calls like `platform()` throw at module
    // evaluation time, taking the whole frontend down. The Rust-only log plugin
    // can stay inside `setup()`.
    //
    // Why log in release too: without this, MSI/NSIS/DMG installs were
    // silent — when a user hit a recording error there was no way to ask
    // them for a log file, so every report had to be reproduced live.
    // `tauri_plugin_log`'s defaults write to both stdout AND a rotating
    // file under the OS log dir (Windows: `%LOCALAPPDATA%\com.kanakkholwal.recast\logs\`,
    // macOS: `~/Library/Logs/com.kanakkholwal.recast/`, Linux:
    // `~/.local/share/com.kanakkholwal.recast/logs/`).
    //
    // The dispatch is built permissively (Trace); the EFFECTIVE level is set at
    // runtime by `commands::system::apply_log_level` from the persisted
    // `diagnostic_logging` flag (see below in `setup`). That single
    // `log::set_max_level` gate covers both the Rust backend and the webview
    // logs the frontend forwards through this same plugin — so a user can flip
    // verbose diagnostics on without a restart. Default stays quiet: Warn in
    // release (no per-frame info noise on user disks), Info in debug builds.
    builder = builder.plugin(
        tauri_plugin_log::Builder::default()
            .level(log::LevelFilter::Trace)
            .build(),
    );

    builder
        // Enable camera/mic in the WebView the moment each page starts
        // loading. No-op everywhere but Linux (WebKitGTK); macOS/Windows
        // expose MediaDevices natively once their privacy gates are met.
        .on_page_load(|_webview, _payload| {
            #[cfg(target_os = "linux")]
            enable_webview_media(_webview);
        })
        .setup(|app| {
            let handle = app.handle();
            let mut config = load_config(handle);

            // First run: default the output location to <Videos>/Recast so
            // recordings land somewhere discoverable and durable, not the temp
            // dir the OS periodically purges. Persisted so it shows in Settings
            // and the user can still change it.
            if config.output_dir.is_none() {
                let default_dir = commands::system::default_output_dir(handle);
                let _ = std::fs::create_dir_all(&default_dir);
                config.output_dir = Some(default_dir.to_string_lossy().to_string());
                commands::system::save_config(handle, &config);
            }

            // Apply the saved log verbosity now (the plugin was built at Trace).
            // Off by default → release stays at Warn; on → Debug captures
            // backend + forwarded webview diagnostics for support bundles.
            commands::system::apply_log_level(config.diagnostic_logging);

            // Seed the self-host cloud-endpoint override from persisted config
            // so the no-arg `cloud_api_url()` resolver reflects the user's
            // saved choice from the very first auth/sync request onward.
            commands::auth::init_cloud_api_override(config.cloud_api_url.clone());

            // Cold-start file-association path: stash any `.recast` arg the
            // OS handed us so the main window can drain it on mount via
            // `take_pending_open_file`. None for a normal launch.
            let cold_open_file: Vec<String> = std::env::args().collect();
            let pending_open_file = parse_open_arg(&cold_open_file);
            let launched_for_new_recording = cold_open_file.iter().any(|a| a == "--new-recording");

            // Load the saved recording profiles into the shared store so the CLI
            // (`recast profile list/use`) and the panel read one source. Absent
            // file => in-memory seed, initialized=false (frontend migrates once).
            let (profiles_state, profiles_initialized) =
                commands::profiles::load_profiles_state(handle);

            app.manage(AppState {
                recording_manager: std::sync::Arc::new(RecordingManager::default()),
                last_file_path: Mutex::new(None),
                config: parking_lot::RwLock::new(config),
                export_cancel: Mutex::new(HashMap::new()),
                auth_poller: Mutex::new(None),
                pending_open_file: Mutex::new(pending_open_file),
                power: crate::power::PowerManager::new(),
                pending_new_recording: std::sync::atomic::AtomicBool::new(
                    launched_for_new_recording,
                ),
                capture_intent: parking_lot::RwLock::new(commands::types::CaptureIntent::default()),
                profiles: parking_lot::RwLock::new(profiles_state),
                profiles_initialized: std::sync::atomic::AtomicBool::new(profiles_initialized),
                db: crate::db::Db::open(handle),
                export_wake: std::sync::Arc::new(tokio::sync::Notify::new()),
            });

            // Export queue: recover any job left mid-run by an unclean shutdown
            // (mark it interrupted), then start the single serial worker that
            // drains the queue. Must run after AppState is managed.
            commands::export_queue::reconcile_on_load(handle);
            commands::export_queue::spawn_export_worker(handle.clone());
            // GC stale queue entries (terminal jobs older than the TTL) + orphaned
            // payloads. Runs on a blocking worker so it never stalls startup.
            commands::export_queue::sweep_stale_jobs(handle);

            // Register the `recast://` scheme at runtime for dev builds. In
            // release the installer writes the Windows registry / Linux .desktop
            // entry from tauri.conf; macOS uses the generated Info.plist and
            // cannot register at runtime.
            #[cfg(any(target_os = "linux", all(debug_assertions, windows)))]
            {
                use tauri_plugin_deep_link::DeepLinkExt;
                if let Err(e) = app.deep_link().register("recast") {
                    log::warn!("deep-link register failed: {e}");
                }
            }

            // Bring the app forward when a `recast://` URL arrives (esp. macOS
            // in-process delivery and close-to-tray). Routing itself is done in
            // the frontend via getCurrent()/onOpenUrl() → handleDeepLink.
            {
                use tauri_plugin_deep_link::DeepLinkExt;
                let focus_handle = handle.clone();
                app.deep_link().on_open_url(move |_event| {
                    if let Some(w) = focus_handle.get_webview_window("main") {
                        let _ = w.show();
                        let _ = w.unminimize();
                        let _ = w.set_focus();
                    }
                });
            }

            // Register the OS-wide hotkeys. Non-fatal: a conflict (another app
            // owns the combo) just makes that hotkey unavailable.
            {
                use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut};
                let mods = Modifiers::ALT | Modifiers::SHIFT;
                for sc in [
                    Shortcut::new(Some(mods), Code::KeyR),
                    Shortcut::new(Some(mods), Code::KeyP),
                ] {
                    if let Err(e) = app.global_shortcut().register(sc) {
                        log::warn!("global shortcut register failed: {e}");
                    }
                }
            }

            // Native crash reporting. Installed after AppState is managed so the
            // panic hook can read the consent flag + install id. Gated on the
            // user's `telemetry_errors` consent (default on) and PII-scrubbed.
            telemetry::install_panic_hook(handle.clone());

            // System tray. Init failure is non-fatal — the app still works
            // without a tray (the user just can't quick-access actions while
            // the window is hidden, which is fine). Log + continue.
            if let Err(e) = tray::init(handle) {
                log::warn!("tray init failed: {e}");
            }

            #[cfg(windows)]
            jumplist::update(handle);

            // Local control server for the `recast` CLI (status, rec ...).
            // Non-fatal if it can't bind: the GUI is unaffected, the CLI just
            // can't reach this instance.
            control::spawn_server(handle.clone());

            // FFmpeg path resolution probes ffmpeg/ffprobe `-version` against
            // up to 4 candidate locations, each spawn taking ~100–300 ms cold.
            // Doing this on the main thread froze the splash window for up to
            // a second on Windows. Resolve on a blocking worker; commands that
            // need the path will block on the OnceLock if they fire first.
            //
            // We also pre-warm `preferred_h264_encoder()` here (one extra
            // `ffmpeg -encoders` spawn, also ~200–300 ms cold). Without this,
            // the encoder probe ran *during the first recording-start*,
            // delaying the start_recording command by that much — the Windows
            // tester report described it as "the whole window freezes
            // suddenly". Pre-warming on the same blocking worker that
            // resolves FFmpeg paths fixes the first-recording case without
            // adding any extra spawn for subsequent recordings (the result is
            // cached behind an OnceLock).
            let resolver_handle = handle.clone();
            tauri::async_runtime::spawn_blocking(move || {
                ffmpeg::init(&resolver_handle);
                if let Err(e) = ffmpeg::check_availability() {
                    log::warn!("FFmpeg not available: {e}");
                }
                // Touch the OnceLock so the encoder probe runs here, not
                // during the user's first recording. Result is ignored —
                // the function logs internally and falls back to libx264
                // on probe failure.
                let _ = ffmpeg::preferred_h264_encoder();
            });

            // Startup: clean up stale temp files and orphaned session artifacts.
            let state = app.state::<AppState>();
            let output_dir = state.config.read().output_dir.clone();
            if let Some(dir) = output_dir {
                project::autosave::cleanup_stale_sessions(std::path::Path::new(&dir));
            }

            // Sweep abandoned `recast-thumbnails/*` subdirs left behind by
            // crashed/killed editor sessions. The thumbnail extractor
            // best-effort-removes its own per-invocation dir, but a process
            // crash mid-scrub leaks the directory — on a long-running install
            // these can accumulate gigabytes of orphaned JPEGs. Anything
            // older than ~1 hour is safe to drop (no live process is still
            // writing into it).
            tauri::async_runtime::spawn_blocking(|| {
                let thumb_root = std::env::temp_dir().join("recast-thumbnails");
                let Ok(entries) = std::fs::read_dir(&thumb_root) else {
                    return;
                };
                let cutoff =
                    std::time::SystemTime::now().checked_sub(std::time::Duration::from_secs(3600));
                for entry in entries.flatten() {
                    let stale = entry
                        .metadata()
                        .and_then(|m| m.modified())
                        .ok()
                        .zip(cutoff)
                        .map(|(modified, cutoff)| modified < cutoff)
                        .unwrap_or(false);
                    if stale {
                        let _ = std::fs::remove_dir_all(entry.path());
                    }
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_output_dir,
            commands::set_output_dir,
            commands::get_displays,
            commands::get_windows,
            commands::get_last_source,
            commands::set_last_source,
            commands::start_recording,
            commands::stop_recording,
            commands::pause_recording,
            commands::resume_recording,
            commands::is_recording_paused,
            commands::list_recasts,
            commands::list_exports,
            commands::open_file_location,
            commands::delete_file,
            commands::rename_file,
            commands::get_video_metadata,
            commands::load_editor_document,
            commands::migrate_project,
            commands::generate_thumbnails,
            commands::cancel_export,
            commands::enqueue_export,
            commands::list_export_jobs,
            commands::cancel_export_job,
            commands::dismiss_export_job,
            commands::retry_export_job,
            commands::get_audio_devices,
            commands::get_camera_devices,
            commands::validate_camera_source,
            commands::update_camera_preview_state,
            commands::exclude_window_from_capture,
            commands::set_window_aspect_ratio,
            commands::autosave_project,
            commands::save_project_edits,
            commands::clear_autosave,
            commands::get_recoverable_sessions,
            commands::suggest_zoom_regions,
            silence::detect_silence,
            silence::extract_waveform,
            transcription::list_caption_models,
            transcription::caption_capabilities,
            transcription::download_caption_model,
            transcription::delete_caption_model,
            transcription::transcribe_project,
            transcription::has_transcribable_audio,
            transcription::export_captions,
            transcription::list_remote_asr_endpoints,
            transcription::set_remote_asr_endpoint,
            transcription::delete_remote_asr_endpoint,
            transcription::set_remote_asr_key,
            fonts::ensure_google_font,
            commands::ensure_assets_installed,
            commands::get_cached_asset_path,
            commands::hydrate_cached_assets,
            commands::install_extension,
            commands::list_installed_extensions,
            commands::set_extension_enabled,
            commands::uninstall_extension,
            commands::fetch_extension_registry,
            commands::diagnose_ffmpeg,
            commands::probe_video_encoders,
            commands::capture_capabilities,
            commands::cli_install_status,
            commands::install_cli,
            commands::uninstall_cli,
            commands::get_capture_intent,
            commands::set_capture_intent,
            commands::get_profiles,
            commands::set_profiles,
            commands::use_profile,
            commands::auth_start,
            commands::auth_status,
            commands::auth_sign_out,
            commands::auth_cancel,
            commands::get_cloud_api_config,
            commands::set_cloud_api_url,
            commands::get_diagnostic_logging,
            commands::set_diagnostic_logging,
            commands::open_log_dir,
            commands::get_close_to_tray,
            commands::set_close_to_tray,
            commands::get_hide_panel_from_capture,
            commands::set_hide_panel_from_capture,
            commands::get_window_transparency,
            commands::set_window_transparency,
            commands::set_telemetry_consent,
            commands::gdrive_connect,
            commands::gdrive_status,
            commands::gdrive_disconnect,
            commands::gdrive_upload,
            commands::gdrive_cancel_upload,
            commands::gdrive_list_uploads,
            commands::gdrive_forget_upload,
            commands::recast_cloud_upload,
            commands::recast_cloud_update_share,
            commands::recast_cloud_delete,
            commands::recast_cloud_list_shares,
            commands::recast_cloud_list_uploads,
            commands::recast_cloud_forget_upload,
            commands::take_pending_open_file,
            commands::peek_recast_project,
            commands::take_pending_new_recording,
            commands::is_recording_active,
            tray::refresh_tray
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            // macOS/iOS deliver file-association opens (a double-clicked
            // `.recast` in Finder) and URL opens via RunEvent::Opened, NOT argv
            // — so the argv/single-instance path that works on Windows/Linux
            // never fires here. Route file:// paths through the same
            // `app://open-recast` bridge. `recast://` scheme URLs are owned by
            // the deep-link plugin's on_open_url, so filter to file:// to avoid
            // double-handling.
            #[cfg(any(target_os = "macos", target_os = "ios"))]
            if let tauri::RunEvent::Opened { urls } = &event {
                for url in urls {
                    if url.scheme() != "file" {
                        continue;
                    }
                    let Ok(path) = url.to_file_path() else {
                        continue;
                    };
                    let is_recast = path
                        .extension()
                        .and_then(|e| e.to_str())
                        .is_some_and(|e| e.eq_ignore_ascii_case("recast"));
                    if !is_recast || !path.exists() {
                        continue;
                    }
                    // Warm path: main's JS listener catches the event (close-to-
                    // tray keeps it alive). Cold path: stash so the mount-time
                    // drain picks it up. Both are safe — openProjectInNewWindow
                    // dedupes by window label, so a double-fire just refocuses.
                    if let Some(state) = app_handle.try_state::<AppState>() {
                        *state.pending_open_file.lock() = Some(path.clone());
                    }
                    let payload = path.to_string_lossy().to_string();
                    if let Err(e) = app_handle.emit("app://open-recast", payload) {
                        log::warn!("emit app://open-recast (macOS Opened) failed: {e}");
                    }
                    if let Some(w) = app_handle.get_webview_window("main") {
                        let _ = w.show();
                        let _ = w.set_focus();
                    }
                }
            }

            // Main-window close handling has two modes, gated by the user's
            // `close_to_tray` setting (default on):
            //
            //   * close_to_tray=true: prevent the close, hide the window
            //     instead. The tray icon is the only way to bring the app
            //     back or to truly quit. Background captures (recording,
            //     editor autosave) keep running.
            //
            //   * close_to_tray=false: legacy behavior — close auxiliaries
            //     explicitly before exit(0) so Linux/Wayland doesn't race
            //     surface teardown against the main-thread exit.
            //
            // Tray "Quit" calls `app.exit(0)` directly, bypassing this
            // branch entirely (no CloseRequested event fires).
            if let tauri::RunEvent::WindowEvent {
                label,
                event: tauri::WindowEvent::CloseRequested { api, .. },
                ..
            } = &event
            {
                if label == "main" {
                    let hide_to_tray = app_handle
                        .try_state::<AppState>()
                        .map(|state| state.config.read().close_to_tray)
                        .unwrap_or(true);

                    if hide_to_tray {
                        api.prevent_close();
                        if let Some(window) = app_handle.get_webview_window("main") {
                            let _ = window.hide();
                        }
                        tray::rebuild_menu(app_handle);
                        return;
                    }

                    for (aux_label, window) in app_handle.webview_windows() {
                        if aux_label != "main" {
                            let _ = window.close();
                        }
                    }
                    app_handle.exit(0);
                }
            }
        });
}
