//! macOS screen capture via FFmpeg AVFoundation.
//!
//! Replaces the xcap fallback (which on macOS reopens a CoreGraphics
//! session per frame — orders of magnitude slower than necessary) with
//! a single long-lived FFmpeg subprocess that streams raw BGRA frames
//! to stdout. Each `capture_next()` reads exactly one frame's worth of
//! bytes from the pipe.
//!
//! ## Why not ScreenCaptureKit
//!
//! ScreenCaptureKit (macOS 13+) is the right native source — it's
//! lower-latency, includes a system-audio tap, and is what Apple
//! recommends for any new screen recorder. But wiring it up requires:
//!   - non-trivial objc2 bindings for `SCStream`, `SCContentFilter`,
//!     `SCStreamConfiguration`, the async stream delegate, the audio
//!     output coupling …
//!   - a TCC permission scaffolding flow for first-run consent
//!   - testing on macOS 13/14/15 to catch the API renames per release
//!
//! Each of those is its own multi-day landing. FFmpeg AVFoundation
//! ships today on macOS 11+, performs well enough for 1080p60, and
//! shares all the existing infrastructure (binary path resolution,
//! `configure_silent_command`, the encoder downstream). It's the
//! pragmatic bridge until SCKit lands.
//!
//! ## Known limitations
//!
//! - **First screen only.** We pick the lowest-numbered "Capture screen
//!   N" device. Multi-monitor users who selected a secondary display in
//!   the in-app picker get the primary instead. Mapping xcap monitor
//!   IDs to AVFoundation indices is a follow-up.
//! - **No region capture.** `target.crop.{x,y}` are ignored;
//!   AVFoundation captures the whole screen and `-vf scale=W:H` resizes
//!   to the requested dims. Cropping in software is straightforward
//!   (`-vf crop=...,scale=...`) but the picker doesn't surface
//!   region+display selection yet on macOS.
//! - **Permissions.** First record requires Screen Recording consent in
//!   System Settings → Privacy & Security. FFmpeg will spawn but
//!   produce zero frames until granted; the encoder's empty-output
//!   timeout will surface it.

use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::sync::OnceLock;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};

use crate::capture::CaptureSource;
use crate::recording::CaptureTarget;

pub fn create_source(target: &CaptureTarget) -> Result<Box<dyn CaptureSource>> {
    if target.crop.width == 0 || target.crop.height == 0 {
        return Err(anyhow!(
            "macOS capture: target has zero dimensions ({}x{}) — \
             the source picker did not report a usable size",
            target.crop.width,
            target.crop.height
        ));
    }
    let screen_index = first_screen_index().context(
        "no 'Capture screen' device in the AVFoundation listing — \
         ensure Screen Recording is granted in System Settings → \
         Privacy & Security and that FFmpeg has avfoundation support",
    )?;
    let source = MacosCaptureSource::start(screen_index, target.crop.width, target.crop.height)?;
    Ok(Box::new(source))
}

struct MacosCaptureSource {
    child: Child,
    width: u32,
    height: u32,
    frame_bytes: usize,
    buf: Vec<u8>,
}

impl MacosCaptureSource {
    fn start(screen_index: u32, width: u32, height: u32) -> Result<Self> {
        // The pacer in `recording/pipeline.rs` runs at a fixed
        // `target_fps`. Asking AVFoundation for a slightly higher rate
        // (60) leaves slack for the pacer's MAX_DRAIN to pick the
        // freshest frame rather than emit a stale cached one.
        let request_fps = 60u32;
        // AVFoundation's input string: "<video>:<audio>". We do not
        // capture audio here (audio comes from `audio/platform/ffmpeg_unix.rs`),
        // so the audio side stays empty.
        let input = format!("{screen_index}:");
        // Force the output frame geometry to match what the encoder
        // expects, regardless of the screen's native resolution.
        let scale_filter = format!("scale={width}:{height}");
        let mut command = Command::new(crate::ffmpeg::ffmpeg_path());
        command
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-f",
                "avfoundation",
                // Draw the OS cursor into the captured frames. Mirrors
                // CursorMode::Embedded on the Wayland path so the
                // editor's stylized cursor lands on top of a real
                // pixel-baked cursor (we record positions separately).
                "-capture_cursor",
                "1",
                "-framerate",
                &request_fps.to_string(),
                "-i",
                &input,
                "-vf",
                &scale_filter,
                "-pix_fmt",
                "bgra",
                "-f",
                "rawvideo",
                "-",
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        crate::ffmpeg::configure_silent_command(&mut command);
        let child = command
            .spawn()
            .context("failed to spawn FFmpeg avfoundation screen capture")?;
        let frame_bytes = (width as usize) * (height as usize) * 4;
        // Pre-allocate the frame buffer once. `capture_next` reads into
        // this slice and clones it on success; the alloc cost stays out
        // of the per-frame hot path.
        Ok(Self {
            child,
            width,
            height,
            frame_bytes,
            buf: vec![0u8; frame_bytes],
        })
    }
}

impl CaptureSource for MacosCaptureSource {
    fn capture_next(&mut self, _timeout: Duration) -> Result<Option<Vec<u8>>> {
        // Same shape as `X11CaptureSource::capture_next` — the pacer's
        // `MAX_DRAIN` cap keeps us from over-capturing, so we just do a
        // blocking `read_exact`-style pull of one whole frame.
        let stdout = self
            .child
            .stdout
            .as_mut()
            .ok_or_else(|| anyhow!("avfoundation FFmpeg stdout pipe missing"))?;
        let mut read = 0usize;
        while read < self.frame_bytes {
            match stdout.read(&mut self.buf[read..]) {
                Ok(0) => {
                    // FFmpeg closed stdout mid-frame — fetch stderr for
                    // the actual error before propagating.
                    let stderr = read_child_stderr(&mut self.child);
                    return Err(anyhow!(
                        "avfoundation capture exited mid-frame ({}/{} bytes read): {}",
                        read,
                        self.frame_bytes,
                        if stderr.is_empty() {
                            "<no stderr — check Screen Recording permission>".to_string()
                        } else {
                            stderr
                        }
                    ));
                }
                Ok(n) => read += n,
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(anyhow!("avfoundation stdout read failed: {e}")),
            }
        }
        // Clone so the buffer can be reused for the next read while the
        // pipeline owns this frame. The alloc dominates 1080p frame
        // cost (~8 MB) but the underlying frame copy was already
        // unavoidable — FFmpeg writes into our slice, we hand a Vec
        // downstream.
        Ok(Some(self.buf.clone()))
    }

    fn width(&self) -> u32 {
        self.width
    }

    fn height(&self) -> u32 {
        self.height
    }
}

impl Drop for MacosCaptureSource {
    fn drop(&mut self) {
        // Mirror the camera backend's graceful-stop: write `q` to ask
        // FFmpeg to exit cleanly, escalate to kill if it doesn't.
        if let Some(mut stdin) = self.child.stdin.take() {
            use std::io::Write;
            let _ = stdin.write_all(b"q");
            let _ = stdin.flush();
        }
        for _ in 0..40 {
            if matches!(self.child.try_wait(), Ok(Some(_))) {
                return;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        log::warn!("avfoundation capture did not exit gracefully — killing");
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn read_child_stderr(child: &mut Child) -> String {
    let mut s = String::new();
    if let Some(ref mut e) = child.stderr {
        let _ = e.read_to_string(&mut s);
    }
    if s.len() > 500 {
        s.truncate(500);
    }
    s
}

/// First AVFoundation "Capture screen N" device index. Cached for the
/// process lifetime because the listing is stable and the FFmpeg probe
/// it runs costs ~200 ms cold.
fn first_screen_index() -> Result<u32> {
    static CACHED: OnceLock<Option<u32>> = OnceLock::new();
    let cached = CACHED.get_or_init(|| {
        let output = Command::new(crate::ffmpeg::ffmpeg_path())
            .args([
                "-hide_banner",
                "-f",
                "avfoundation",
                "-list_devices",
                "true",
                "-i",
                "",
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .stdin(Stdio::null())
            .output()
            .ok()?;
        let stderr = String::from_utf8_lossy(&output.stderr);
        let mut in_video = false;
        for line in stderr.lines() {
            if line.contains("video devices:") {
                in_video = true;
                continue;
            }
            if line.contains("audio devices:") {
                in_video = false;
                continue;
            }
            if !in_video {
                continue;
            }
            // "Capture screen 0", "Capture screen 1", ...
            let lower = line.to_ascii_lowercase();
            if !lower.contains("capture screen") {
                continue;
            }
            // The AVFoundation device index is in the LAST bracket pair
            // on the line — skipping the leading
            // `[AVFoundation indev @ 0x...]` log prefix.
            let bytes = line.as_bytes();
            let close = bytes.iter().rposition(|&b| b == b']')?;
            let open = bytes[..close].iter().rposition(|&b| b == b'[')?;
            let inner = std::str::from_utf8(&bytes[open + 1..close]).ok()?;
            if let Ok(n) = inner.trim().parse::<u32>() {
                return Some(n);
            }
        }
        None
    });
    cached.ok_or_else(|| anyhow!("no 'Capture screen' device in AVFoundation listing"))
}
