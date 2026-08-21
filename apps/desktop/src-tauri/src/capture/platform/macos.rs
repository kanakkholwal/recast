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
//! ## Coordinate model
//!
//! This source captures the WHOLE selected display at its physical
//! resolution and emits full-`source`-sized BGRA frames; the encoder
//! crops to the region/window. That's the same contract the Windows DXGI
//! path follows, so region & window recordings work the same on both.
//!
//! - **Multi-monitor.** The display the user picked is mapped to its
//!   AVFoundation "Capture screen N" via its position in
//!   `CGGetActiveDisplayList` (which xcap's `Monitor::all()` mirrors), so
//!   secondary displays record correctly — see `screen_input_index`.
//! - **Retina.** `recording::apply_device_scale` lifts xcap's logical
//!   `source`/`crop` into the physical pixels AVFoundation delivers (using
//!   `Monitor::scale_factor`), and the cursor track is scaled to match. So
//!   crops land on-target and recordings keep full Retina resolution.
//!
//! ## Known limitations
//!
//! - **Permissions.** First record requires Screen Recording consent in
//!   System Settings → Privacy & Security. FFmpeg will spawn but
//!   produce zero frames until granted; the encoder's empty-output
//!   timeout will surface it.
//! - **Whole-display capture for regions.** Like the Windows path, a small
//!   region on a large display still pipes full-display frames to the
//!   encoder before cropping. Cropping inside AVFoundation would be lighter
//!   but is a cross-platform optimization, not a macOS-specific one.

use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{sync_channel, Receiver};
use std::sync::{Arc, OnceLock};
use std::thread;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use parking_lot::Mutex;

use xcap::Monitor;

use crate::capture::CaptureSource;
use crate::recording::CaptureTarget;

pub fn create_source(target: &CaptureTarget) -> Result<Box<dyn CaptureSource>> {
    if target.source.width == 0 || target.source.height == 0 {
        return Err(anyhow!(
            "macOS capture: source has zero dimensions ({}x{}) — \
             the source picker did not report a usable size",
            target.source.width,
            target.source.height
        ));
    }
    // AVFoundation numbers its "Capture screen N" inputs in CGGetActiveDisplayList
    // order — the same order xcap's `Monitor::all()` returns — so the captured
    // display's position in that list is its screen ordinal. Map the target
    // display to that ordinal instead of always grabbing the first screen, so
    // multi-monitor users record the display they actually picked.
    let ordinal = screen_ordinal_for_display(target.display_id);
    let screen_index = screen_input_index(ordinal).context(
        "no matching 'Capture screen' device in the AVFoundation listing — \
         ensure Screen Recording is granted in System Settings → \
         Privacy & Security and that FFmpeg has avfoundation support",
    )?;
    // Capture the WHOLE display at its physical resolution and let the encoder
    // crop to the requested region. This is the cross-platform CaptureSource
    // contract (the Windows DXGI path does the same): the source always emits
    // full-`source`-sized frames; region/window cropping is the encoder's job.
    // `target.source` is already in physical device pixels (see
    // `apply_device_scale` in recording/mod.rs).
    let source =
        MacosCaptureSource::start(screen_index, target.source.width, target.source.height)?;
    Ok(Box::new(source))
}

struct MacosCaptureSource {
    /// The FFmpeg process. Kept for graceful-stop (`q` on stdin) + kill in
    /// `Drop`; its stdout/stderr are owned by the reader thread.
    child: Child,
    width: u32,
    height: u32,
    /// Freshest decoded frames. Bounded (see `start`) so a momentarily-slow
    /// consumer applies backpressure to FFmpeg instead of buffering unbounded
    /// 8 MB frames.
    rx: Receiver<Vec<u8>>,
    /// Reader thread pulling whole frames off FFmpeg's stdout. Joined in `Drop`
    /// after the child dies (which EOFs its stdout).
    reader: Option<thread::JoinHandle<()>>,
    /// Set by the reader thread when FFmpeg exits/errors, so `capture_next` can
    /// report the real cause (typically a missing Screen Recording grant)
    /// instead of a generic channel disconnect.
    error: Arc<Mutex<Option<String>>>,
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
        // Normalize the captured display to the exact (physical) `source` size
        // the encoder declares, then stop — we deliberately do NOT crop here.
        // Region/window cropping is the encoder's job (`crop_relative_to_source`),
        // so this source always emits full-`source`-sized frames like every
        // other platform's CaptureSource. When AVFoundation already delivers at
        // `width`x`height` (the common case) swscale fast-paths the no-op.
        //
        // (History: this filter used to also `crop=W:H:X:Y` to the region and
        // emit crop-sized frames, which collided with the encoder's own crop and
        // corrupted region/window recordings — frames were the wrong byte size.)
        let filter = format!("scale={width}:{height}");
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
                &filter,
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
        let mut child = command
            .spawn()
            .context("failed to spawn FFmpeg avfoundation screen capture")?;
        let frame_bytes = (width as usize) * (height as usize) * 4;

        let mut stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow!("avfoundation FFmpeg stdout pipe missing"))?;
        // Drained continuously rather than only at EOF: this child lives for the
        // whole recording, and a full stderr pipe stalls it mid-capture.
        let stderr = child.stderr.take().map(crate::ffmpeg::StderrTail::spawn);

        // Small bounded buffer: the pacer drains several frames per tick, so a
        // depth of 2 keeps the freshest pixels available without letting a
        // warmup stall pile up hundreds of MB of frames. A full buffer blocks
        // the reader's `send`, which backpressures FFmpeg's stdout pipe.
        let (tx, rx) = sync_channel::<Vec<u8>>(2);
        let error = Arc::new(Mutex::new(None));

        // Read whole frames on a dedicated thread so `capture_next` is a
        // cancellable channel `recv` rather than a blocking pipe read. The old
        // in-line blocking read IGNORED its timeout and never re-checked the
        // stop flag mid-read, so when FFmpeg produced no frames (missing Screen
        // Recording permission) the capture thread hung and `stop()`'s `join()`
        // hung with it — the Stop button looked dead and the user mashed it.
        let reader = {
            let error = error.clone();
            thread::Builder::new()
                .name("recast-macos-capture-reader".into())
                .spawn(move || {
                    let mut buf = vec![0u8; frame_bytes];
                    loop {
                        let mut read = 0usize;
                        while read < frame_bytes {
                            match stdout.read(&mut buf[read..]) {
                                Ok(0) => {
                                    // EOF. Mid-frame ⇒ FFmpeg died unexpectedly;
                                    // on a frame boundary it's usually a clean
                                    // stop (Drop killed it), but surface stderr
                                    // either way so a permission denial that
                                    // prints-then-exits isn't swallowed.
                                    let msg = read_stderr(stderr.as_ref());
                                    if read != 0 {
                                        *error.lock() = Some(format!(
                                            "avfoundation capture exited mid-frame \
                                             ({read}/{frame_bytes} bytes): {}",
                                            if msg.is_empty() {
                                                "<no stderr — check Screen \
                                                 Recording permission>"
                                                    .to_string()
                                            } else {
                                                msg
                                            }
                                        ));
                                    } else if !msg.is_empty() {
                                        *error.lock() =
                                            Some(format!("avfoundation capture ended: {msg}"));
                                    }
                                    return;
                                }
                                Ok(n) => read += n,
                                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                                Err(e) => {
                                    *error.lock() =
                                        Some(format!("avfoundation stdout read failed: {e}"));
                                    return;
                                }
                            }
                        }
                        // Full frame. `send` blocks while the bounded buffer is
                        // full (backpressure) and errors once the receiver is
                        // dropped (capture stopped) — either way we stop cleanly.
                        if tx.send(buf.clone()).is_err() {
                            return;
                        }
                    }
                })
                .context("failed to spawn avfoundation reader thread")?
        };

        Ok(Self {
            child,
            width,
            height,
            rx,
            reader: Some(reader),
            error,
        })
    }

    /// The reason the reader thread ended, if it recorded one — otherwise a
    /// generic permission-hint fallback.
    fn ended_error(&self) -> anyhow::Error {
        let msg = self.error.lock().take();
        anyhow!(msg.unwrap_or_else(|| {
            "avfoundation capture ended unexpectedly — grant Screen Recording in \
             System Settings → Privacy & Security, then record again"
                .to_string()
        }))
    }
}

impl CaptureSource for MacosCaptureSource {
    fn capture_next(&mut self, timeout: Duration) -> Result<Option<Vec<u8>>> {
        use std::sync::mpsc::{RecvTimeoutError, TryRecvError};
        // A disconnected channel means the reader thread ended (FFmpeg
        // exited/errored), so we surface its recorded reason instead of
        // blocking. `timeout == 0` is the pacer's non-blocking drain.
        if timeout.is_zero() {
            match self.rx.try_recv() {
                Ok(frame) => Ok(Some(frame)),
                Err(TryRecvError::Empty) => Ok(None),
                Err(TryRecvError::Disconnected) => Err(self.ended_error()),
            }
        } else {
            match self.rx.recv_timeout(timeout) {
                Ok(frame) => Ok(Some(frame)),
                Err(RecvTimeoutError::Timeout) => Ok(None),
                Err(RecvTimeoutError::Disconnected) => Err(self.ended_error()),
            }
        }
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
        // Mirror the camera backend's graceful-stop: write `q` to ask FFmpeg to
        // exit cleanly, escalate to kill if it doesn't.
        if let Some(mut stdin) = self.child.stdin.take() {
            use std::io::Write;
            let _ = stdin.write_all(b"q");
            let _ = stdin.flush();
        }
        let mut exited = false;
        for _ in 0..40 {
            if matches!(self.child.try_wait(), Ok(Some(_))) {
                exited = true;
                break;
            }
            thread::sleep(Duration::from_millis(50));
        }
        if !exited {
            log::warn!("avfoundation capture did not exit gracefully — killing");
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
        // The child is dead, so its stdout is at EOF. Drain any buffered frames
        // first to release a reader parked on a full `send`, then join it.
        if let Some(handle) = self.reader.take() {
            while self.rx.try_recv().is_ok() {}
            let _ = handle.join();
        }
    }
}

fn read_stderr(tail: Option<&crate::ffmpeg::StderrTail>) -> String {
    let s = tail.map(|t| t.snapshot()).unwrap_or_default();
    if s.len() <= 500 {
        return s;
    }
    // Keep the END — the failure reason is the last thing FFmpeg printed, not
    // the banner. Back off to a char boundary; lossy decoding can leave a
    // multi-byte char straddling the cut.
    let mut cut = s.len() - 500;
    while cut < s.len() && !s.is_char_boundary(cut) {
        cut += 1;
    }
    s[cut..].to_string()
}

/// Ordinal (0-based position in `CGGetActiveDisplayList`) of the display with
/// the given `CGDirectDisplayID`. xcap's `Monitor::all()` enumerates in exactly
/// that order, so the index in the returned list is the AVFoundation "Capture
/// screen N" number. Falls back to 0 (the primary) if the id isn't found.
fn screen_ordinal_for_display(display_id: u32) -> u32 {
    Monitor::all()
        .ok()
        .and_then(|monitors| {
            monitors
                .iter()
                .position(|m| m.id().ok() == Some(display_id))
        })
        .map(|pos| pos as u32)
        .unwrap_or(0)
}

/// Cached `(screen_ordinal, avfoundation_input_index)` pairs for this process.
/// The parsing lives in `super::parse_capture_screen_listing` (pure +
/// unit-tested on every host); here we just feed it the cached device listing,
/// shared with the camera/audio probes via `ffmpeg::cached_avfoundation_devices`
/// so the FFmpeg listing spawn runs at most once per launch.
fn capture_screen_indices() -> &'static [(u32, u32)] {
    static CACHED: OnceLock<Vec<(u32, u32)>> = OnceLock::new();
    CACHED.get_or_init(|| {
        let stderr = crate::ffmpeg::cached_avfoundation_devices();
        super::parse_capture_screen_listing(&stderr)
    })
}

/// AVFoundation input index for the "Capture screen {ordinal}" device, falling
/// back to the lowest-ordinal screen when the exact ordinal isn't listed (so a
/// stale display arrangement still records *something* rather than failing).
fn screen_input_index(ordinal: u32) -> Result<u32> {
    let map = capture_screen_indices();
    if let Some((_, idx)) = map.iter().find(|(ord, _)| *ord == ordinal) {
        return Ok(*idx);
    }
    map.iter()
        .min_by_key(|(ord, _)| *ord)
        .map(|(_, idx)| *idx)
        .ok_or_else(|| anyhow!("no 'Capture screen' device in AVFoundation listing"))
}
