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

    // Check common install locations on macOS/Linux.
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    for &dir in common_ffmpeg_dirs() {
        let (ffmpeg, ffprobe) = system_ffmpeg_pair(dir);
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

    // PATH lookup last, because PATH may contain broken package-manager shims.
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

/// Well-known ffmpeg install prefixes, probed before the PATH fallback.
///
/// A Finder- or launcher-started `.app` inherits a minimal PATH (often just
/// `/usr/bin:/bin`) that excludes Homebrew and MacPorts, so a PATH lookup alone
/// reports "ffmpeg not found" even when it is installed. These are absolute, so
/// they resolve regardless of the inherited PATH.
#[cfg(any(target_os = "macos", target_os = "linux"))]
fn common_ffmpeg_dirs() -> &'static [&'static str] {
    #[cfg(target_os = "macos")]
    {
        &[
            "/opt/homebrew/bin", // Apple Silicon Homebrew
            "/usr/local/bin",    // Intel Homebrew
            "/opt/local/bin",    // MacPorts
        ]
    }
    #[cfg(target_os = "linux")]
    {
        &["/usr/bin", "/usr/local/bin", "/bin", "/snap/bin"]
    }
}

/// The `(ffmpeg, ffprobe)` pair inside a system install dir. Both must come from
/// the SAME directory: mixing a Homebrew ffmpeg with a different ffprobe is the
/// mismatched-pair bug `is_usable_pair` exists to reject.
#[cfg(any(target_os = "macos", target_os = "linux"))]
fn system_ffmpeg_pair(dir: &str) -> (PathBuf, PathBuf) {
    let base = Path::new(dir);
    (base.join("ffmpeg"), base.join("ffprobe"))
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

/// Maximum stderr tail retained for diagnostics. The fatal line is always at
/// the end (codec error, disk full, etc.); FFmpeg's startup chatter is noise.
const STDERR_TAIL_LIMIT: usize = 8192;

/// Drains a long-lived FFmpeg child's stderr to a bounded tail on its own thread. Every such child that pipes stderr MUST be wrapped at spawn.
/// Load-bearing, not diagnostic: an undrained ~64KB pipe blocks FFmpeg's write, it stops producing frames, and a graceful quit stalls into a corrupt MP4.
pub struct StderrTail {
    handle: Option<std::thread::JoinHandle<()>>,
    sink: std::sync::Arc<parking_lot::Mutex<String>>,
}

impl StderrTail {
    /// Spawn the drain thread for `stderr`. Returns immediately; the thread runs for the whole life of the process and exits at EOF (i.e. when FFmpeg closes stderr on exit).
    pub fn spawn(stderr: std::process::ChildStderr) -> Self {
        let sink = std::sync::Arc::new(parking_lot::Mutex::new(String::new()));
        let sink_clone = sink.clone();
        let handle = std::thread::Builder::new()
            .name("recast-ffmpeg-stderr".into())
            .spawn(move || pump_stderr_tail(stderr, sink_clone))
            .ok();
        Self { handle, sink }
    }

    /// A snapshot of the tail retained so far, without consuming the pump. Safe
    /// to call while the child is still alive (e.g. on an early-exit error path).
    pub fn snapshot(&self) -> String {
        self.sink.lock().clone()
    }

    /// Join the drain thread and return the retained tail. The pump only exits
    /// once FFmpeg closes stderr, so the child must already be exiting/exited
    /// before this is called (it is, after `graceful_stop`/`wait`).
    pub fn collect(mut self) -> String {
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
        self.sink.lock().clone()
    }
}

impl Drop for StderrTail {
    fn drop(&mut self) {
        // If `collect()` already ran the handle is gone; otherwise detach, since the child has closed stderr and the pump hits EOF.
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn pump_stderr_tail(
    stderr: std::process::ChildStderr,
    sink: std::sync::Arc<parking_lot::Mutex<String>>,
) {
    use std::io::Read;
    let mut reader = std::io::BufReader::new(stderr);
    let mut chunk = [0u8; 4096];
    loop {
        match reader.read(&mut chunk) {
            Ok(0) => break, // EOF — FFmpeg closed stderr (i.e. exited).
            Ok(n) => {
                let mut tail = sink.lock();
                tail.push_str(&String::from_utf8_lossy(&chunk[..n]));
                if tail.len() > STDERR_TAIL_LIMIT {
                    let mut cut = tail.len() - STDERR_TAIL_LIMIT;
                    // Prefer a newline boundary so the tail starts on a clean line; fall back to the raw offset.
                    if let Some(nl) = tail[cut..].find('\n') {
                        cut += nl + 1;
                    }
                    // `drain` panics off a char boundary, and lossy decoding can straddle chunks, so back off first.
                    while cut < tail.len() && !tail.is_char_boundary(cut) {
                        cut += 1;
                    }
                    tail.drain(..cut);
                }
            }
            Err(_) => break,
        }
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

/// Best available H.264 encoder, found by actually running a 1-frame encode per hardware candidate; `-encoders` only proves a codec was compiled in.
/// Priority nvenc, amf, qsv, then libx264. Cached for the process: a failed init surfaces ~100ms in as "the pipe is being closed (os error 232)".
pub fn preferred_h264_encoder() -> &'static str {
    // Cache only a WORKING HARDWARE encoder: a software fallback is usually a transient NVENC-session miss, and caching it pinned libx264 for the whole run.
    static CACHED_HW: OnceLock<&'static str> = OnceLock::new();
    if let Some(hw) = CACHED_HW.get() {
        return hw;
    }
    for (name, extra_args) in [
        // Apple Silicon / macOS Hardware Encoder
        ("h264_videotoolbox", &["-realtime", "1"][..]),
        // NVIDIA
        ("h264_nvenc", &["-preset", "p1"][..]),
        // AMD
        ("h264_amf", &["-quality", "speed"][..]),
        // Intel
        ("h264_qsv", &["-preset", "veryfast"][..]),
    ] {
        if probe_encoder(name, extra_args) {
            log::info!("preferred H.264 encoder: {name} (init probe ok)");
            let _ = CACHED_HW.set(name);
            return name;
        }
    }
    log::info!("preferred H.264 encoder: libx264 (no working hardware encoder this attempt)");
    "libx264"
}

/// Real availability of one H.264 encoder on THIS machine. Unlike
/// `ffmpeg -encoders` (which only reports what was *compiled in* — the
/// bundled binaries always ship NVENC/AMF/QSV), `available` reflects an
/// actual 1-frame init probe, so it's true only when the GPU + driver
/// combination can really encode. Surfaced to Settings → About so users
/// can see exactly which hardware acceleration their device supports.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EncoderAvailability {
    /// FFmpeg codec name, e.g. `h264_nvenc`.
    pub name: String,
    /// Human-readable label, e.g. `NVIDIA NVENC`.
    pub label: String,
    /// Vendor family, e.g. `NVIDIA` / `AMD` / `Intel` / `Software`.
    pub vendor: String,
    /// Codec family the row belongs to — `H.264` or `HEVC`. Lets the
    /// diagnostics UI group the matrix into sections.
    pub family: String,
    /// Hardware-accelerated (GPU) vs software (CPU) path.
    pub hardware: bool,
    /// Whether a 1-frame encode actually succeeded on this machine.
    pub available: bool,
    /// The encoder the recorder/export will pick — the highest-priority
    /// available one (mirrors `preferred_h264_encoder`). Only ever set on
    /// an H.264 row, since the recording pipeline is H.264-only today; the
    /// HEVC rows are informational (which HEVC encoders this GPU exposes).
    pub active: bool,
}

/// Probes every encoder candidate for real init success, H.264 then HEVC, each in NVIDIA, AMD, Intel, CPU order.
/// Exactly one entry is `active` (the picked H.264, libx264 always present); HEVC rows are informational. Each hardware probe spawns FFmpeg, so run it off the UI thread.
pub fn probe_recordable_encoders() -> Vec<EncoderAvailability> {
    // (name, label, vendor, family, hardware, extra_args). H.264 first so the `active` lookup lands on the recorder's codec.
    #[allow(clippy::type_complexity)] // one-off literal table; a type alias wouldn't help
    let candidates: [(&str, &str, &str, &str, bool, &[&str]); 10] = [
        (
            "h264_videotoolbox",
            "Apple VideoToolbox",
            "Apple",
            "H.264",
            true,
            &["-realtime", "1"],
        ),
        (
            "h264_nvenc",
            "NVIDIA NVENC",
            "NVIDIA",
            "H.264",
            true,
            &["-preset", "p1"],
        ),
        (
            "h264_amf",
            "AMD AMF",
            "AMD",
            "H.264",
            true,
            &["-quality", "speed"],
        ),
        (
            "h264_qsv",
            "Intel Quick Sync",
            "Intel",
            "H.264",
            true,
            &["-preset", "veryfast"],
        ),
        ("libx264", "x264 (CPU)", "Software", "H.264", false, &[]),
        (
            "hevc_videotoolbox",
            "Apple VideoToolbox",
            "Apple",
            "HEVC",
            true,
            &["-realtime", "1"],
        ),
        (
            "hevc_nvenc",
            "NVIDIA NVENC",
            "NVIDIA",
            "HEVC",
            true,
            &["-preset", "p1"],
        ),
        (
            "hevc_amf",
            "AMD AMF",
            "AMD",
            "HEVC",
            true,
            &["-quality", "speed"],
        ),
        (
            "hevc_qsv",
            "Intel Quick Sync",
            "Intel",
            "HEVC",
            true,
            &["-preset", "veryfast"],
        ),
        ("libx265", "x265 (CPU)", "Software", "HEVC", false, &[]),
    ];

    let mut list: Vec<EncoderAvailability> = candidates
        .into_iter()
        // Probe only encoders that can exist on this OS, avoiding a guaranteed-to-fail spawn and its noisy stderr.
        .filter(|c| encoder_applies_to_platform(c.0))
        .map(|(name, label, vendor, family, hardware, extra)| {
            // libx264 ships in every bundled build and always initializes, so skip its spawn; everything else gets a real probe.
            let available = if name == "libx264" {
                true
            } else {
                probe_encoder(name, extra)
            };
            EncoderAvailability {
                name: name.to_string(),
                label: label.to_string(),
                vendor: vendor.to_string(),
                family: family.to_string(),
                hardware,
                available,
                active: false,
            }
        })
        .collect();

    // Same order as `preferred_h264_encoder`, computed from the probe results so the chain isn't probed twice.
    if let Some(idx) = list.iter().position(|e| e.available) {
        list[idx].active = true;
    }

    list
}

/// Whether an encoder can plausibly exist on the current OS, so we skip a
/// guaranteed-to-fail probe (and its noisy FFmpeg stderr) for the rest.
/// VideoToolbox is macOS-only; NVENC/AMF/QSV don't exist on macOS. Software
/// encoders (libx264/libx265) and anything unrecognized are always probed.
fn encoder_applies_to_platform(name: &str) -> bool {
    let is_mac = cfg!(target_os = "macos");
    if name.contains("videotoolbox") {
        is_mac
    } else if name.contains("nvenc") || name.contains("amf") || name.contains("qsv") {
        !is_mac
    } else {
        true
    }
}

fn probe_encoder(name: &str, extra_args: &[&str]) -> bool {
    let mut command = Command::new(ffmpeg_path());
    command.args([
        "-hide_banner",
        "-loglevel",
        "error",
        "-f",
        "lavfi",
        // 320x240, not 64x64: NVENC enforces a minimum frame size and rejected the tiny probe, reporting every NVENC GPU unavailable.
        "-i",
        "nullsrc=s=320x240:d=0.04",
        "-c:v",
        name,
    ]);
    command.args(extra_args);
    command.args(["-f", "null", "-"]);
    configure_silent_command(&mut command);
    match command.output() {
        Ok(out) if out.status.success() => true,
        Ok(out) => {
            // Expected for hardware this machine can't use: log only the first meaningful line, at debug, so dev runs aren't flooded.
            let reason = String::from_utf8_lossy(&out.stderr)
                .lines()
                .map(str::trim)
                .find(|l| !l.is_empty())
                .unwrap_or("no encoder output")
                .to_string();
            log::debug!("{name} encoder unavailable: {reason}");
            false
        }
        Err(e) => {
            log::warn!("{name} probe could not run: {e}");
            false
        }
    }
}

/// Whether the resolved FFmpeg has `name` as a filter; `-encoders` says nothing about libass, which is a separate `--enable-` flag.
/// Not grounds for rejecting the binary: without libass it still records and exports, and disqualifying it would drop the app to the PATH fallback.
pub fn has_filter(name: &str) -> bool {
    static CACHED: OnceLock<std::collections::HashSet<String>> = OnceLock::new();
    let filters = CACHED.get_or_init(|| {
        let mut command = Command::new(ffmpeg_path());
        command.args(["-hide_banner", "-filters"]);
        configure_silent_command(&mut command);
        match command.output() {
            Ok(out) => parse_filter_names(&String::from_utf8_lossy(&out.stdout)),
            Err(e) => {
                log::warn!("ffmpeg filter probe failed: {e}");
                std::collections::HashSet::new()
            }
        }
    });
    filters.contains(name)
}

/// Pull filter names out of `ffmpeg -filters` stdout.
///
/// Rows look like `.. ass  V->V  Render ASS subtitles...`: flag column, name,
/// then an `in->out` spec. Requiring the arrow is what separates a real row from
/// the legend block at the top (`T.. = Timeline support`), whose lines share the
/// same leading-flag shape.
fn parse_filter_names(stdout: &str) -> std::collections::HashSet<String> {
    stdout
        .lines()
        .filter_map(|line| {
            let mut tokens = line.split_whitespace();
            let _flags = tokens.next()?;
            let name = tokens.next()?;
            tokens.next().filter(|io| io.contains("->"))?;
            Some(name.to_string())
        })
        .collect()
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

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;

    /// Verbatim shape of `ffmpeg -filters` stdout: the legend block first, then
    /// the real rows. Trimmed to the entries the parser has to get right.
    const FILTERS_STDOUT: &str = "\
Filters:
  T.. = Timeline support
  .S. = Slice threading
  ..C = Command support
  A = Audio input/output
  V = Video input/output
  | = Source or sink filter
 ... abench            A->A       Benchmark part of a filtergraph.
 ..C ass               V->V       Render ASS subtitles onto input video using the libass library.
 T.. drawtext          V->V       Draw text on top of video frames using libfreetype library.
 ... subtitles         V->V       Render text subtitles onto input video using the libass library.
 ..C overlay           VV->V      Overlay a video source on top of the input.
 ... color             |->V       Provide an uniformly colored input.
";

    /// The legend rows (`T.. = Timeline support`) have the same leading-flag
    /// shape as a real filter row, so a naive "second token is the name" parse
    /// silently admits `=` as a filter. Keying off the `in->out` arrow is what
    /// keeps them out.
    #[test]
    fn parses_filter_names_and_skips_the_legend() {
        let filters = parse_filter_names(FILTERS_STDOUT);

        for name in ["ass", "subtitles", "drawtext", "overlay", "color", "abench"] {
            assert!(
                filters.contains(name),
                "{name} should be parsed as a filter"
            );
        }
        assert!(
            !filters.contains("="),
            "legend rows must not parse as filters"
        );
        assert!(!filters.contains("Filters:"));
        assert_eq!(filters.len(), 6, "exactly the six real rows");
    }

    /// The libass-less binaries this guard exists for are otherwise complete, so
    /// the absent filter is the ONLY signal. An empty/garbage probe must report
    /// "no such filter" rather than optimistically claiming support.
    #[test]
    fn missing_ass_filter_is_detected() {
        let without_libass = parse_filter_names(
            " ... abench            A->A       Benchmark part of a filtergraph.\n",
        );
        assert!(!without_libass.contains("ass"));
        assert!(parse_filter_names("").is_empty());
    }

    /// The install prefixes must be ABSOLUTE. The whole point is to resolve
    /// ffmpeg when the inherited PATH is minimal (a Finder-launched .app), so a
    /// relative entry here would defeat the fallback.
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    #[test]
    fn common_ffmpeg_dirs_are_absolute_and_non_empty() {
        let dirs = common_ffmpeg_dirs();
        assert!(!dirs.is_empty());
        for dir in dirs {
            assert!(
                Path::new(dir).is_absolute(),
                "{dir} must be absolute to survive a minimal PATH"
            );
        }
    }

    /// Both Homebrew prefixes must be covered: Apple Silicon installs to
    /// /opt/homebrew, Intel to /usr/local. Missing either is the "ffmpeg not
    /// found despite Homebrew" bug on that architecture.
    #[cfg(target_os = "macos")]
    #[test]
    fn macos_probes_both_homebrew_prefixes() {
        let dirs = common_ffmpeg_dirs();
        assert!(
            dirs.contains(&"/opt/homebrew/bin"),
            "Apple Silicon Homebrew"
        );
        assert!(dirs.contains(&"/usr/local/bin"), "Intel Homebrew");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_probes_the_standard_bin_prefixes() {
        let dirs = common_ffmpeg_dirs();
        assert!(dirs.contains(&"/usr/bin"));
        assert!(dirs.contains(&"/usr/local/bin"));
    }

    /// ffmpeg and ffprobe must be taken from the SAME dir, so we never pair a
    /// Homebrew ffmpeg with some other ffprobe.
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    #[test]
    fn system_pair_takes_both_binaries_from_one_dir() {
        let (ffmpeg, ffprobe) = system_ffmpeg_pair("/opt/homebrew/bin");
        assert_eq!(ffmpeg, Path::new("/opt/homebrew/bin/ffmpeg"));
        assert_eq!(ffprobe, Path::new("/opt/homebrew/bin/ffprobe"));
        assert_eq!(ffmpeg.parent(), ffprobe.parent());
    }
}
