// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // Headless CLI branch exits before the webview boots; every normal launch falls through to the GUI.
    if recast_lib::cli::should_handle() {
        recast_lib::cli::run_and_exit();
    }
    recast_lib::run();
}
