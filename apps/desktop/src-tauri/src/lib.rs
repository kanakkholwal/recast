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

use commands::system::load_config;
use commands::types::AppState;
use parking_lot::Mutex;
use recording::RecordingManager;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let handle = app.handle();
            let config = load_config(&handle);

            app.manage(AppState {
                recording_manager: RecordingManager::default(),
                last_file_path: Mutex::new(None),
                config: Mutex::new(config),
                export_cancel: Mutex::new(HashMap::new()),
            });

            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            app.handle().plugin(tauri_plugin_dialog::init())?;
            app.handle().plugin(tauri_plugin_os::init())?;

            // Check FFmpeg availability at startup.
            if let Err(e) = ffmpeg::check_availability() {
                log::warn!("FFmpeg not available: {e}");
            }

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
            commands::start_recording,
            commands::stop_recording,
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
            commands::autosave_project,
            commands::clear_autosave,
            commands::get_recoverable_sessions,
            commands::suggest_zoom_regions,
            commands::ensure_assets_installed,
            commands::get_cached_asset_path
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
