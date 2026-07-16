//! System tray icon, menu, and event wiring.
//!
//! The tray is the canonical entry-point for quick actions while the main
//! window is hidden (close-to-tray) or while the user is in another app.
//! Items are grouped by frequency and reversibility:
//!
//!   1. **Status** (disabled) — `○ Ready` / `● Recording` / `⏸ Paused`. Gives a
//!      glanceable state without opening the menu contents.
//!   2. **Recording control** — toggle + (Pause/Resume if live). Mutually
//!      exclusive based on `IS_RECORDING`.
//!   3. **Output access** — "Open Output Folder" + Recent Exports +
//!      Recent Projects. The two recents are separate because users want
//!      them as different actions (re-share an export vs. resume editing a
//!      project) and the OS shows them differently.
//!   4. **Window toggle** — Show/Hide Recast.
//!   5. **App maintenance** — Check for Updates, About, Quit (destructive last).
//!
//! The menu is rebuilt, not mutated, on every state change. Tauri v2's
//! `Menu` is immutable post-creation; `TrayIcon::set_menu` swaps the whole
//! tree. The cost is negligible: rebuild happens only on real transitions
//! (recording start/stop/pause, export complete). The frontend pushes state
//! via [`refresh_tray`] (Tauri command) so the Rust side never reads UI
//! state directly.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::UNIX_EPOCH;

use tauri::{
    menu::{IsMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, Wry,
};
use tauri_plugin_opener::OpenerExt;

use crate::commands::system::get_active_output_dir;
use crate::commands::types::AppState;

const TRAY_ID: &str = "recast.main";

const MENU_ID_STATUS: &str = "tray.status";
const MENU_ID_SHOW_HIDE: &str = "tray.show_hide";
const MENU_ID_RECORD_TOGGLE: &str = "tray.record_toggle";
const MENU_ID_PAUSE_TOGGLE: &str = "tray.pause_toggle";
const MENU_ID_OPEN_OUTPUT: &str = "tray.open_output_folder";
const MENU_ID_CHECK_UPDATES: &str = "tray.check_updates";
const MENU_ID_QUIT: &str = "tray.quit";
const MENU_ID_ABOUT_DOCS: &str = "tray.about.docs";
const MENU_ID_ABOUT_GITHUB: &str = "tray.about.github";
const MENU_ID_RECENT_EXPORTS_PREFIX: &str = "tray.recent_export:";
const MENU_ID_RECENT_PROJECTS_PREFIX: &str = "tray.recent_project:";

/// Frontend-mirrored recording state, similar to the prior `AtomicBool`.
/// `STARTED_AT_MS` is informational — populated by the frontend when it
/// triggers `start_recording`; the menu ignores it for now (kept on the
/// struct so a future live-timer feature doesn't require another IPC change).
static IS_RECORDING: AtomicBool = AtomicBool::new(false);
static STARTED_AT_MS: AtomicU64 = AtomicU64::new(0);

pub fn is_recording_active() -> bool {
    IS_RECORDING.load(Ordering::Relaxed)
}

/// Build the tray once at app startup. Subsequent state changes call
/// `rebuild_menu` to swap the menu tree.
pub fn init(app: &AppHandle) -> tauri::Result<()> {
    let menu = build_menu(app)?;

    // The default icon is configured in tauri.conf.json so this should always
    // be present — but a missing icon is a recoverable degradation (the app
    // works without a tray), not a reason to abort startup.
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

/// Rebuild + swap the tray menu. Reads the mirrored recording flag and
/// window visibility to label items. Safe to call from any thread.
pub fn rebuild_menu(app: &AppHandle) {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        if let Ok(menu) = build_menu(app) {
            let _ = tray.set_menu(Some(menu));
        }
        let tooltip = if IS_RECORDING.load(Ordering::Relaxed) {
            "Recast — Recording"
        } else {
            "Recast"
        };
        let _ = tray.set_tooltip(Some(tooltip));
    }
    #[cfg(windows)]
    crate::jumplist::update(app);
}

fn status_label(is_recording: bool, is_paused: bool) -> &'static str {
    match (is_recording, is_paused) {
        (false, _) => "○  Ready",
        (true, true) => "⏸  Paused",
        (true, false) => "●  Recording",
    }
}

fn build_menu(app: &AppHandle) -> tauri::Result<Menu<Wry>> {
    let is_recording = IS_RECORDING.load(Ordering::Relaxed);
    let is_paused = app
        .try_state::<AppState>()
        .map(|s| s.recording_manager.is_paused())
        .unwrap_or(false);

    let show_hide_label = match main_window_visible(app) {
        Some(true) => "Hide Recast",
        _ => "Show Recast",
    };

    // Group 1: status (disabled — read-only).
    let status = MenuItem::with_id(
        app,
        MENU_ID_STATUS,
        status_label(is_recording, is_paused),
        false,
        None::<&str>,
    )?;

    // Group 2: recording control. Mutually exclusive label.
    let record_label = if is_recording {
        "Stop Recording"
    } else {
        "Start Recording"
    };
    let record_toggle =
        MenuItem::with_id(app, MENU_ID_RECORD_TOGGLE, record_label, true, None::<&str>)?;
    let pause_toggle = if is_recording {
        let label = if is_paused {
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

    // Group 3: output access.
    let open_output = MenuItem::with_id(
        app,
        MENU_ID_OPEN_OUTPUT,
        "Open Output Folder",
        true,
        None::<&str>,
    )?;
    let recent_exports = build_recent_exports_submenu(app)?;
    let recent_projects = build_recent_projects_submenu(app)?;

    // Group 4: window toggle.
    let show_hide = MenuItem::with_id(app, MENU_ID_SHOW_HIDE, show_hide_label, true, None::<&str>)?;

    // Group 5: app maintenance.
    let check_updates = MenuItem::with_id(
        app,
        MENU_ID_CHECK_UPDATES,
        "Check for Updates…",
        true,
        None::<&str>,
    )?;
    let about = build_about_submenu(app)?;
    let quit = MenuItem::with_id(app, MENU_ID_QUIT, "Quit Recast", true, None::<&str>)?;

    let sep1 = PredefinedMenuItem::separator(app)?;
    let sep2 = PredefinedMenuItem::separator(app)?;
    let sep3 = PredefinedMenuItem::separator(app)?;
    let sep4 = PredefinedMenuItem::separator(app)?;
    let sep5 = PredefinedMenuItem::separator(app)?;

    let mut items: Vec<&dyn IsMenuItem<Wry>> = Vec::new();
    items.push(&status);
    items.push(&sep1);
    if is_recording {
        items.push(&record_toggle);
        if let Some(ref p) = pause_toggle {
            items.push(p);
        }
    } else {
        items.push(&record_toggle);
    }
    items.push(&sep2);
    items.push(&open_output);
    items.push(&recent_exports);
    items.push(&recent_projects);
    items.push(&sep3);
    items.push(&show_hide);
    items.push(&sep4);
    items.push(&check_updates);
    items.push(&about);
    items.push(&sep5);
    items.push(&quit);

    Menu::with_items(app, &items)
}

fn build_recent_exports_submenu(app: &AppHandle) -> tauri::Result<Submenu<Wry>> {
    let recents = recent_exports(app, 5);
    if recents.is_empty() {
        let placeholder = MenuItem::with_id(
            app,
            "tray.recent_exports.empty",
            "(No exports yet)",
            false,
            None::<&str>,
        )?;
        return Submenu::with_items(app, "Recent Exports", true, &[&placeholder]);
    }
    let mut items: Vec<MenuItem<Wry>> = Vec::with_capacity(recents.len());
    for (path, label) in recents {
        let id = format!("{MENU_ID_RECENT_EXPORTS_PREFIX}{path}");
        items.push(MenuItem::with_id(app, &id, &label, true, None::<&str>)?);
    }
    let refs: Vec<&dyn IsMenuItem<Wry>> = items.iter().map(|m| m as &dyn IsMenuItem<Wry>).collect();
    Submenu::with_items(app, "Recent Exports", true, &refs)
}

fn build_recent_projects_submenu(app: &AppHandle) -> tauri::Result<Submenu<Wry>> {
    let recents = recent_projects(app, 5);
    if recents.is_empty() {
        let placeholder = MenuItem::with_id(
            app,
            "tray.recent_projects.empty",
            "(No projects yet)",
            false,
            None::<&str>,
        )?;
        return Submenu::with_items(app, "Recent Projects", true, &[&placeholder]);
    }
    let mut items: Vec<MenuItem<Wry>> = Vec::with_capacity(recents.len());
    for (path, label) in recents {
        let id = format!("{MENU_ID_RECENT_PROJECTS_PREFIX}{path}");
        items.push(MenuItem::with_id(app, &id, &label, true, None::<&str>)?);
    }
    let refs: Vec<&dyn IsMenuItem<Wry>> = items.iter().map(|m| m as &dyn IsMenuItem<Wry>).collect();
    Submenu::with_items(app, "Recent Projects", true, &refs)
}

fn build_about_submenu(app: &AppHandle) -> tauri::Result<Submenu<Wry>> {
    let version_label = format!("Version {}", env!("CARGO_PKG_VERSION"));
    let version = MenuItem::with_id(
        app,
        "tray.about.version",
        version_label,
        false, // disabled — informational only
        None::<&str>,
    )?;
    let docs = MenuItem::with_id(app, MENU_ID_ABOUT_DOCS, "Documentation", true, None::<&str>)?;
    let github = MenuItem::with_id(
        app,
        MENU_ID_ABOUT_GITHUB,
        "Open on GitHub",
        true,
        None::<&str>,
    )?;
    Submenu::with_items(app, "About Recast", true, &[&version, &docs, &github])
}

/// Top-N most recent exports by mtime under `<output_dir>/exports/`. Mirrors
/// the extension filter used by `commands::list_exports` so the tray submenu
/// stays in sync with the in-app list.
fn recent_exports(app: &AppHandle, limit: usize) -> Vec<(String, String)> {
    let Some(state) = app.try_state::<AppState>() else {
        return Vec::new();
    };
    let exports_dir = get_active_output_dir(&state).join("exports");
    let Ok(entries) = fs::read_dir(&exports_dir) else {
        return Vec::new();
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
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let name = entry.file_name().to_string_lossy().to_string();
        rows.push((mtime, path, name));
    }
    rows.sort_by(|a, b| b.0.cmp(&a.0));
    rows.into_iter()
        .take(limit)
        .map(|(_, path, name)| (path.to_string_lossy().to_string(), name))
        .collect()
}

/// Top-N most recent `.recast` projects by mtime under `<output_dir>/recasts/`.
/// Mirrors `jumplist::recent_recasts` so the tray and Windows Jump List show
/// the same items. Duplicate filtering is per-source (not cross-source), so
/// the two UIs might disagree if a project gets rewritten faster than the
/// tray rebuild cadence — acceptable for a sync every few seconds.
fn recent_projects(app: &AppHandle, limit: usize) -> Vec<(String, String)> {
    let Some(state) = app.try_state::<AppState>() else {
        return Vec::new();
    };
    let projects_dir = get_active_output_dir(&state).join("recasts");
    let Ok(entries) = fs::read_dir(&projects_dir) else {
        return Vec::new();
    };

    let mut rows: Vec<(u64, PathBuf, String)> = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let is_recast = path
            .extension()
            .and_then(|s| s.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("recast"));
        if !is_recast {
            continue;
        }
        let mtime = entry
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let label = path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        rows.push((mtime, path, label));
    }
    rows.sort_by(|a, b| b.0.cmp(&a.0));
    rows.into_iter()
        .take(limit)
        .map(|(_, path, label)| (path.to_string_lossy().to_string(), label))
        .collect()
}

/// Open the active output directory in the OS file manager. Same code path
/// as `commands::open_log_dir` — uses `tauri-plugin-opener`'s `open_path`
/// which routes to the right shell verb on each OS (`open` on macOS,
/// `explorer.exe` on Windows, `xdg-open` on Linux).
fn open_output_folder(app: &AppHandle) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let dir = get_active_output_dir(&state);
    let _ = std::fs::create_dir_all(&dir);
    let display = dir.to_string_lossy().to_string();
    if let Err(e) = app.opener().open_path(display, None::<&str>) {
        log::warn!("open output folder failed: {e}");
    }
}

fn handle_menu_event(app: &AppHandle, event: MenuEvent) {
    let id = event.id().as_ref();
    match id {
        MENU_ID_STATUS => {} // disabled — consume only
        MENU_ID_SHOW_HIDE => toggle_main_window(app),
        MENU_ID_RECORD_TOGGLE => {
            let _ = app.emit("tray:record-toggle", ());
        }
        MENU_ID_PAUSE_TOGGLE => {
            let _ = app.emit("tray:pause-toggle", ());
        }
        MENU_ID_OPEN_OUTPUT => open_output_folder(app),
        MENU_ID_CHECK_UPDATES => {
            // Surface the window first so the corner card the frontend
            // surfaces is actually visible.
            show_main_window(app);
            let _ = app.emit("updater:check-from-tray", ());
        }
        MENU_ID_ABOUT_DOCS => {
            if let Err(e) = app.opener().open_url(
                "https://github.com/kanakkholwal/recast/blob/main/apps/desktop/docs",
                None::<&str>,
            ) {
                log::warn!("open docs failed: {e}");
            }
        }
        MENU_ID_ABOUT_GITHUB => {
            if let Err(e) = app
                .opener()
                .open_url("https://github.com/kanakkholwal/recast", None::<&str>)
            {
                log::warn!("open github failed: {e}");
            }
        }
        MENU_ID_QUIT => {
            app.exit(0);
        }
        other if other.starts_with(MENU_ID_RECENT_EXPORTS_PREFIX) => {
            let path = other[MENU_ID_RECENT_EXPORTS_PREFIX.len()..].to_string();
            // Reveal in file manager. `open_file_location` is async; the
            // handler is sync, so spawn it rather than dropping the future
            // unrun (which silently did nothing).
            tauri::async_runtime::spawn(async move {
                let _ = crate::commands::system::open_file_location(path).await;
            });
        }
        other if other.starts_with(MENU_ID_RECENT_PROJECTS_PREFIX) => {
            let path = other[MENU_ID_RECENT_PROJECTS_PREFIX.len()..].to_string();
            // Open (not reveal) — the `.recast` file-association routes through
            // single-instance back to the running window. `open_path` delegates
            // to the OS's default handler for the extension.
            if let Err(e) = app.opener().open_path(&path, None::<&str>) {
                log::warn!("open project failed: {e}");
            }
        }
        _ => {}
    }
}

fn handle_tray_icon_event(tray: &tauri::tray::TrayIcon, event: TrayIconEvent) {
    // Left-click toggles the main window on Windows/Linux. macOS opens the
    // menu on left-click natively (set `show_menu_on_left_click(true)` if we
    // ever want explicit macOS-style click-to-open-menu), and the tray crate
    // passes the same Click event through. The toggle is harmless there.
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
    rebuild_menu(app);
}

/// Tauri command — lets the frontend trigger a tray rebuild after state
/// changes the Rust side can't observe directly:
///   * recording start/stop (frontend passes `is_recording=Some(...)`,
///     optionally `started_at_ms=Some(...)` for a future live-timer feature)
///   * fresh export / project landed (frontend passes `is_recording=None`;
///     we leave the cached recording flag alone and just rebuild for the new
///     file list)
///
/// All three parameters are optional; `None` means "leave whatever's there".
#[tauri::command]
pub fn refresh_tray(app: AppHandle, is_recording: Option<bool>, started_at_ms: Option<u64>) {
    if let Some(value) = is_recording {
        IS_RECORDING.store(value, Ordering::Relaxed);
    }
    if let Some(ms) = started_at_ms {
        STARTED_AT_MS.store(ms, Ordering::Relaxed);
    }
    rebuild_menu(&app);
}
