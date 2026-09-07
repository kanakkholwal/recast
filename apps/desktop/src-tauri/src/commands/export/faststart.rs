use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub(crate) fn needs_faststart(format: &str) -> bool {
    matches!(format, "mp4" | "mov" | "m4v")
}

fn temp_sibling(path: &Path) -> PathBuf {
    let mut name = path.file_stem().unwrap_or_default().to_os_string();
    name.push(".faststart.");
    name.push(path.extension().unwrap_or_default());
    path.with_file_name(name)
}

/// Rewrite `path` with the `moov` atom in front so a shared upload starts
/// playing before it has fully downloaded. Stream copy, so no re-encode.
/// Non-fatal: a failure leaves the original export untouched.
pub(crate) fn apply(path: &Path) -> Result<(), String> {
    let temp = temp_sibling(path);
    let mut command = Command::new(crate::ffmpeg::ffmpeg_path());
    command
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-i",
            &path.to_string_lossy(),
            "-map",
            "0",
            "-c",
            "copy",
            "-movflags",
            "+faststart",
            &temp.to_string_lossy(),
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    crate::ffmpeg::configure_silent_command(&mut command);

    let output = command.output().map_err(|e| e.to_string())?;
    if !output.status.success() {
        let _ = std::fs::remove_file(&temp);
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    if std::fs::metadata(&temp).map(|m| m.len()).unwrap_or(0) == 0 {
        let _ = std::fs::remove_file(&temp);
        return Err("faststart remux produced an empty file".into());
    }
    std::fs::rename(&temp, path).map_err(|e| {
        let _ = std::fs::remove_file(&temp);
        e.to_string()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_mp4_family_outputs_are_remuxed() {
        assert!(needs_faststart("mp4"));
        assert!(needs_faststart("mov"));
        assert!(!needs_faststart("webm"));
        assert!(!needs_faststart("gif"));
    }

    #[test]
    fn the_temp_file_is_a_sibling_of_the_export() {
        let out = Path::new("/exports/My Recording (1).mp4");
        let temp = temp_sibling(out);
        assert_eq!(temp.parent(), out.parent());
        assert_eq!(temp.file_name().unwrap(), "My Recording (1).faststart.mp4");
    }

    #[test]
    fn a_missing_source_fails_rather_than_deleting_the_export() {
        let missing = Path::new("this-export-does-not-exist.mp4");
        assert!(apply(missing).is_err());
        assert!(!temp_sibling(missing).exists());
    }
}
