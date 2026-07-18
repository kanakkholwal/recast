//! Register (or remove) the `recast` binary on the user's PATH so the CLI is
//! invocable as a bare `recast` command.
//!
//! OS-agnostic surface, per-OS mechanics:
//! - **Windows**: copy the binary to `%LOCALAPPDATA%\com.kanakkholwal.recast\bin\recast.exe`
//!   (stable per-user install — survives dev rebuilds), then add that
//!   folder to the user PATH env var (`HKCU\Environment\Path`) and broadcast
//!   `WM_SETTINGCHANGE`. `uninstall` deletes the copy AND removes the
//!   registry entry, so the CLI is fully gone.
//! - **macOS / Linux**: copy the binary to
//!   `~/.local/share/com.kanakkholwal.recast/bin/recast` (per-user, no
//!   sudo) and symlink `~/.local/bin/recast` to it. Shell PATH is updated
//!   via a guarded block on `~/.zprofile`/`~/.zshrc` (macOS) or
//!   `~/.bashrc`/`~/.zshrc`/`~/.profile`/`~/.bash_profile` (Linux).
//!   `uninstall` deletes both the copy and the symlink, and reverts the
//!   rc blocks.
//!
//! **Why not symlink the dev binary directly?** In dev mode the binary
//! lives at `<repo>/apps/desktop/src-tauri/target/debug/recast` and
//! gets replaced on every `cargo tauri dev` rebuild. Symlinking to that
//! path leaves `recast` resolvable even after `uninstall`, because the
//! dev binary (and any unrelated PATH entry to its parent dir) persists
//! independently of our installer. Copying to a stable, recast-owned
//! directory makes uninstall fully effective.
//!
//! Shared by the `recast install`/`uninstall` CLI verbs, the in-app
//! settings panel, and the first-launch auto-install hook in
//! `lib.rs::run`.

use serde::Serialize;
#[cfg(unix)]
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

// Marker constants are kept module-level (not `cfg(unix)`) so the unit
// tests can locate the block regardless of host platform — but the only
// production user is the Unix install path.
#[allow(dead_code)]
const BEGIN_MARKER: &str = "# >>> recast cli >>>";
#[allow(dead_code)]
const END_MARKER: &str = "# <<< recast cli <<<";

/// The single PATH line the installer appends. Idempotent inside each file:
/// between the begin/end markers there is at most one occurrence, and we
/// re-write the block on every install rather than appending duplicates.
#[cfg(unix)]
const PATH_BLOCK_BODY: &str = "export PATH=\"$HOME/.local/bin:$PATH\"";

#[cfg(unix)]
const PATH_BLOCK: &str = "\
# >>> recast cli >>>
# Added by Recast so the `recast` CLI resolves as a bare command.
# Safe to delete; Recast's uninstall verb removes this block.
if [ -d \"$HOME/.local/bin\" ]; then
    case \":$PATH:\" in
        *\":$HOME/.local/bin:\"*) ;;
        *) export PATH=\"$HOME/.local/bin:$PATH\" ;;
    esac
fi
# <<< recast cli <<<";

#[derive(Serialize, Clone)]
#[cfg(unix)]
#[serde(rename_all = "camelCase")]
pub struct RcFileChange {
    /// Absolute path of the file written or removed.
    pub path: String,
    /// `added`, `updated`, `removed`, or `no_change`.
    pub change: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct InstallStatus {
    /// Whether `recast` currently resolves from a terminal.
    pub on_path: bool,
    /// The directory the CLI is (or would be) reachable from.
    pub bin_dir: String,
    /// Human-readable summary.
    pub detail: String,
    /// Per-shell rc files that already carry the `recast` block. UI uses
    /// this to render "Modified ~/.zshrc" pill chips beside the toggle.
    #[serde(default)]
    pub modified_rc_files: Vec<String>,
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

/// Stable per-user install directory — independent of the dev tree so
/// uninstall is fully effective. The copy inside this directory is what
/// `recast` resolves to in any shell; the symlink / registry entry in
/// `install()` points at it. `uninstall()` deletes the file itself, so
/// `recast` truly goes away.
fn stable_install_dir() -> PathBuf {
    #[cfg(unix)]
    {
        // XDG-style per-user data dir: $HOME/.local/share/<bundle-id>/bin
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_default();
        home.join(".local")
            .join("share")
            .join("com.kanakkholwal.recast")
            .join("bin")
    }
    #[cfg(windows)]
    {
        // %LOCALAPPDATA%\<bundle-id>\bin
        let local: PathBuf = std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                // Fallback: USERPROFILE%\AppData\Local — matches the
                // Windows default of %LOCALAPPDATA% when not explicitly
                // set (rare; only seen under some service contexts).
                std::env::var_os("USERPROFILE")
                    .map(PathBuf::from)
                    .unwrap_or_default()
                    .join("AppData")
                    .join("Local")
            });
        local.join("com.kanakkholwal.recast").join("bin")
    }
}

fn stable_install_file() -> PathBuf {
    #[cfg(windows)]
    {
        stable_install_dir().join("recast.exe")
    }
    #[cfg(not(windows))]
    {
        stable_install_dir().join("recast")
    }
}

/// Bytes copied into the stable install dir. We write with `read +
/// write` (not `copy`) so we can clamp the file mode on Unix to
/// owner-rwx without depending on `fs::copy`'s mode preservation.
fn copy_to_stable() -> Result<String, String> {
    use std::io::{Read, Write};
    let src = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut in_f = std::fs::File::open(&src)
        .map_err(|e| format!("open source binary {}: {e}", src.display()))?;
    let dst = stable_install_file();
    if let Some(parent) = dst.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("create install dir {}: {e}", parent.display()))?;
    }
    let mut buf = Vec::with_capacity(64 * 1024);
    in_f.read_to_end(&mut buf).map_err(|e| e.to_string())?;
    {
        let mut out_f = std::fs::File::create(&dst)
            .map_err(|e| format!("create install file {}: {e}", dst.display()))?;
        out_f.write_all(&buf).map_err(|e| e.to_string())?;
        out_f.flush().map_err(|e| e.to_string())?;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o755);
        std::fs::set_permissions(&dst, perms).map_err(|e| e.to_string())?;
    }
    Ok(dst.to_string_lossy().to_string())
}

/// Read which rc files already carry our block. Used by `status()` so the
/// settings UI can show "Modified: ~/.zshrc, ~/.bash_profile".
#[cfg(unix)]
pub(crate) fn modified_rc_files() -> Vec<String> {
    shell_rc_candidates()
        .into_iter()
        .filter(|p| p.exists() && file_contains_block(p))
        .map(|p| p.to_string_lossy().to_string())
        .collect()
}

/// Return the shell-rc candidates we'd touch on this platform, in priority
/// order. On macOS we always include `~/.zprofile` (Cataloga+ default zsh
/// login) and `~/.zshrc`; on Linux we include whichever of `~/.bashrc`,
/// `~/.zshrc`, and `~/.profile` exist or could be created.
#[cfg(unix)]
fn shell_rc_candidates() -> Vec<PathBuf> {
    let mut out = Vec::new();
    let home = match std::env::var_os("HOME") {
        Some(h) if !h.is_empty() => PathBuf::from(h),
        _ => return out,
    };
    #[cfg(target_os = "macos")]
    {
        out.push(home.join(".zprofile"));
        out.push(home.join(".zshrc"));
        out.push(home.join(".bash_profile"));
        out.push(home.join(".bashrc"));
    }
    #[cfg(not(target_os = "macos"))]
    {
        out.push(home.join(".bashrc"));
        out.push(home.join(".zshrc"));
        out.push(home.join(".profile"));
        out.push(home.join(".bash_profile"));
    }
    out
}

/// Idempotently insert the `recast` block into `path`. Returns `RcFileChange`
/// describing what actually happened.
#[cfg(unix)]
fn upsert_rc_block(path: &Path) -> RcFileChange {
    let path_str = path.to_string_lossy().to_string();
    let had_block = file_contains_block(path);
    let body = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(_) => String::new(),
    };
    if had_block {
        // Already present — leave the existing block verbatim so a manual
        // user edit survives `recast install`. The settings toggle's notion
        // of "modified" stays accurate.
        return RcFileChange {
            path: path_str,
            change: "no_change".into(),
        };
    }
    let mut next = body;
    if !next.is_empty() && !next.ends_with('\n') {
        next.push('\n');
    }
    if !next.is_empty() {
        next.push('\n');
    }
    next.push_str(PATH_BLOCK);
    next.push('\n');
    if let Err(e) = atomic_write(path, next.as_bytes()) {
        return RcFileChange {
            path: path_str,
            change: format!("write_failed:{e}"),
        };
    }
    RcFileChange {
        path: path_str,
        change: "added".into(),
    }
}

/// Remove the `recast` block from `path`, if present.
#[cfg(unix)]
fn remove_rc_block(path: &Path) -> RcFileChange {
    let path_str = path.to_string_lossy().to_string();
    if !path.exists() {
        return RcFileChange {
            path: path_str,
            change: "no_change".into(),
        };
    }
    let body = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(_) => {
            return RcFileChange {
                path: path_str,
                change: "no_change".into(),
            }
        }
    };
    let Some((start, end)) = locate_block(&body) else {
        return RcFileChange {
            path: path_str,
            change: "no_change".into(),
        };
    };
    // Strip markers + surrounding blank lines cleanly.
    let mut next = String::with_capacity(body.len());
    next.push_str(&body[..start]);
    next.push_str(&body[end..]);
    if let Err(e) = atomic_write(path, next.as_bytes()) {
        return RcFileChange {
            path: path_str,
            change: format!("write_failed:{e}"),
        };
    }
    RcFileChange {
        path: path_str,
        change: "removed".into(),
    }
}

#[cfg(unix)]
fn file_contains_block(path: &Path) -> bool {
    std::fs::read_to_string(path)
        .map(|s| locate_block(&s).is_some())
        .unwrap_or(false)
}

/// Returns `(start, end)` byte indices of the `recast` block if present.
#[allow(dead_code)] // tests use it on Windows; production use is Unix-only.
fn locate_block(body: &str) -> Option<(usize, usize)> {
    let start = body.find(BEGIN_MARKER)?;
    // Include the trailing newline of the END marker line if any, so
    // removal leaves a clean break.
    let after = &body[start..];
    let end_rel = after.find(END_MARKER)? + END_MARKER.len();
    let mut end = start + end_rel;
    if body[end..].starts_with('\n') {
        end += 1;
    }
    Some((start, end))
}

#[allow(dead_code)] // tests use it on Windows; production use is Unix-only.
fn atomic_write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let tmp = path.with_extension("recast-tmp");
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

/// Apply the install/uninstall block edits to whichever rc files exist on
/// this platform. Returns the list of changes so the caller can surface a
/// per-file status in the UI.
#[cfg(unix)]
fn apply_rc_files(action: RcAction) -> Vec<RcFileChange> {
    let candidates: Vec<PathBuf> = shell_rc_candidates()
        .into_iter()
        .filter(|p| p.exists())
        .collect();
    if candidates.is_empty() {
        return Vec::new();
    }
    match action {
        RcAction::Install => candidates
            .into_iter()
            .map(|p| upsert_rc_block(&p))
            .collect(),
        RcAction::Uninstall => candidates
            .into_iter()
            .map(|p| remove_rc_block(&p))
            .collect(),
    }
}

#[derive(Copy, Clone)]
#[cfg(unix)]
enum RcAction {
    Install,
    Uninstall,
}

#[cfg(windows)]
mod platform {
    use super::*;
    use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_WRITE, REG_EXPAND_SZ};
    use winreg::{RegKey, RegValue};

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
        // Trust only what's in the registry — and only against the stable
        // install dir, not the dev binary's parent. Dev shells sometimes
        // have `target/debug/` on PATH for unrelated reasons, which
        // would otherwise claim the CLI is "still installed" after we
        // uninstalled.
        let dir = super::stable_install_dir();
        let in_registry = open_env()
            .map(|e| contains(&read_path(&e), &dir.to_string_lossy()))
            .unwrap_or(false);
        let on_path = in_registry;
        InstallStatus {
            detail: if on_path {
                "recast is on your PATH".into()
            } else {
                "recast is not on your PATH".into()
            },
            on_path,
            bin_dir: dir.to_string_lossy().to_string(),
            modified_rc_files: Vec::new(),
        }
    }

    pub fn install() -> Result<String, String> {
        // Always copy to the stable install dir first. Even when dev
        // mode, the registered PATH points at the copy (not at the dev
        // binary's parent dir), so uninstall is fully effective.
        super::copy_to_stable()?;
        let dir = super::stable_install_dir();
        let dir_str = dir.to_string_lossy();
        let env = open_env()?;
        let current = read_path(&env);
        if contains(&current, &dir_str) {
            return Ok(format!("`recast` is already on your PATH ({dir_str})."));
        }
        let next = if current.trim().is_empty() {
            dir_str.to_string()
        } else {
            format!("{};{dir_str}", current.trim_end_matches(';'),)
        };
        write_path(&env, &next)?;
        broadcast();
        Ok(format!(
            "Installed `recast` to {dir_str}. Open a new terminal to use it."
        ))
    }

    pub fn uninstall() -> Result<String, String> {
        let mut actions = Vec::new();
        let dir = super::stable_install_dir();
        let dir_str = dir.to_string_lossy();
        if let Ok(env) = open_env() {
            let current = read_path(&env);
            if contains(&current, &dir_str) {
                let next: Vec<&str> = current
                    .split(';')
                    .filter(|p| !p.trim().is_empty() && !p.trim().eq_ignore_ascii_case(&dir_str))
                    .collect();
                write_path(&env, &next.join(";"))?;
                broadcast();
                actions.push(format!("Removed `recast` from your PATH ({dir_str})."));
            }
        }
        // Always attempt to delete the stable copy too — uninstall is
        // fully effective only when the underlying executable is gone.
        let binary = super::stable_install_file();
        if binary.exists() {
            match std::fs::remove_file(&binary) {
                Ok(()) => actions.push(format!("Deleted {}.", binary.display())),
                Err(e) => actions.push(format!("Failed to delete {}: {e}", binary.display())),
            }
        }
        if actions.is_empty() {
            Ok("`recast` was not installed.".into())
        } else {
            Ok(actions.join(" "))
        }
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
    use std::os::unix::fs::symlink;
    use std::path::PathBuf;

    fn home() -> PathBuf {
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_default()
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

    /// Set of rc files that already carry our PATH block.
    pub fn rc_files_with_block() -> std::collections::BTreeSet<String> {
        let mut out = BTreeSet::new();
        for p in shell_rc_candidates() {
            if p.exists() && file_contains_block(&p) {
                out.insert(p.to_string_lossy().to_string());
            }
        }
        out
    }

    pub fn status() -> InstallStatus {
        // After install, `recast` is reachable via the symlink at
        // `~/.local/bin/recast`, which points at the stable copy at
        // `~/.local/share/com.kanakkholwal.recast/bin/recast`. The
        // symlink check is what matters here — checking the dev binary's
        // PATH entry would falsely report "installed" because dev shells
        // typically inherit a PATH that contains the dev tree.
        let symlink_ok = link_path().exists();
        let modified = rc_files_with_block().into_iter().collect::<Vec<_>>();
        let new_shell_will_have_dir = dir_on_path(&bin_dir()) || !modified.is_empty();
        let on_path = symlink_ok && new_shell_will_have_dir;
        InstallStatus {
            detail: if on_path {
                "recast is on your PATH".into()
            } else {
                "recast is not on your PATH".into()
            },
            on_path,
            bin_dir: bin_dir().to_string_lossy().to_string(),
            modified_rc_files: modified,
        }
    }

    pub fn install() -> Result<String, String> {
        // Copy first — the symlink target must point at a stable file
        // outside the dev tree, otherwise `recast --help` keeps
        // resolving even after uninstall (the dev binary never gets
        // removed).
        let copied = super::copy_to_stable()?;
        std::fs::create_dir_all(&bin_dir()).map_err(|e| e.to_string())?;
        let link = link_path();
        if link.symlink_metadata().is_ok() {
            let _ = std::fs::remove_file(&link);
        }
        symlink(Path::new(&copied), &link).map_err(|e| e.to_string())?;
        let rc_changes = apply_rc_files(RcAction::Install);

        let mut msg = format!("Installed `recast` to {copied}.");
        if dir_on_path(&bin_dir()) {
            msg.push_str(" Now resolvable as `recast` in any new terminal.");
        } else {
            msg.push_str(" Added `~/.local/bin` to your shell PATH via the recast block in: ");
            msg.push_str(
                &rc_changes
                    .iter()
                    .filter(|c| c.change == "added")
                    .map(|c| c.path.clone())
                    .collect::<Vec<_>>()
                    .join(", "),
            );
            msg.push_str(
                ". Open a new terminal (or `source` one of those files) for `recast` to resolve.",
            );
        }
        Ok(msg)
    }

    pub fn uninstall() -> Result<String, String> {
        let mut actions = Vec::new();

        // Drop the symlink in `~/.local/bin/`.
        let link = link_path();
        if link.symlink_metadata().is_ok() {
            std::fs::remove_file(&link).map_err(|e| e.to_string())?;
            actions.push(format!("Removed {}.", link.display()));
        }

        // Drop the stable copy at `~/.local/share/...`. Critical: this
        // is what makes uninstall fully effective in dev mode — without
        // deleting the copy, the next shell that re-sources a non-recast
        // PATH entry pointing at the same dir (e.g. a Tauri dev hook)
        // would still resolve `recast`.
        let stable = super::stable_install_file();
        if stable.exists() {
            match std::fs::remove_file(&stable) {
                Ok(()) => actions.push(format!("Deleted {}.", stable.display())),
                Err(e) => actions.push(format!("Failed to delete {}: {e}", stable.display())),
            }
        }

        // Revert the rc-file edits.
        let rc_changes = apply_rc_files(RcAction::Uninstall);
        for change in &rc_changes {
            if change.change == "removed" {
                actions.push(format!("Removed recast block from {}.", change.path));
            }
        }

        if actions.is_empty() {
            Ok("`recast` was not installed.".into())
        } else {
            Ok(actions.join(" "))
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn locate_block_finds_markers() {
        let body = "\
# preamble
# >>> recast cli >>>
# Added by Recast so the `recast` CLI resolves as a bare command.
if [ -d \"$HOME/.local/bin\" ]; then
    case \":$PATH:\" in
        *\":$HOME/.local/bin:\"*) ;;
        *) export PATH=\"$HOME/.local/bin:$PATH\" ;;
    esac
fi
# <<< recast cli <<<
# after
";
        assert!(locate_block(body).is_some());
    }

    #[test]
    fn locate_block_returns_none_when_absent() {
        let body = "some other content\nno markers here\n";
        assert!(locate_block(body).is_none());
    }

    #[test]
    fn upsert_then_remove_roundtrip() {
        let dir = tempdir_test();
        let path = dir.join(".zshrc");
        std::fs::write(&path, "# pre-existing\n").unwrap();
        let ch = upsert_rc_block(&path);
        assert_eq!(ch.change, "added");
        let after = std::fs::read_to_string(&path).unwrap();
        assert!(after.contains(BEGIN_MARKER));
        assert!(after.contains(END_MARKER));

        // Idempotent: a second install is a no-op.
        let ch = upsert_rc_block(&path);
        assert_eq!(ch.change, "no_change");

        // Uninstall restores the file but keeps the rest of the content.
        let ch = remove_rc_block(&path);
        assert_eq!(ch.change, "removed");
        let after = std::fs::read_to_string(&path).unwrap();
        assert!(!after.contains(BEGIN_MARKER));
        assert!(after.contains("# pre-existing"), "got: {after}");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn install_path_block_is_valid_shell() {
        // Sanity: the literal block parses as a `case` against an empty PATH.
        // We can't run sh in a unit test, but we can grep for the obvious
        // structure markers so a typo in PATH_BLOCK doesn't silently break
        // every macOS/Linux install.
        assert!(PATH_BLOCK.contains(BEGIN_MARKER));
        assert!(PATH_BLOCK.contains(END_MARKER));
        assert!(PATH_BLOCK.contains("$HOME/.local/bin"));
        assert!(PATH_BLOCK.contains("export PATH"));
    }

    fn tempdir_test() -> PathBuf {
        let base = std::env::temp_dir().join(format!(
            "recast-path-install-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        std::fs::create_dir_all(&base).unwrap();
        base
    }
}

#[cfg(test)]
mod stable_install_tests {
    //! Cross-platform invariants on `stable_install_dir` /
    //! `stable_install_file`. Run on every host so a future platform
    //! branch change (e.g. moving to AppData/Local again) keeps the
    //! "outside the dev tree" guarantee.
    use super::*;

    #[test]
    fn stable_install_dir_is_outside_dev_tree() {
        // The install dir must NOT live inside the dev tree
        // (`target/debug/...`) or `~/.cargo`. Otherwise an uninstall
        // that only deletes from PATH leaves the binary still
        // resolvable through a stale dev-PATH entry — the user's
        // reported bug.
        let dir = stable_install_dir();
        let dir_str = dir.to_string_lossy();
        assert!(
            !dir_str.contains("target"),
            "stable install dir must not be under `target/` (got {dir_str})"
        );
        assert!(
            !dir_str.contains(".cargo"),
            "stable install dir must not be under `~/.cargo/` (got {dir_str})"
        );
        assert!(
            dir_str.contains("recast"),
            "stable install dir should be Recast-owned (got {dir_str})"
        );
    }

    #[test]
    fn stable_install_file_has_sensible_name() {
        // Lowercase `recast` everywhere — Windows is case-insensitive on
        // PATH lookups, but the documented CLI contract and the smoke
        // test script both call `recast` (not `Recast`).
        let f = stable_install_file();
        let name = f.file_name().and_then(|n| n.to_str()).unwrap_or("");
        assert!(
            name == "recast" || name == "recast.exe",
            "stable install filename must be the canonical `recast` (got `{name}`)"
        );
    }
}
