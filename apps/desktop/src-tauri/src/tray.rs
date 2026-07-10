//! System tray icon, menu, and event wiring.
//!
//! The tray is the canonical entry point for quick actions while the main
//! window is hidden (close-to-tray) or while the user is in another app:
//!   * Show / Hide the main window
//!   * Toggle recording (state owned by the frontend; tray just emits)
//!   * Recent exports submenu (top 5 by mtime, opens File Explorer / Finder)
//!   * Check for updates (frontend listens, runs the updater check)
//!   * Quit Recast (the only way to actually exit when close-to-tray is on)
//!
//! The menu is rebuilt — not mutated — when state changes (recent exports
//! list, recording state). Tauri v2's `Menu` is immutable after creation;
//! `TrayIcon::set_menu` swaps the whole tree, which keeps the code simple
//! and avoids bookkeeping per-item handles. The cost is negligible: rebuild
//! happens on rare events (recording start/stop, export complete).

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

use tauri::{
    menu::{IsMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, Wry,
};

use crate::commands::system::get_active_output_dir;
use crate::commands::types::AppState;

/// Frontend-owned recording state, mirrored here so menu rebuilds triggered
/// by Rust-only code paths (window toggle, close-to-tray hide) can pick the
/// right label without round-tripping to JS. The frontend calls
/// `refresh_tray(is_recording)` on every state transition.
static IS_RECORDING: AtomicBool = AtomicBool::new(false);

/// Read the mirrored recording state. Used by the file-association path to
/// refuse opening a `.recast` while capture is in flight (editor windows
/// spawn FFmpeg thumbnail probes that compete with the recording pipeline).
pub fn is_recording_active() -> bool {
    IS_RECORDING.load(Ordering::Relaxed)
}

const TRAY_ID: &str = "recast.main";
const MENU_ID_SHOW_HIDE: &str = "tray.show_hide";
const MENU_ID_RECORD_TOGGLE: &str = "tray.record_toggle";
const MENU_ID_PAUSE_TOGGLE: &str = "tray.pause_toggle";
const MENU_ID_CHECK_UPDATES: &str = "tray.check_updates";
const MENU_ID_QUIT: &str = "tray.quit";
const MENU_ID_RECENT_PREFIX: &str = "tray.recent:";

/// One row in the Recent Exports submenu. Lightweight — we only need the
/// path (for the reveal action) and a display label.
struct RecentExport {
    path: String,
    label: String,
}

/// Build the tray once at app startup. Subsequent state changes call
/// `rebuild_menu` to swap the menu tree.
pub fn init(app: &AppHandle) -> tauri::Result<()> {
    let menu = build_menu(app)?;

    // The default icon is configured in tauri.conf.json so this should always
    // be present — but a missing icon is a recoverable degradation (the app
    // works without a tray), not a reason to abort startup. Skip the tray and
    // let the caller log it.
    let Some(icon) = app.default_window_icon().cloned() else {
        log::warn!("no default window icon; skipping system tray");
        return Ok(());
    };

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon)
        .tooltip("Recast")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(handle_menu_event)
        .on_tray_icon_event(handle_tray_icon_event)
        .build(app)?;

    Ok(())
}

/// Rebuild + swap the tray menu. Reads the current `IS_RECORDING` flag and
/// window visibility to label the Show/Hide and Start/Stop items.
pub fn rebuild_menu(app: &AppHandle) {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        if let Ok(menu) = build_menu(app) {
            let _ = tray.set_menu(Some(menu));
        }
        let tooltip = if IS_RECORDING.load(Ordering::Relaxed) {
            "Recast (Recording)"
        } else {
            "Recast"
        };
        let _ = tray.set_tooltip(Some(tooltip));
    }

    // Keep the taskbar jump list's Recent Projects in sync with new recordings.
    #[cfg(windows)]
    crate::jumplist::update(app);
}

fn build_menu(app: &AppHandle) -> tauri::Result<Menu<Wry>> {
    let show_hide_label = match main_window_visible(app) {
        Some(true) => "Hide Recast",
        _ => "Show Recast",
    };
    let is_recording = IS_RECORDING.load(Ordering::Relaxed);
    let record_label = if is_recording {
        "Stop Recording"
    } else {
        "Start Recording"
    };

    let show_hide = MenuItem::with_id(app, MENU_ID_SHOW_HIDE, show_hide_label, true, None::<&str>)?;
    let record_toggle =
        MenuItem::with_id(app, MENU_ID_RECORD_TOGGLE, record_label, true, None::<&str>)?;

    // Pause/Resume is only meaningful mid-recording, so it appears then. The
    // label reflects the manager's live pause state.
    let pause_toggle = if is_recording {
        let paused = app
            .try_state::<AppState>()
            .map(|s| s.recording_manager.is_paused())
            .unwrap_or(false);
        let label = if paused {
            "Resume Recording"
        } else {
            "Pause Recording"
        };
        Some(MenuItem::with_id(
            app,
            MENU_ID_PAUSE_TOGGLE,
            label,
            true,
            None::<&str>,
        )?)
    } else {
        None
    };
    let check_updates = MenuItem::with_id(
        app,
        MENU_ID_CHECK_UPDATES,
        "Check for Updates…",
        true,
        None::<&str>,
    )?;
    let quit = MenuItem::with_id(app, MENU_ID_QUIT, "Quit Recast", true, None::<&str>)?;

    let recents = recent_exports(app, 5);
    let recent_submenu = build_recent_submenu(app, &recents)?;

    let sep1 = PredefinedMenuItem::separator(app)?;
    let sep2 = PredefinedMenuItem::separator(app)?;
    let sep3 = PredefinedMenuItem::separator(app)?;
    let sep4 = PredefinedMenuItem::separator(app)?;

    let mut items: Vec<&dyn IsMenuItem<Wry>> = vec![&show_hide, &sep1, &record_toggle];
    if let Some(ref pause) = pause_toggle {
        items.push(pause);
    }
    items.extend_from_slice(&[&sep2, &recent_submenu, &check_updates, &sep3, &quit, &sep4]);

    Menu::with_items(app, &items)
}

fn build_recent_submenu(app: &AppHandle, recents: &[RecentExport]) -> tauri::Result<Submenu<Wry>> {
    if recents.is_empty() {
        // Empty submenu — render a disabled "(No exports yet)" placeholder so
        // the menu doesn't look broken. A submenu with zero children would
        // collapse to an unclickable header on some platforms.
        let placeholder = MenuItem::with_id(
            app,
            "tray.recent.empty",
            "(No exports yet)",
            false,
            None::<&str>,
        )?;
        return Submenu::with_items(app, "Recent Exports", true, &[&placeholder]);
    }

    let mut items: Vec<MenuItem<Wry>> = Vec::with_capacity(recents.len());
    for entry in recents {
        let id = format!("{MENU_ID_RECENT_PREFIX}{}", entry.path);
        items.push(MenuItem::with_id(
            app,
            &id,
            &entry.label,
            true,
            None::<&str>,
        )?);
    }
    let refs: Vec<&dyn tauri::menu::IsMenuItem<Wry>> = items
        .iter()
        .map(|m| m as &dyn tauri::menu::IsMenuItem<Wry>)
        .collect();
    Submenu::with_items(app, "Recent Exports", true, &refs)
}

/// Top-N most recent exports by mtime under `<output_dir>/exports/`. Mirrors
/// the extension filter used by `commands::list_exports` so the tray submenu
/// stays in sync with the in-app list.
fn recent_exports(app: &AppHandle, limit: usize) -> Vec<RecentExport> {
    let state: tauri::State<'_, AppState> = match app.try_state::<AppState>() {
        Some(s) => s,
        None => return Vec::new(),
    };
    let exports_dir = get_active_output_dir(&state).join("exports");
    let entries = match fs::read_dir(&exports_dir) {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };

    let mut rows: Vec<(u64, PathBuf, String)> = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let ext = path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        if !matches!(ext.as_str(), "mp4" | "webm" | "gif") {
            continue;
        }
        let mtime = entry
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let name = entry.file_name().to_string_lossy().to_string();
        rows.push((mtime, path, name));
    }
    rows.sort_by(|a, b| b.0.cmp(&a.0));
    rows.into_iter()
        .take(limit)
        .map(|(_, path, name)| RecentExport {
            path: path.to_string_lossy().to_string(),
            label: name,
        })
        .collect()
}

fn handle_menu_event(app: &AppHandle, event: MenuEvent) {
    let id = event.id().as_ref();
    match id {
        MENU_ID_SHOW_HIDE => toggle_main_window(app),
        MENU_ID_RECORD_TOGGLE => {
            let _ = app.emit("tray:record-toggle", ());
        }
        MENU_ID_PAUSE_TOGGLE => {
            let _ = app.emit("tray:pause-toggle", ());
        }
        MENU_ID_CHECK_UPDATES => {
            // Surface the window first so the corner card the frontend
            // surfaces is actually visible. Cheap on the happy path —
            // `show()` is a no-op when already visible.
            show_main_window(app);
            let _ = app.emit("updater:check-from-tray", ());
        }
        MENU_ID_QUIT => {
            app.exit(0);
        }
        other if other.starts_with(MENU_ID_RECENT_PREFIX) => {
            let path = other[MENU_ID_RECENT_PREFIX.len()..].to_string();
            let _ = crate::commands::system::open_file_location(path);
        }
        _ => {}
    }
}

fn handle_tray_icon_event(tray: &tauri::tray::TrayIcon, event: TrayIconEvent) {
    // Left-click toggles the main window on Windows/Linux. macOS opens the
    // menu on left-click natively (set `show_menu_on_left_click(true)` if we
    // ever want explicit macOS-style click-to-open-menu), and tray crates
    // pass the same Click event through. The toggle is harmless even there.
    if let TrayIconEvent::Click {
        button: MouseButton::Left,
        button_state: MouseButtonState::Up,
        ..
    } = event
    {
        toggle_main_window(tray.app_handle());
    }
}

fn main_window(app: &AppHandle) -> Option<tauri::WebviewWindow> {
    app.get_webview_window("main")
}

fn main_window_visible(app: &AppHandle) -> Option<bool> {
    main_window(app).and_then(|w| w.is_visible().ok())
}

fn show_main_window(app: &AppHandle) {
    if let Some(window) = main_window(app) {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

fn toggle_main_window(app: &AppHandle) {
    let Some(window) = main_window(app) else {
        return;
    };
    let visible = window.is_visible().unwrap_or(false);
    if visible {
        let _ = window.hide();
    } else {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
    // Refresh the menu so the Show/Hide label matches the new state.
    // Recording state is read from IS_RECORDING — last value the frontend
    // pushed via refresh_tray.
    rebuild_menu(app);
}

/// Tauri command — lets the frontend trigger a tray rebuild after state
/// changes the Rust side can't observe directly:
///   * recording start/stop (frontend passes `is_recording=Some(...)`)
///   * fresh export landed (frontend passes `is_recording=None`; we leave
///     the cached recording flag alone and just rebuild for the new file list)
#[tauri::command]
pub fn refresh_tray(app: AppHandle, is_recording: Option<bool>) {
    if let Some(value) = is_recording {
        IS_RECORDING.store(value, Ordering::Relaxed);
    }
    rebuild_menu(&app);
}
