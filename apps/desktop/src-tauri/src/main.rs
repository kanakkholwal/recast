// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // Headless CLI branch: `recast <verb>` enumerates capture sources as JSON
    // and exits before the webview boots. Normal launches (bare, `.recast`
    // file, `recast://` URL, `--new-recording`) fall through to the GUI.
    if recast_lib::cli::should_handle() {
        recast_lib::cli::run_and_exit();
    }
    recast_lib::run();
}
