use std::collections::HashMap;

mod audio;
mod camera;
mod capture;
mod commands;
mod cursor;
mod encoder;
pub mod ffmpeg;
mod project;
mod recording;
mod render;
mod silence;

use commands::system::load_config;
use commands::types::AppState;
use parking_lot::Mutex;
use recording::RecordingManager;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_os::init());

    // JS-injecting plugins (dialog, os) MUST be added on the Builder before
    // any window is created — registering them later via `app.handle().plugin()`
    // inside `setup()` is too late: the WebView has already loaded the bundle
    // without the plugin's init script, so `window.__TAURI_OS_PLUGIN_INTERNALS__`
    // is undefined and synchronous calls like `platform()` throw at module
    // evaluation time, taking the whole frontend down. The Rust-only log plugin
    // can stay inside `setup()`.
    if cfg!(debug_assertions) {
        builder = builder.plugin(
            tauri_plugin_log::Builder::default()
                .level(log::LevelFilter::Info)
                .build(),
        );
    }

    builder
        .setup(|app| {
            let handle = app.handle();
            let config = load_config(&handle);

            app.manage(AppState {
                recording_manager: RecordingManager::default(),
                last_file_path: Mutex::new(None),
                config: Mutex::new(config),
                export_cancel: Mutex::new(HashMap::new()),
            });

            // FFmpeg path resolution probes ffmpeg/ffprobe `-version` against
            // up to 4 candidate locations, each spawn taking ~100–300 ms cold.
            // Doing this on the main thread froze the splash window for up to
            // a second on Windows. Resolve on a blocking worker; commands that
            // need the path will block on the OnceLock if they fire first.
            let resolver_handle = handle.clone();
            tauri::async_runtime::spawn_blocking(move || {
                ffmpeg::init(&resolver_handle);
                if let Err(e) = ffmpeg::check_availability() {
                    log::warn!("FFmpeg not available: {e}");
                }
            });

            // Startup: clean up stale temp files and orphaned session artifacts.
            let state = app.state::<AppState>();
            let output_dir = state.config.lock().output_dir.clone();
            if let Some(dir) = output_dir {
                project::autosave::cleanup_stale_sessions(std::path::Path::new(&dir));
            }

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
            commands::generate_thumbnails,
            commands::export_video,
            commands::cancel_export,
            commands::get_audio_devices,
            commands::get_camera_devices,
            commands::validate_camera_source,
            commands::update_camera_preview_state,
            commands::exclude_window_from_capture,
            commands::autosave_project,
            commands::save_project_edits,
            commands::clear_autosave,
            commands::get_recoverable_sessions,
            commands::suggest_zoom_regions,
            silence::detect_silence,
            commands::ensure_assets_installed,
            commands::get_cached_asset_path,
            commands::hydrate_cached_assets,
            commands::diagnose_ffmpeg
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            // Closing the main window must exit the whole app — auxiliary
            // windows (camera-preview, recording-panel, editor-*, region-picker,
            // …) would otherwise keep the process alive after the user thinks
            // they've quit.
            //
            // We close auxiliaries explicitly before `exit(0)` because on
            // Linux/Wayland the close-event delivery is racy: `exit(0)` can
            // tear the app down before the WM has finished delivering close
            // events to the aux windows, which on some compositors leaves
            // their surfaces lingering or blocks the main window's own close.
            // Explicit close-then-exit is deterministic on every platform.
            if let tauri::RunEvent::WindowEvent {
                label,
                event: tauri::WindowEvent::CloseRequested { .. },
                ..
            } = &event
            {
                if label == "main" {
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
