use std::path::PathBuf;
use std::process::Command;
use std::sync::OnceLock;

/// Resolved paths to ffmpeg and ffprobe binaries.
/// Checked once at startup and cached for the process lifetime.
struct FfmpegPaths {
    ffmpeg: PathBuf,
    ffprobe: PathBuf,
}

static PATHS: OnceLock<FfmpegPaths> = OnceLock::new();

fn resolve() -> &'static FfmpegPaths {
    PATHS.get_or_init(|| {
        // 1. Check for bundled sidecar binaries next to the executable.
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                let ffmpeg = dir.join("ffmpeg.exe");
                let ffprobe = dir.join("ffprobe.exe");
                if ffmpeg.exists() && ffprobe.exists() {
                    log::info!("using bundled ffmpeg: {}", ffmpeg.display());
                    return FfmpegPaths { ffmpeg, ffprobe };
                }

                // Also check a `bin/` subdirectory.
                let ffmpeg = dir.join("bin").join("ffmpeg.exe");
                let ffprobe = dir.join("bin").join("ffprobe.exe");
                if ffmpeg.exists() && ffprobe.exists() {
                    log::info!("using bundled ffmpeg from bin/: {}", ffmpeg.display());
                    return FfmpegPaths { ffmpeg, ffprobe };
                }
            }
        }

        // 2. Check common install locations on Windows.
        #[cfg(windows)]
        {
            let common_paths = [
                r"C:\ffmpeg\bin\ffmpeg.exe",
                r"C:\Program Files\ffmpeg\bin\ffmpeg.exe",
                r"C:\tools\ffmpeg\bin\ffmpeg.exe",
            ];
            for path in common_paths {
                let ffmpeg = PathBuf::from(path);
                if ffmpeg.exists() {
                    let ffprobe = ffmpeg.with_file_name("ffprobe.exe");
                    if ffprobe.exists() {
                        log::info!("using system ffmpeg: {}", ffmpeg.display());
                        return FfmpegPaths { ffmpeg, ffprobe };
                    }
                }
            }
        }

        // 3. Fall back to PATH lookup.
        log::info!("using ffmpeg from PATH");
        FfmpegPaths {
            ffmpeg: PathBuf::from("ffmpeg"),
            ffprobe: PathBuf::from("ffprobe"),
        }
    })
}

/// Get the resolved path to the ffmpeg binary.
pub fn ffmpeg_path() -> &'static PathBuf {
    &resolve().ffmpeg
}

/// Get the resolved path to the ffprobe binary.
pub fn ffprobe_path() -> &'static PathBuf {
    &resolve().ffprobe
}

/// Detect the best available H.264 encoder on the system.
/// Prefers hardware NVENC when available, falling back to libx264.
/// Cached for the process lifetime — `ffmpeg -encoders` costs ~200–300ms cold.
pub fn preferred_h264_encoder() -> &'static str {
    static CACHED: OnceLock<&'static str> = OnceLock::new();
    CACHED.get_or_init(|| {
        let output = Command::new(ffmpeg_path())
            .args(["-hide_banner", "-encoders"])
            .output();
        match output {
            Ok(result) if result.status.success() => {
                let encoders = String::from_utf8_lossy(&result.stdout);
                if encoders.contains("h264_nvenc") {
                    log::info!("preferred H.264 encoder: h264_nvenc");
                    "h264_nvenc"
                } else {
                    log::info!("preferred H.264 encoder: libx264 (no hardware encoder detected)");
                    "libx264"
                }
            }
            _ => {
                log::warn!("failed to probe ffmpeg encoders, defaulting to libx264");
                "libx264"
            }
        }
    })
}

/// Check if ffmpeg is available. Returns an error message if not.
pub fn check_availability() -> Result<(), String> {
    let output = Command::new(ffmpeg_path()).arg("-version").output();

    match output {
        Ok(o) if o.status.success() => Ok(()),
        Ok(o) => Err(format!(
            "ffmpeg found but returned error: {}",
            String::from_utf8_lossy(&o.stderr)
        )),
        Err(e) => Err(format!(
            "ffmpeg not found. Install ffmpeg and add it to PATH, or place ffmpeg.exe next to the application. Error: {e}"
        )),
    }
}
