//! Windows taskbar Jump List: a "New Recording" task plus a Recent Projects
//! category, reached by right-clicking the taskbar or Start icon. Tauri has no
//! API for this, so it is built directly on the Shell COM interfaces.
//!
//! "New Recording" relaunches the exe with `--new-recording` (single-instance
//! forwards it to the running app). Recent items relaunch the exe with a
//! `.recast` path, which the existing file-association argv path opens.

use std::path::PathBuf;

use tauri::{AppHandle, Manager};
use windows::core::{Interface, GUID, HSTRING, PROPVARIANT};
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER};
use windows::Win32::UI::Shell::Common::{IObjectArray, IObjectCollection};
use windows::Win32::UI::Shell::PropertiesSystem::{IPropertyStore, PROPERTYKEY};
use windows::Win32::UI::Shell::{
    DestinationList, EnumerableObjectCollection, ICustomDestinationList, IShellLinkW, ShellLink,
};

use crate::commands::system::get_active_output_dir;
use crate::commands::types::AppState;

/// PKEY_Title: {F29F85E0-4FF9-1068-AB91-08002B27B3D9}, pid 2. The display name
/// shown for a jump-list link.
const PKEY_TITLE: PROPERTYKEY = PROPERTYKEY {
    fmtid: GUID::from_u128(0xF29F85E0_4FF9_1068_AB91_08002B27B3D9),
    pid: 2,
};

/// Rebuild the jump list. Non-fatal on any COM error.
pub fn update(app: &AppHandle) {
    if let Err(e) = build(app) {
        log::warn!("jump list update failed: {e}");
    }
}

fn build(app: &AppHandle) -> windows::core::Result<()> {
    let exe = std::env::current_exe().unwrap_or_default();
    let exe = exe.to_string_lossy().to_string();
    let recents = recent_recasts(app, 6);

    unsafe {
        let list: ICustomDestinationList =
            CoCreateInstance(&DestinationList, None, CLSCTX_INPROC_SERVER)?;
        let mut slots: u32 = 0;
        let _removed: IObjectArray = list.BeginList(&mut slots)?;

        let tasks: IObjectCollection =
            CoCreateInstance(&EnumerableObjectCollection, None, CLSCTX_INPROC_SERVER)?;
        let new_recording = shell_link(&exe, "--new-recording", "New Recording", &exe)?;
        tasks.AddObject(&new_recording)?;
        list.AddUserTasks(&tasks.cast::<IObjectArray>()?)?;

        if !recents.is_empty() {
            let coll: IObjectCollection =
                CoCreateInstance(&EnumerableObjectCollection, None, CLSCTX_INPROC_SERVER)?;
            for (path, label) in &recents {
                let link = shell_link(&exe, &format!("\"{path}\""), label, &exe)?;
                coll.AddObject(&link)?;
            }
            list.AppendCategory(
                &HSTRING::from("Recent Projects"),
                &coll.cast::<IObjectArray>()?,
            )?;
        }

        list.CommitList()?;
    }
    Ok(())
}

unsafe fn shell_link(
    path: &str,
    args: &str,
    title: &str,
    icon: &str,
) -> windows::core::Result<IShellLinkW> {
    let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)?;
    link.SetPath(&HSTRING::from(path))?;
    link.SetArguments(&HSTRING::from(args))?;
    link.SetIconLocation(&HSTRING::from(icon), 0)?;
    let store: IPropertyStore = link.cast()?;
    store.SetValue(&PKEY_TITLE, &PROPVARIANT::from(title))?;
    store.Commit()?;
    Ok(link)
}

fn recent_recasts(app: &AppHandle, limit: usize) -> Vec<(String, String)> {
    let Some(state) = app.try_state::<AppState>() else {
        return Vec::new();
    };
    let dir = get_active_output_dir(&state).join("recasts");
    let Ok(entries) = std::fs::read_dir(&dir) else {
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
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
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
