//! Register (or remove) the `recast` binary on the user's PATH so the CLI is
//! invocable as a bare `recast` command.
//!
//! OS-agnostic surface, per-OS mechanics:
//! - Windows: append the exe's folder to the user PATH env var (HKCU\Environment),
//!   written as REG_EXPAND_SZ so existing `%VAR%` entries keep expanding, then
//!   broadcast the change. No admin, no NSIS string-length footgun.
//! - macOS/Linux: symlink the binary into `~/.local/bin` (user-writable, no sudo).
//!
//! Shared by the `recast install`/`uninstall` CLI verbs and the in-app action.

use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallStatus {
    /// Whether `recast` currently resolves from a terminal.
    pub on_path: bool,
    /// The directory the CLI is (or would be) reachable from.
    pub bin_dir: String,
    /// Human-readable summary.
    pub detail: String,
}

pub fn status() -> InstallStatus {
    platform::status()
}

pub fn install() -> Result<String, String> {
    platform::install()
}

pub fn uninstall() -> Result<String, String> {
    platform::uninstall()
}

fn exe_dir() -> Result<std::path::PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    exe.parent()
        .map(|p| p.to_path_buf())
        .ok_or_else(|| "could not resolve executable directory".to_string())
}

#[cfg(windows)]
mod platform {
    use super::*;
    use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_WRITE, REG_EXPAND_SZ};
    use winreg::{RegKey, RegValue};

    fn dir_string() -> Result<String, String> {
        Ok(exe_dir()?.to_string_lossy().to_string())
    }

    fn open_env() -> Result<RegKey, String> {
        RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey_with_flags("Environment", KEY_READ | KEY_WRITE)
            .map_err(|e| e.to_string())
    }

    fn read_path(env: &RegKey) -> String {
        env.get_value("Path").unwrap_or_default()
    }

    fn contains(path: &str, dir: &str) -> bool {
        path.split(';').any(|p| p.trim().eq_ignore_ascii_case(dir))
    }

    /// Write PATH back as REG_EXPAND_SZ (UTF-16LE, null-terminated) so any
    /// existing `%USERPROFILE%`-style entries keep expanding.
    fn write_path(env: &RegKey, value: &str) -> Result<(), String> {
        let mut bytes: Vec<u8> = value
            .encode_utf16()
            .chain(std::iter::once(0))
            .flat_map(|u| u.to_le_bytes())
            .collect();
        // Ensure trailing UTF-16 null even for an empty value.
        if bytes.is_empty() {
            bytes.extend_from_slice(&[0, 0]);
        }
        env.set_raw_value(
            "Path",
            &RegValue {
                vtype: REG_EXPAND_SZ,
                bytes,
            },
        )
        .map_err(|e| e.to_string())
    }

    pub fn status() -> InstallStatus {
        let dir = dir_string().unwrap_or_default();
        let in_process = std::env::var("PATH")
            .unwrap_or_default()
            .split(';')
            .any(|p| p.trim().eq_ignore_ascii_case(&dir));
        let in_registry = open_env().map(|e| contains(&read_path(&e), &dir)).unwrap_or(false);
        let on_path = in_process || in_registry;
        InstallStatus {
            detail: if on_path {
                "recast is on your PATH".into()
            } else {
                "recast is not on your PATH".into()
            },
            on_path,
            bin_dir: dir,
        }
    }

    pub fn install() -> Result<String, String> {
        let dir = dir_string()?;
        let env = open_env()?;
        let current = read_path(&env);
        if contains(&current, &dir) {
            return Ok(format!("`recast` is already on your PATH ({dir})."));
        }
        let next = if current.trim().is_empty() {
            dir.clone()
        } else {
            format!("{};{}", current.trim_end_matches(';'), dir)
        };
        write_path(&env, &next)?;
        broadcast();
        Ok(format!(
            "Added `recast` to your PATH ({dir}). Open a new terminal to use it."
        ))
    }

    pub fn uninstall() -> Result<String, String> {
        let dir = dir_string()?;
        let env = open_env()?;
        let current = read_path(&env);
        if !contains(&current, &dir) {
            return Ok("`recast` was not on your PATH.".into());
        }
        let next: Vec<&str> = current
            .split(';')
            .filter(|p| !p.trim().is_empty() && !p.trim().eq_ignore_ascii_case(&dir))
            .collect();
        write_path(&env, &next.join(";"))?;
        broadcast();
        Ok(format!("Removed `recast` from your PATH ({dir})."))
    }

    /// Tell running shells/Explorer that the environment changed so new
    /// terminals pick up the PATH without a logout.
    fn broadcast() {
        use windows::core::PCWSTR;
        use windows::Win32::Foundation::{LPARAM, WPARAM};
        use windows::Win32::UI::WindowsAndMessaging::{
            SendMessageTimeoutW, HWND_BROADCAST, SMTO_ABORTIFHUNG, WM_SETTINGCHANGE,
        };
        let env: Vec<u16> = "Environment\0".encode_utf16().collect();
        unsafe {
            let _ = SendMessageTimeoutW(
                HWND_BROADCAST,
                WM_SETTINGCHANGE,
                WPARAM(0),
                LPARAM(PCWSTR(env.as_ptr()).as_ptr() as isize),
                SMTO_ABORTIFHUNG,
                5000,
                None,
            );
        }
    }
}

#[cfg(unix)]
mod platform {
    use super::*;
    use std::path::PathBuf;

    fn home() -> PathBuf {
        std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default()
    }

    fn bin_dir() -> PathBuf {
        home().join(".local").join("bin")
    }

    fn link_path() -> PathBuf {
        bin_dir().join("recast")
    }

    fn dir_on_path(dir: &std::path::Path) -> bool {
        std::env::var("PATH")
            .unwrap_or_default()
            .split(':')
            .any(|p| PathBuf::from(p) == dir)
    }

    pub fn status() -> InstallStatus {
        let dir = bin_dir();
        let on_path = link_path().exists() && dir_on_path(&dir);
        InstallStatus {
            detail: if on_path {
                "recast is on your PATH".into()
            } else {
                "recast is not on your PATH".into()
            },
            on_path,
            bin_dir: dir.to_string_lossy().to_string(),
        }
    }

    pub fn install() -> Result<String, String> {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let dir = bin_dir();
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let link = link_path();
        if link.exists() || link.symlink_metadata().is_ok() {
            let _ = std::fs::remove_file(&link);
        }
        std::os::unix::fs::symlink(&exe, &link).map_err(|e| e.to_string())?;
        if dir_on_path(&dir) {
            Ok(format!("Linked `recast` into {}.", dir.display()))
        } else {
            Ok(format!(
                "Linked `recast` into {}, but that folder is not on your PATH. Add it to your shell profile: export PATH=\"$HOME/.local/bin:$PATH\"",
                dir.display()
            ))
        }
    }

    pub fn uninstall() -> Result<String, String> {
        let link = link_path();
        if link.symlink_metadata().is_ok() {
            std::fs::remove_file(&link).map_err(|e| e.to_string())?;
            Ok(format!("Removed {}.", link.display()))
        } else {
            Ok("`recast` link was not present.".into())
        }
    }
}
