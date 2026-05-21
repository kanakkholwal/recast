use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;
use tauri::Manager;

/// Resolved paths to ffmpeg and ffprobe binaries.
/// Checked once at startup and cached for the process lifetime.
struct FfmpegPaths {
    ffmpeg: PathBuf,
    ffprobe: PathBuf,
}

static PATHS: OnceLock<FfmpegPaths> = OnceLock::new();

#[cfg(windows)]
const EXE_SUFFIX: &str = ".exe";
#[cfg(not(windows))]
const EXE_SUFFIX: &str = "";

#[cfg(all(windows, target_arch = "x86_64"))]
const TARGET_TRIPLE: &str = "x86_64-pc-windows-msvc";
#[cfg(all(windows, target_arch = "aarch64"))]
const TARGET_TRIPLE: &str = "aarch64-pc-windows-msvc";
#[cfg(all(target_os = "macos", target_arch = "x86_64"))]
const TARGET_TRIPLE: &str = "x86_64-apple-darwin";
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
const TARGET_TRIPLE: &str = "aarch64-apple-darwin";
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
const TARGET_TRIPLE: &str = "x86_64-unknown-linux-gnu";
#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
const TARGET_TRIPLE: &str = "aarch64-unknown-linux-gnu";
#[cfg(not(any(
    all(windows, any(target_arch = "x86_64", target_arch = "aarch64")),
    all(
        target_os = "macos",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ),
    all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    )
)))]
const TARGET_TRIPLE: &str = "";

/// Initialize FFmpeg resolution with Tauri's resource directory available.
/// Call this during app setup before any export/recording command runs.
pub fn init(app: &tauri::AppHandle) {
    let _ = PATHS.get_or_init(|| resolve_paths(Some(app)));
}

fn resolve() -> &'static FfmpegPaths {
    PATHS.get_or_init(|| resolve_paths(None))
}

fn resolve_paths(app: Option<&tauri::AppHandle>) -> FfmpegPaths {
    if let Some(paths) = find_bundled_pair(app) {
        return paths;
    }

    // Check common install locations on Windows.
    #[cfg(windows)]
    {
        let common_paths = [
            r"C:\ffmpeg\bin\ffmpeg.exe",
            r"C:\Program Files\ffmpeg\bin\ffmpeg.exe",
            r"C:\tools\ffmpeg\bin\ffmpeg.exe",
        ];
        for path in common_paths {
            let ffmpeg = PathBuf::from(path);
            let ffprobe = ffmpeg.with_file_name("ffprobe.exe");
            if is_usable_pair(&ffmpeg, &ffprobe) {
                log::info!("using system ffmpeg: {}", ffmpeg.display());
                return FfmpegPaths { ffmpeg, ffprobe };
            }
            if ffmpeg.exists() || ffprobe.exists() {
                log::warn!(
                    "ignoring unusable system ffmpeg pair: {} / {}",
                    ffmpeg.display(),
                    ffprobe.display()
                );
            }
        }
    }

    // Fall back to PATH lookup. This is intentionally last because PATH may
    // contain broken package-manager shims.
    let ffmpeg = PathBuf::from(format!("ffmpeg{EXE_SUFFIX}"));
    let ffprobe = PathBuf::from(format!("ffprobe{EXE_SUFFIX}"));
    if is_usable_pair(&ffmpeg, &ffprobe) {
        log::info!("using ffmpeg from PATH");
    } else {
        log::warn!("ffmpeg/ffprobe from PATH are not currently executable");
    }

    FfmpegPaths { ffmpeg, ffprobe }
}

fn find_bundled_pair(app: Option<&tauri::AppHandle>) -> Option<FfmpegPaths> {
    let mut roots = Vec::new();

    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            roots.push(dir.to_path_buf());
        }
    }

    if let Some(app) = app {
        if let Ok(resource_dir) = app.path().resource_dir() {
            roots.push(resource_dir);
        }
    }

    for root in roots {
        for dir in bundled_search_dirs(&root) {
            for (ffmpeg, ffprobe) in candidate_pairs(&dir) {
                if is_usable_pair(&ffmpeg, &ffprobe) {
                    log::info!("using bundled ffmpeg: {}", ffmpeg.display());
                    return Some(FfmpegPaths { ffmpeg, ffprobe });
                }
                if ffmpeg.exists() || ffprobe.exists() {
                    log::warn!(
                        "ignoring unusable bundled ffmpeg pair: {} / {}",
                        ffmpeg.display(),
                        ffprobe.display()
                    );
                }
            }
        }
    }

    None
}

fn bundled_search_dirs(root: &Path) -> Vec<PathBuf> {
    vec![root.to_path_buf(), root.join("bin"), root.join("binaries")]
}

fn candidate_pairs(dir: &Path) -> Vec<(PathBuf, PathBuf)> {
    let mut pairs = vec![(
        dir.join(format!("ffmpeg{EXE_SUFFIX}")),
        dir.join(format!("ffprobe{EXE_SUFFIX}")),
    )];

    if !TARGET_TRIPLE.is_empty() {
        pairs.push((
            dir.join(format!("ffmpeg-{TARGET_TRIPLE}{EXE_SUFFIX}")),
            dir.join(format!("ffprobe-{TARGET_TRIPLE}{EXE_SUFFIX}")),
        ));
    }

    pairs
}

fn is_usable_pair(ffmpeg: &Path, ffprobe: &Path) -> bool {
    ffmpeg.exists()
        && ffprobe.exists()
        && command_succeeds(ffmpeg, "-version")
        && command_succeeds(ffprobe, "-version")
}

fn command_succeeds(path: &Path, arg: &str) -> bool {
    let mut command = Command::new(path);
    command.arg(arg);
    configure_silent_command(&mut command);
    command
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

/// Apply Windows-specific spawn options that hide the console window.
/// No-op on non-Windows platforms. Call on every ffmpeg/ffprobe `Command`
/// before `.spawn()` / `.output()` to prevent black console windows from
/// flashing on Windows when sidecar binaries are launched.
pub fn configure_silent_command(cmd: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    #[cfg(not(windows))]
    {
        let _ = cmd;
    }
}

/// Get the resolved path to the ffmpeg binary.
pub fn ffmpeg_path() -> &'static PathBuf {
    &resolve().ffmpeg
}

/// Get the resolved path to the ffprobe binary.
pub fn ffprobe_path() -> &'static PathBuf {
    &resolve().ffprobe
}

/// Cached AVFoundation device listing (macOS only).
///
/// `ffmpeg -f avfoundation -list_devices true -i ""` is the only way to
/// enumerate AVFoundation video + audio devices, and the probe is
/// expensive: spawning FFmpeg, loading the framework, and walking the
/// device list takes ~200–500 ms cold. Three subsystems need this
/// output (camera enumeration, audio loopback detection, and the screen
/// capture's "first screen index" lookup), and before this helper each
/// of them ran its own private listing — so every recording start
/// spawned FFmpeg twice or three times just to list devices, on top of
/// the actual capture processes. That showed up as a burst of
/// short-lived FFmpeg children in Activity Monitor and as a visible
/// stutter on slower Macs.
///
/// Caching the output once per process collapses that to a single
/// probe at the first device-list need. The downside is brand-new
/// devices plugged in *after* launch won't appear until restart; for a
/// recorder app where the device picker runs on the JS side anyway,
/// this is an acceptable trade.
#[cfg(target_os = "macos")]
pub fn cached_avfoundation_devices() -> &'static str {
    static CACHED: OnceLock<String> = OnceLock::new();
    CACHED.get_or_init(|| {
        let mut command = Command::new(ffmpeg_path());
        // The empty `-i ""` is intentional: AVFoundation listing requires
        // the format flag, and FFmpeg refuses to start without an input,
        // so we hand it an empty one and let it fail. The device list
        // gets printed to stderr *before* the failure, which is what we
        // care about.
        command.args([
            "-hide_banner",
            "-f",
            "avfoundation",
            "-list_devices",
            "true",
            "-i",
            "",
        ]);
        configure_silent_command(&mut command);
        match command.output() {
            Ok(out) => String::from_utf8_lossy(&out.stderr).into_owned(),
            Err(e) => {
                log::warn!("avfoundation device list probe failed: {e}");
                String::new()
            }
        }
    })
}

/// Detect the best available H.264 encoder on the system.
/// Prefers hardware NVENC when available, falling back to libx264.
/// Cached for the process lifetime — `ffmpeg -encoders` costs ~200–300ms cold.
pub fn preferred_h264_encoder() -> &'static str {
    static CACHED: OnceLock<&'static str> = OnceLock::new();
    CACHED.get_or_init(|| {
        let mut command = Command::new(ffmpeg_path());
        command.args(["-hide_banner", "-encoders"]);
        configure_silent_command(&mut command);
        let output = command.output();
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
    let mut command = Command::new(ffmpeg_path());
    command.arg("-version");
    configure_silent_command(&mut command);
    let output = command.output();

    match output {
        Ok(o) if o.status.success() => Ok(()),
        Ok(o) => Err(format!(
            "ffmpeg at {} returned error: {}",
            ffmpeg_path().display(),
            String::from_utf8_lossy(&o.stderr)
        )),
        Err(e) => Err(format!(
            "ffmpeg not found or not executable at {}. Bundle ffmpeg/ffprobe as Tauri sidecars, install ffmpeg, or place ffmpeg{EXE_SUFFIX} and ffprobe{EXE_SUFFIX} next to the application. Error: {e}",
            ffmpeg_path().display()
        )),
    }
}
