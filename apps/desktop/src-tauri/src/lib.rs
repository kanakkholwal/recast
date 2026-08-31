use std::collections::HashMap;
use std::path::PathBuf;

mod audio;
pub mod audio_decode;
mod cache;
mod camera;
pub mod caption_parity;
mod capture;
pub mod cli;
mod commands;
mod control;
mod cursor;
mod db;
mod encoder;
pub mod export_audio;
pub mod export_engine;
pub mod export_parity;
pub mod ffmpeg;
mod fonts;
#[cfg(windows)]
mod jumplist;
mod mcp;
// Only the ocrs engine seam is behind the `ocr` feature; a no-default-features build still reports the engine absent.
mod ocr;
mod path_install;
mod permissions;
mod power;
mod project;
mod recording;
pub mod render;
mod silence;
mod telemetry;
mod transcription;
mod tray;
#[cfg(windows)]
mod window_aspect;

use commands::system::load_config;
use commands::types::AppState;
use parking_lot::Mutex;
use recording::RecordingManager;
use tauri::{Emitter, Manager};

/// A `.recast` path from argv when the OS launched us via the file association, else `None`.
/// Skips `argv[0]` and any `-` flag (dev flags, macOS `-psn_`), matches the extension case-insensitively, and verifies the file still exists.
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

/// Linux only: WebKitGTK ships `enable-media-stream` off, so `navigator.mediaDevices` is undefined until flipped and every `getUserMedia` raises a `permission-request` it denies by default.
/// Wired from `on_page_load` and deduped by label, so runtime-spawned windows that never pass through `setup()` are covered too.
#[cfg(target_os = "linux")]
fn enable_webview_media(webview: &tauri::Webview) {
    use parking_lot::Mutex;
    use std::collections::HashSet;
    use std::sync::OnceLock;

    static CONFIGURED: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    // Configure each webview once, or reloads stack handlers; `parking_lot::Mutex` can't poison a panic into an abort.
    if !CONFIGURED
        .get_or_init(|| Mutex::new(HashSet::new()))
        .lock()
        .insert(webview.label().to_string())
    {
        return;
    }

    let result = webview.with_webview(|platform| {
        // webkit2gtk 2.0.x has no `prelude` module, so pull the extension traits in directly.
        use webkit2gtk::glib::prelude::*;
        use webkit2gtk::{PermissionRequestExt, SettingsExt, WebViewExt};

        let wv = platform.inner();
        if let Some(settings) = wv.settings() {
            settings.set_enable_media_stream(true);
        }
        wv.connect_permission_request(|_, request| {
            // getUserMedia is the only permission this app triggers; leave the rest to WebKit's deny-by-default.
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

/// Registers `tauri-plugin-single-instance` in release builds only.
/// Its OS mutex is keyed on `app.identifier()`, identical in dev and release, so without the split a dev run forwards its argv to the installed binary and exits.
#[cfg(not(debug_assertions))]
fn install_singleton_plugin<R: tauri::Runtime>(builder: tauri::Builder<R>) -> tauri::Builder<R> {
    // MUST be first: the handler runs inside the second-launched process, where any earlier plugin would already have initialized.
    builder.plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.show();
            let _ = window.unminimize();
            let _ = window.set_focus();
        }
        // Warm start: the ghost process's argv arrives here, and close-to-tray keeps main's JS alive to catch the emit.
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
}

#[cfg(debug_assertions)]
fn install_singleton_plugin<R: tauri::Runtime>(builder: tauri::Builder<R>) -> tauri::Builder<R> {
    // Dev builds skip single-instance so `cargo tauri dev` can run alongside an installed build.
    builder
}

/// Delete entries directly under `root` (optionally only those named
/// `prefix*`) that nothing has touched for an hour. Best-effort throughout: a
/// dir still in use, or one we can't read, is simply left alone.
fn sweep_stale_temp(root: PathBuf, prefix: Option<&str>) {
    let Ok(entries) = std::fs::read_dir(&root) else {
        return;
    };
    let Some(cutoff) =
        std::time::SystemTime::now().checked_sub(std::time::Duration::from_secs(3600))
    else {
        return;
    };
    for entry in entries.flatten() {
        if let Some(prefix) = prefix {
            if !entry.file_name().to_string_lossy().starts_with(prefix) {
                continue;
            }
        }
        let Ok(meta) = entry.metadata() else { continue };
        if meta.modified().map(|m| m >= cutoff).unwrap_or(true) {
            continue;
        }
        let _ = if meta.is_dir() {
            std::fs::remove_dir_all(entry.path())
        } else {
            std::fs::remove_file(entry.path())
        };
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Explicit path, not `dotenvy::dotenv()`'s walk-up: the cargo CWD is `src-tauri/`, where a stray `.env` would shadow the app file.
    #[cfg(debug_assertions)]
    let _ = dotenvy::from_path(concat!(env!("CARGO_MANIFEST_DIR"), "/../.env"));

    let mut builder = tauri::Builder::default();
    // Gated on cfg(debug_assertions); in release this MUST be the first `.plugin(...)` call. See `install_singleton_plugin`.
    builder = install_singleton_plugin(builder);
    let mut builder = builder
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        // Injects JS, so it joins the pre-window group like dialog/os/sharekit.
        .plugin(tauri_plugin_clipboard_manager::init())
        // JS-injecting plugin: must be on the Builder before any window, like dialog and os below.
        .plugin(tauri_plugin_sharekit::init())
        // Deep-link injects JS into the webview, so it sits in the pre-window group too.
        .plugin(tauri_plugin_deep_link::init())
        // OS-wide hotkeys in Rust so they fire unfocused: Alt+Shift+R record, +P pause, +S capture area.
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
                    } else if shortcut == &Shortcut::new(Some(mods), Code::KeyS) && !recording {
                        // Refused mid-recording: the overlay lands in the recording it interrupts.
                        if let Err(e) = crate::commands::screenshot::open_region_overlay(app) {
                            log::warn!("region overlay failed to open: {e}");
                        }
                    }
                })
                .build(),
        )
        .plugin(tauri_plugin_os::init());

    // JS-injecting plugins must precede any window, or the bundle loads without their init script and `platform()` throws at module eval. Log ships in release too, at the level `apply_log_level` sets at runtime.
    builder = builder.plugin(
        tauri_plugin_log::Builder::default()
            .level(log::LevelFilter::Trace)
            // Cap crates that flood debug and trace (tract's shape dump, tao's event churn) at Warn even on full diagnostics.
            .level_for("tract_hir", log::LevelFilter::Warn)
            .level_for("tract_core", log::LevelFilter::Warn)
            .level_for("tract_linalg", log::LevelFilter::Warn)
            .level_for("tract_onnx", log::LevelFilter::Warn)
            .level_for("tract_nnef", log::LevelFilter::Warn)
            .level_for("tract_pulse", log::LevelFilter::Warn)
            .level_for("tao", log::LevelFilter::Warn)
            .level_for("wgpu_core", log::LevelFilter::Warn)
            .level_for("wgpu_hal", log::LevelFilter::Warn)
            .level_for("naga", log::LevelFilter::Warn)
            .build(),
    );

    builder
        // No-op outside Linux and WebKitGTK; macOS and Windows expose MediaDevices once their privacy gates are met.
        .on_page_load(|_webview, _payload| {
            #[cfg(target_os = "linux")]
            enable_webview_media(_webview);
        })
        .setup(|app| {
            let handle = app.handle();
            let mut config = load_config(handle);

            // First run: default output to <Videos>/Recast so recordings don't land in a temp dir the OS purges.
            if config.output_dir.is_none() {
                let default_dir = commands::system::default_output_dir(handle);
                let _ = std::fs::create_dir_all(&default_dir);
                config.output_dir = Some(default_dir.to_string_lossy().to_string());
                commands::system::save_config(handle, &config);
            }

            // The plugin was built at Trace: off leaves release at Warn, on captures backend and forwarded webview diagnostics.
            commands::system::apply_log_level(config.diagnostic_logging);

            // Seed the self-host override so the no-arg `cloud_api_url()` reflects the saved choice from the first request.
            commands::auth::init_cloud_api_override(config.cloud_api_url.clone());

            // Cold start: stash any `.recast` arg for the main window to drain via `take_pending_open_file`.
            let cold_open_file: Vec<String> = std::env::args().collect();
            let pending_open_file = parse_open_arg(&cold_open_file);
            let launched_for_new_recording = cold_open_file.iter().any(|a| a == "--new-recording");

            // One source for the CLI and the panel; an absent file seeds in memory with initialized=false.
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
                registered_shortcuts: Mutex::new(Vec::new()),
                pending_new_recording: std::sync::atomic::AtomicBool::new(
                    launched_for_new_recording,
                ),
                capture_intent: parking_lot::RwLock::new(commands::types::CaptureIntent::default()),
                profiles: parking_lot::RwLock::new(profiles_state),
                profiles_initialized: std::sync::atomic::AtomicBool::new(profiles_initialized),
                db: crate::db::Db::open(handle),
                export_wake: std::sync::Arc::new(tokio::sync::Notify::new()),
                editor_session: parking_lot::RwLock::new(commands::types::EditorSession::default()),
            });

            // Restore a previously-held editor lock if its holder is still alive; failure just leaves the lock idle.
            let _ =
                commands::load_on_startup(app.state::<commands::types::AppState>().inner(), handle);

            // One-shot CLI install so `recast --help` works out of the box; opt out via `cli_auto_install`, and `recast uninstall` is sticky.
            let app_state = app.state::<commands::types::AppState>();
            {
                let mut cfg = app_state.config.write();
                if !cfg.cli_install_attempted {
                    cfg.cli_install_attempted = true;
                    commands::system::save_config(handle, &cfg);
                    let cli_auto = cfg.cli_auto_install;
                    drop(cfg);
                    if cli_auto && !crate::path_install::status().on_path {
                        match crate::path_install::install() {
                            Ok(msg) => log::info!("cli auto-install: {msg}"),
                            Err(e) => log::warn!("cli auto-install failed: {e}"),
                        }
                    }
                }
            }

            // Mark any job left mid-run as interrupted, then start the serial worker. Must run after AppState is managed.
            commands::export_queue::reconcile_on_load(handle);
            commands::export_queue::spawn_export_worker(handle.clone());
            // GC terminal jobs past the TTL and orphaned payloads, on a blocking worker so startup never stalls.
            commands::export_queue::sweep_stale_jobs(handle);

            // Dev only: release installers write the registry or .desktop entry, and macOS cannot register at runtime.
            #[cfg(any(target_os = "linux", all(debug_assertions, windows)))]
            {
                use tauri_plugin_deep_link::DeepLinkExt;
                if let Err(e) = app.deep_link().register("recast") {
                    log::warn!("deep-link register failed: {e}");
                }
            }

            // Bring the app forward on a `recast://` URL; the frontend does the routing via getCurrent and onOpenUrl.
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

            // Non-fatal on conflict. Registrations are stashed so `run` can unregister them: the plugin never auto-releases.
            use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut};
            let registered: Vec<Shortcut> = {
                let mods = Modifiers::ALT | Modifiers::SHIFT;
                let mut registered = Vec::with_capacity(3);
                for sc in [
                    Shortcut::new(Some(mods), Code::KeyR),
                    Shortcut::new(Some(mods), Code::KeyP),
                    Shortcut::new(Some(mods), Code::KeyS),
                ] {
                    match app.global_shortcut().register(sc) {
                        Ok(()) => registered.push(sc),
                        Err(e) => log::warn!("global shortcut register failed: {e}"),
                    }
                }
                registered
            };
            if let Some(state) = app.try_state::<AppState>() {
                *state.registered_shortcuts.lock() = registered;
            }

            // After AppState, so the panic hook can read the consent flag and install id; PII-scrubbed.
            telemetry::install_panic_hook(handle.clone());

            // Non-fatal: without a tray the user only loses quick actions while the window is hidden.
            if let Err(e) = tray::init(handle) {
                log::warn!("tray init failed: {e}");
            }

            #[cfg(windows)]
            jumplist::update(handle);

            // Local control server for the CLI; non-fatal if it can't bind, the CLI just can't reach this instance.
            control::spawn_server(handle.clone());

            // Off the main thread: probing four FFmpeg locations plus the encoder froze the splash ~1s and delayed the first recording.
            let resolver_handle = handle.clone();
            tauri::async_runtime::spawn_blocking(move || {
                ffmpeg::init(&resolver_handle);
                if let Err(e) = ffmpeg::check_availability() {
                    log::warn!("FFmpeg not available: {e}");
                }
                // Touch the OnceLock so the encoder probe runs here, not during the user's first recording.
                let _ = ffmpeg::preferred_h264_encoder();
            });

            // Startup: clean up stale temp files and orphaned session artifacts.
            let state = app.state::<AppState>();
            let output_dir = state.config.read().output_dir.clone();
            if let Some(dir) = output_dir {
                // The last disk walk in setup(), which runs before the event loop and delayed first paint on macOS.
                tauri::async_runtime::spawn_blocking(move || {
                    project::autosave::cleanup_stale_sessions(std::path::Path::new(&dir));
                });
            }

            // Startup-only: nothing is open yet, so no live editor can lose the assets under it.
            tauri::async_runtime::spawn_blocking(project::reader::sweep_cache);

            // `Drop` doesn't run on a kill, so scratch dirs pile up; startup plus single-instance means nothing is still writing.
            tauri::async_runtime::spawn_blocking(|| {
                // `recast-thumbnails/*`: orphaned JPEGs from a crash mid-scrub.
                sweep_stale_temp(std::env::temp_dir().join("recast-thumbnails"), None);
                // TempDirGuard's territory; `cursor.mov` alone is lossless QTRLE at composite resolution for the whole timeline.
                sweep_stale_temp(std::env::temp_dir(), Some("recast-export-"));
                // Oversized `-filter_complex_script` files.
                sweep_stale_temp(std::env::temp_dir(), Some("recast-filtergraph-"));
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
            commands::caption_sidecar_vtt,
            commands::open_file_location,
            commands::delete_file,
            commands::rename_file,
            commands::get_video_metadata,
            commands::load_editor_document,
            commands::list_branches,
            commands::create_branch,
            commands::append_to_branch,
            commands::diff_branch,
            commands::materialize_branch,
            commands::truncate_branch,
            commands::discard_branch,
            commands::apply_branch,
            commands::get_editor_session,
            commands::acquire_editor_write,
            commands::release_editor_write,
            commands::force_release_editor_write,
            commands::migrate_project,
            commands::generate_thumbnails,
            commands::cancel_export,
            commands::enqueue_export,
            commands::list_export_jobs,
            commands::cancel_export_job,
            commands::dismiss_export_job,
            commands::retry_export_job,
            commands::screenshot::capture_region_shot,
            commands::screenshot::open_area_picker,
            commands::get_audio_devices,
            commands::get_camera_devices,
            commands::validate_camera_source,
            commands::update_camera_preview_state,
            commands::start_camera_preview,
            commands::stop_camera_preview,
            commands::save_browser_export_video,
            commands::exclude_window_from_capture,
            commands::set_window_aspect_ratio,
            commands::autosave_project,
            commands::save_project_edits,
            commands::clear_autosave,
            commands::get_recoverable_sessions,
            commands::suggest_zoom_regions,
            silence::detect_silence,
            silence::extract_waveform,
            ocr::command::read_video_text,
            ocr::command::export_screen_text,
            transcription::list_caption_models,
            transcription::caption_capabilities,
            transcription::download_caption_model,
            transcription::delete_caption_model,
            transcription::transcribe_project,
            transcription::cancel_transcription,
            transcription::has_transcribable_audio,
            transcription::export_captions,
            transcription::list_remote_asr_endpoints,
            transcription::set_remote_asr_endpoint,
            transcription::delete_remote_asr_endpoint,
            transcription::set_remote_asr_key,
            fonts::ensure_google_font,
            fonts::caption_font_file,
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
            commands::get_cli_auto_install,
            commands::set_cli_auto_install,
            commands::get_native_encoder,
            commands::set_native_encoder,
            commands::native_encoder_available,
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
            // The plugin never auto-releases, so an unclean close leaves the hotkey bound to a dead process for the OS lifetime.
            if matches!(
                event,
                tauri::RunEvent::Exit | tauri::RunEvent::ExitRequested { .. }
            ) {
                use tauri_plugin_global_shortcut::GlobalShortcutExt;
                if let Some(state) = app_handle.try_state::<AppState>() {
                    let mut shortcuts = state.registered_shortcuts.lock();
                    for sc in shortcuts.drain(..) {
                        if let Err(e) = app_handle.global_shortcut().unregister(sc) {
                            log::warn!("global shortcut unregister failed: {e}");
                        }
                    }
                }
                // Flush the lock snapshot so a managed shutdown doesn't strand a held lock in memory only.
                commands::persist(app_handle.state::<AppState>().inner(), app_handle);
            }

            // Only on `Exit`: tray quit calls `process::exit` and skips `Drop for RecordingManager`, so mic and camera children would outlive the app.
            if matches!(event, tauri::RunEvent::Exit) {
                if let Some(state) = app_handle.try_state::<AppState>() {
                    state.recording_manager.abort_for_shutdown();
                }
            }

            // macOS delivers file opens via RunEvent::Opened, not argv; filter to file:// so the deep-link plugin keeps `recast://`.
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
                    // Warm path: main's JS listener catches it. Cold path: stash for the mount drain. Both dedupe by window label.
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

            // Rust owns the release: the panel closes the preview with a raw `close()`, skipping its teardown.
            if let tauri::RunEvent::WindowEvent {
                label,
                event: tauri::WindowEvent::Destroyed,
                ..
            } = &event
            {
                // A replacement under the same label means this was a reopen, not a close.
                if label == "camera-preview"
                    && app_handle.get_webview_window("camera-preview").is_none()
                {
                    crate::camera::session::release();
                }
            }

            // close_to_tray hides instead of closing (the tray is the only way back); otherwise auxiliaries close before exit(0) so Wayland doesn't race teardown.
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
