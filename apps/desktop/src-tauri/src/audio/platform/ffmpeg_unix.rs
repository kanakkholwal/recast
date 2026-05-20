//! Audio + microphone capture for macOS (AVFoundation) and Linux
//! (PulseAudio) via FFmpeg.
//!
//! ## Why FFmpeg instead of native APIs
//!
//! - **macOS:** there is no built-in loopback API for system audio
//!   prior to ScreenCaptureKit (macOS 13+). Until we ship a
//!   ScreenCaptureKit backend, the only paths are AVFoundation (for
//!   mic) and a virtual-loopback driver (BlackHole, Soundflower,
//!   VB-Cable) for system audio. FFmpeg knows how to talk to all of
//!   these, and routing through one subprocess instead of three
//!   different `objc2` integrations keeps the surface area tiny.
//! - **Linux:** PulseAudio (and pipewire-pulse) expose any output sink
//!   as a `.monitor` source for loopback. FFmpeg's `pulse` input does
//!   the heavy lifting; we just have to discover the default sink name.
//!
//! ## Pause semantics
//!
//! The Windows WASAPI backend drains the device every tick but only
//! *writes samples* when `pause_flag` is clear — producing a gap-free
//! WAV across recording pauses. We mirror that exactly: FFmpeg streams
//! PCM `s16le` to its stdout, this thread reads in 8 KB chunks, and
//! `pause_flag` selectively suppresses `WavWriter::write_samples`. The
//! pipe is always drained even while paused so FFmpeg doesn't back-
//! pressure and stall.
//!
//! ## Loopback graceful-degradation
//!
//! macOS without a loopback driver and Linux without PulseAudio cannot
//! capture system audio. Rather than failing the whole recording, we
//! detect the absence at `start()` time, log a clear actionable warning,
//! and fall back to writing a silence WAV (matches the original
//! `fallback.rs` behaviour). Microphone capture has no such fallback —
//! a user who explicitly enables the mic and lacks a working device
//! gets an error, which is the correct UX.

use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use parking_lot::Mutex;

use crate::audio::wav::WavWriter;
use crate::audio::{AudioCaptureConfig, MicrophoneCaptureConfig};

// Target PCM format we ask FFmpeg to emit + write into the WAV.
// 48 kHz / 2 ch / 16-bit s16le is what every consumer of the
// recorded WAV downstream supports and matches the editor's mux
// defaults. Float32 would marginally lower quantisation noise; nothing
// the editor does is sensitive to it.
const PCM_SAMPLE_RATE: u32 = 48_000;
const PCM_CHANNELS: u16 = 2;
const PCM_BITS: u16 = 16;

// -- System audio (loopback) -------------------------------------------------

/// Backend variant for `PlatformAudioSession`. The macOS happy path is
/// `Sckit` (ScreenCaptureKit — no virtual driver needed); `Live` runs
/// FFmpeg against a virtual loopback device (BlackHole on macOS, the
/// default sink monitor on Linux); `Silence` writes a silence WAV when
/// no loopback source is reachable.
enum LoopbackBackend {
    Live {
        stop_flag: Arc<AtomicBool>,
        thread_handle: JoinHandle<Result<PathBuf>>,
    },
    #[cfg(target_os = "macos")]
    Sckit {
        session: super::macos_sckit::ScKitLoopback,
    },
    Silence {
        output_path: PathBuf,
        started_at: Instant,
    },
}

pub struct PlatformAudioSession {
    backend: LoopbackBackend,
}

impl PlatformAudioSession {
    pub fn start(config: AudioCaptureConfig) -> Result<Self> {
        // macOS-first: try ScreenCaptureKit before any FFmpeg path. SCKit
        // is the only built-in macOS API for system-audio loopback (no
        // virtual driver required) and is what every modern recorder
        // uses. If SCKit isn't available — macOS < 13, Screen Recording
        // not granted, framework error — fall through to BlackHole /
        // silence so the rest of the recording still proceeds.
        #[cfg(target_os = "macos")]
        {
            let sckit_path = config.output_path.clone();
            let sckit_pause = config.pause_flag.clone();
            match super::macos_sckit::ScKitLoopback::try_start(sckit_path, sckit_pause) {
                Ok(session) => {
                    return Ok(Self {
                        backend: LoopbackBackend::Sckit { session },
                    });
                }
                Err(e) => {
                    log::info!(
                        "ScreenCaptureKit loopback unavailable ({e}) — \
                         checking for a virtual loopback driver (BlackHole / Soundflower / etc.)"
                    );
                }
            }
        }

        let output_path = config.output_path.clone();
        let pause_flag = config.pause_flag.clone();

        match detect_loopback_source() {
            Some(source) => {
                let stop_flag = Arc::new(AtomicBool::new(false));
                let flag_for_thread = stop_flag.clone();
                let label = "system-audio loopback";
                let source_summary = format!("{} '{}'", source.format, source.device);
                let thread_handle = thread::Builder::new()
                    .name("recast-audio-loopback".into())
                    .spawn(move || {
                        run_pcm_capture(
                            output_path,
                            source,
                            pause_flag,
                            flag_for_thread,
                            label,
                        )
                    })
                    .context("failed to spawn loopback capture thread")?;
                log::info!(
                    "system-audio loopback capture started via {source_summary}, output: {}",
                    config.output_path.display()
                );
                Ok(Self {
                    backend: LoopbackBackend::Live {
                        stop_flag,
                        thread_handle,
                    },
                })
            }
            None => {
                log::warn!(
                    "{}",
                    loopback_unavailable_message()
                );
                Ok(Self {
                    backend: LoopbackBackend::Silence {
                        output_path: config.output_path,
                        started_at: Instant::now(),
                    },
                })
            }
        }
    }

    pub fn stop(self) -> Result<PathBuf> {
        match self.backend {
            LoopbackBackend::Live {
                stop_flag,
                thread_handle,
            } => {
                stop_flag.store(true, Ordering::Release);
                thread_handle
                    .join()
                    .map_err(|_| anyhow!("audio capture thread panicked"))?
            }
            #[cfg(target_os = "macos")]
            LoopbackBackend::Sckit { session } => session.stop(),
            LoopbackBackend::Silence {
                output_path,
                started_at,
            } => {
                let duration = started_at.elapsed().as_secs_f64();
                crate::audio::wav::write_silence_wav(
                    &output_path,
                    PCM_SAMPLE_RATE,
                    PCM_CHANNELS,
                    duration,
                )?;
                Ok(output_path)
            }
        }
    }
}

// -- Microphone --------------------------------------------------------------

pub struct PlatformMicrophoneSession {
    stop_flag: Arc<AtomicBool>,
    thread_handle: JoinHandle<Result<PathBuf>>,
}

impl PlatformMicrophoneSession {
    pub fn start(config: MicrophoneCaptureConfig) -> Result<Self> {
        let stop_flag = Arc::new(AtomicBool::new(false));
        let flag_for_thread = stop_flag.clone();
        let output_path = config.output_path.clone();
        let pause_flag = config.pause_flag.clone();
        let source = resolve_microphone_source(&config.device_id);

        let source_summary = format!("{} '{}'", source.format, source.device);
        let thread_handle = thread::Builder::new()
            .name("recast-microphone".into())
            .spawn(move || {
                run_pcm_capture(
                    output_path,
                    source,
                    pause_flag,
                    flag_for_thread,
                    "microphone",
                )
            })
            .context("failed to spawn microphone capture thread")?;

        log::info!(
            "microphone capture started via {source_summary}, output: {}",
            config.output_path.display()
        );

        Ok(Self {
            stop_flag,
            thread_handle,
        })
    }

    pub fn stop(self) -> Result<PathBuf> {
        self.stop_flag.store(true, Ordering::Release);
        self.thread_handle
            .join()
            .map_err(|_| anyhow!("microphone capture thread panicked"))?
    }
}

// -- Shared FFmpeg PCM capture ----------------------------------------------

#[derive(Clone)]
struct PcmSource {
    /// FFmpeg input format keyword (`avfoundation`, `pulse`, `alsa`, ...).
    format: &'static str,
    /// FFmpeg input device argument matching the format keyword.
    device: String,
}

/// Spawn FFmpeg writing raw little-endian PCM to stdout, copy the
/// stream into a WAV file (honouring `pause_flag`), and clean up the
/// subprocess + watcher thread on stop.
fn run_pcm_capture(
    output_path: PathBuf,
    source: PcmSource,
    pause_flag: Arc<AtomicBool>,
    stop_flag: Arc<AtomicBool>,
    label: &'static str,
) -> Result<PathBuf> {
    let mut command = Command::new(crate::ffmpeg::ffmpeg_path());
    command
        .args([
            "-hide_banner",
            "-loglevel",
            "warning",
            "-f",
            source.format,
            "-i",
            &source.device,
            // Re-sample to a fixed target so downstream consumers don't
            // need a per-recording sample-rate detector.
            "-ac",
            &PCM_CHANNELS.to_string(),
            "-ar",
            &PCM_SAMPLE_RATE.to_string(),
            "-f",
            "s16le",
            "-",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    crate::ffmpeg::configure_silent_command(&mut command);

    let mut child = command
        .spawn()
        .with_context(|| format!("failed to start FFmpeg {label} capture"))?;

    let stdout = child
        .stdout
        .take()
        .context("FFmpeg stdout pipe missing — cannot read PCM stream")?;

    let mut writer = WavWriter::new(&output_path, PCM_SAMPLE_RATE, PCM_CHANNELS, PCM_BITS)
        .with_context(|| format!("failed to create {label} WAV writer"))?;

    // The child is owned by both threads (read loop + stop watcher) so
    // either can kill it. parking_lot::Mutex<Option<…>> models the
    // "exactly one of you will tear this down" contract.
    let child_lock: Arc<Mutex<Option<Child>>> = Arc::new(Mutex::new(Some(child)));

    // Watcher: blocks on the stop flag, then asks FFmpeg to quit
    // gracefully so any in-flight encoder buffer is flushed. The
    // graceful path matters less for raw-PCM output than for the MP4
    // muxer in the camera backend (PCM has no header to write at
    // close), but it's cheap and consistent.
    let watcher = spawn_stop_watcher(stop_flag.clone(), child_lock.clone(), label);

    // Copy stdout → WAV. EOF terminates the loop (signaled by the
    // watcher killing FFmpeg, or by FFmpeg exiting on its own).
    let mut reader = std::io::BufReader::new(stdout);
    let mut buf = vec![0u8; 8192];
    loop {
        match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                // Drain the pipe even when paused so FFmpeg never
                // blocks on a full output buffer; just suppress the
                // WAV write to keep the file gap-free.
                if !pause_flag.load(Ordering::Acquire) {
                    if let Err(e) = writer.write_samples(&buf[..n]) {
                        log::error!("{label} WAV write failed: {e}");
                        break;
                    }
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => {
                log::warn!("{label} FFmpeg stdout read error: {e}");
                break;
            }
        }
    }

    // Ensure both the watcher and the child are wound down before we
    // touch the WAV finaliser — the child may still hold the output
    // file's stdin pipe open on some platforms.
    stop_flag.store(true, Ordering::Release);
    let _ = watcher.join();
    if let Some(mut c) = child_lock.lock().take() {
        let _ = c.kill();
        let _ = c.wait();
    }

    writer
        .finish()
        .with_context(|| format!("failed to finalise {label} WAV"))?;
    log::info!("{label} capture finished: {}", output_path.display());
    Ok(output_path)
}

fn spawn_stop_watcher(
    stop_flag: Arc<AtomicBool>,
    child_lock: Arc<Mutex<Option<Child>>>,
    label: &'static str,
) -> thread::JoinHandle<()> {
    thread::Builder::new()
        .name(format!("recast-{label}-watcher").replace(' ', "-"))
        .spawn(move || {
            // Poll instead of using a condvar — the read loop in
            // `run_pcm_capture` is the hot path, and a 50 ms tick on
            // shutdown is imperceptible.
            loop {
                if stop_flag.load(Ordering::Acquire) {
                    break;
                }
                thread::sleep(Duration::from_millis(50));
            }
            let mut slot = child_lock.lock();
            let Some(mut c) = slot.take() else { return };
            // Graceful: tell FFmpeg to quit. PCM has no muxer epilogue
            // to flush, but this still lets FFmpeg log a clean
            // shutdown line instead of "ERROR: signal interrupted".
            if let Some(mut stdin) = c.stdin.take() {
                let _ = stdin.write_all(b"q");
                let _ = stdin.flush();
            }
            for _ in 0..20 {
                if matches!(c.try_wait(), Ok(Some(_))) {
                    return;
                }
                thread::sleep(Duration::from_millis(50));
            }
            log::warn!("FFmpeg {label} did not exit on quit — killing");
            let _ = c.kill();
            let _ = c.wait();
        })
        .expect("watcher thread spawn should not fail")
}

// -- Source detection (platform-specific) -----------------------------------

#[cfg(target_os = "macos")]
fn detect_loopback_source() -> Option<PcmSource> {
    // macOS has no native loopback. Look for a virtual driver the user
    // may have installed: BlackHole (most common today), Soundflower
    // (legacy), VB-Cable (Windows port that also runs on macOS), or
    // anything named "loopback".
    let stderr = list_avfoundation_devices().ok()?;
    let mut in_audio = false;
    for line in stderr.lines() {
        if line.contains("video devices:") {
            in_audio = false;
            continue;
        }
        if line.contains("audio devices:") {
            in_audio = true;
            continue;
        }
        if !in_audio {
            continue;
        }
        let lower = line.to_ascii_lowercase();
        let is_loopback = ["blackhole", "soundflower", "loopback", "vb-cable", "vb cable"]
            .iter()
            .any(|n| lower.contains(n));
        if !is_loopback {
            continue;
        }
        let idx = avfoundation_index(line)?;
        return Some(PcmSource {
            format: "avfoundation",
            // AVFoundation audio-only input spec: ":<index>"
            device: format!(":{idx}"),
        });
    }
    None
}

#[cfg(target_os = "linux")]
fn detect_loopback_source() -> Option<PcmSource> {
    // PulseAudio (and pipewire-pulse) exposes a `.monitor` source for
    // every output sink. We query the default sink name and append
    // `.monitor`. If `pactl` isn't installed or PA isn't running, return
    // None so the caller falls back to silence.
    let output = Command::new("pactl")
        .args(["get-default-sink"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let sink = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if sink.is_empty() {
        return None;
    }
    Some(PcmSource {
        format: "pulse",
        device: format!("{sink}.monitor"),
    })
}

#[cfg(target_os = "macos")]
fn resolve_microphone_source(device_id: &Option<String>) -> PcmSource {
    let arg = match device_id.as_deref() {
        Some(id) if !id.trim().is_empty() && !id.eq_ignore_ascii_case("default") => {
            // User-supplied identifier — could be a numeric index or a
            // device name; AVFoundation accepts either. Prefix with ':'
            // because audio inputs live on the audio side of the
            // "video:audio" spec.
            format!(":{}", id.trim())
        }
        _ => ":0".to_string(),
    };
    PcmSource {
        format: "avfoundation",
        device: arg,
    }
}

#[cfg(target_os = "linux")]
fn resolve_microphone_source(device_id: &Option<String>) -> PcmSource {
    let arg = match device_id.as_deref() {
        Some(id) if !id.trim().is_empty() && !id.eq_ignore_ascii_case("default") => {
            id.trim().to_string()
        }
        _ => "default".to_string(),
    };
    PcmSource {
        format: "pulse",
        device: arg,
    }
}

#[cfg(target_os = "macos")]
fn loopback_unavailable_message() -> &'static str {
    "system-audio loopback not available on this macOS install — \
     recording silence on the system-audio track. To capture system \
     audio, install BlackHole (https://github.com/ExistentialAudio/BlackHole) \
     and route system output through it, or wait for the ScreenCaptureKit \
     backend (macOS 13+, in a future build)."
}

#[cfg(target_os = "linux")]
fn loopback_unavailable_message() -> &'static str {
    "system-audio loopback not available — `pactl` did not return a default \
     sink. Ensure PulseAudio (or pipewire-pulse) is running, or install \
     pulseaudio-utils. Recording silence on the system-audio track."
}

#[cfg(target_os = "macos")]
fn list_avfoundation_devices() -> Result<String> {
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
        .output()?;
    Ok(String::from_utf8_lossy(&output.stderr).into_owned())
}

#[cfg(target_os = "macos")]
fn avfoundation_index(line: &str) -> Option<u32> {
    let bytes = line.as_bytes();
    let close = bytes.iter().rposition(|&b| b == b']')?;
    let open = bytes[..close].iter().rposition(|&b| b == b'[')?;
    let inner = std::str::from_utf8(&bytes[open + 1..close]).ok()?;
    inner.trim().parse::<u32>().ok()
}
