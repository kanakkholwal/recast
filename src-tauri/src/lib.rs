mod audio;
mod capture;
mod commands;
mod cursor;
mod encoder;
mod project;
mod recording;
mod render;

use commands::types::AppState;
use commands::system::load_config;
use parking_lot::Mutex;
use recording::RecordingManager;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let handle = app.handle();
            let config = load_config(&handle);
            app.manage(AppState {
                recording_manager: RecordingManager::default(),
                last_file_path: Mutex::new(None),
                config: Mutex::new(config),
            });

            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            app.handle().plugin(tauri_plugin_dialog::init())?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_output_dir,
            commands::set_output_dir,
            commands::get_displays,
            commands::get_windows,
            commands::start_recording,
            commands::stop_recording,
            commands::list_recordings,
            commands::open_file_location,
            commands::get_video_metadata,
            commands::load_editor_document,
            commands::render_preview_frame,
            commands::generate_thumbnails,
            commands::export_video
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
