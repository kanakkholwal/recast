//! macOS system-audio loopback via ScreenCaptureKit.
//!
//! SCKit is the only built-in macOS API for system-audio capture
//! without a virtual driver (BlackHole / Soundflower / VB-Cable).
//! It shipped audio support in macOS 13; on earlier versions
//! `try_start` returns `Err` and the caller falls through to the
//! virtual-driver detection in `ffmpeg_unix::PlatformAudioSession`.
//!
//! ## How it works
//!
//! 1. We discover the first available display via `SCShareableContent`
//!    (SCKit needs *some* content filter even for an audio-only capture).
//! 2. Build an `SCContentFilter` with that display and no excluded windows.
//! 3. Configure `SCStreamConfiguration` to:
//!    - `captures_audio = true` — request system-audio samples,
//!    - `excludes_current_process_audio = true` — drop the app's own
//!      sound output so the recording isn't a feedback loop,
//!    - 48 kHz / 2 ch — matches the WAV writer used by the rest of the
//!      pipeline, no resampling needed.
//! 4. Register an `SCStreamOutputTrait` handler for the Audio output
//!    type; the handler converts each `CMSampleBuffer`'s Float32 PCM
//!    into s16le and appends to the project's WAV.
//! 5. A no-op Screen handler is registered too — SCKit's audio-only
//!    streams still emit Screen sample buffers (it's a single stream)
//!    and not draining them would back-pressure the pipeline.
//!
//! ## Pause semantics
//!
//! Identical to the Windows WASAPI path: while `pause_flag` is set the
//! handler drops samples (the SCKit stream keeps running, so we never
//! starve its dispatch queue). This produces a gap-free WAV across
//! recording pause/resume the same way the other backends do.
//!
//! ## Permissions
//!
//! SCKit requires the **Screen Recording** TCC permission — not "audio
//! input", because the audio path is bolted onto the screen-capture
//! API. The user grants this once via System Settings → Privacy &
//! Security → Screen Recording. We add the `NSScreenCaptureUsageDescription`
//! key via `tauri.conf.json`'s macOS Info.plist so macOS shows our
//! human-readable explanation when prompting.
//!
//! ## Status
//!
//! **Compile-tested only.** This code was written from the
//! `screencapturekit` 6.x docs and examples; it has not been run on
//! macOS hardware yet. The runtime smoke-test checklist lives at the
//! bottom of this file. Until that test pass lands, behaviour on real
//! Macs is unverified — but the fallback chain in
//! `ffmpeg_unix::PlatformAudioSession::start` already handles SCKit
//! failure gracefully (logs an info line, then tries BlackHole, then
//! silence), so a broken SCKit integration degrades rather than blocks.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use parking_lot::Mutex;
use screencapturekit::prelude::*;
// `AudioBufferList` is re-exported from the crate root but intentionally
// not in the prelude (it's a less-common type). The buffer conversion
// helper needs it for its function signature.
use screencapturekit::AudioBufferList;

use crate::audio::wav::WavWriter;

const SAMPLE_RATE: u32 = 48_000;
const CHANNELS: u16 = 2;
const BITS_PER_SAMPLE: u16 = 16;

pub struct ScKitLoopback {
    /// The live capture stream. Stays alive for the duration of the
    /// recording — dropping it would stop capture, but we always go
    /// through `stop()` for an orderly shutdown.
    stream: SCStream,
    /// Shared with the audio handler. `Some(writer)` while open; the
    /// handler appends samples through this. `stop()` takes it out and
    /// finalises the WAV header.
    wav_writer: Arc<Mutex<Option<WavWriter>>>,
    /// Output path we promised the caller. Returned by `stop()` so the
    /// recording manager can hand it to the muxer.
    output_path: PathBuf,
}

impl ScKitLoopback {
    /// Try to start a ScreenCaptureKit audio loopback session.
    ///
    /// Returns `Err` (and the caller falls back to virtual-driver
    /// detection / silence) when:
    /// - the host is macOS < 13 (SCKit audio API is unavailable),
    /// - the user denied Screen Recording permission,
    /// - no displays are present (SCKit can't build a content filter),
    /// - the WAV output path can't be created.
    pub fn try_start(output_path: PathBuf, pause_flag: Arc<AtomicBool>) -> Result<Self> {
        // 1. Get shareable content. This call performs the TCC permission
        //    check; if the user hasn't granted Screen Recording it returns
        //    an error and we fall through to BlackHole detection.
        let content = SCShareableContent::get()
            .map_err(|e| anyhow!("SCShareableContent::get failed (permission denied?): {e:?}"))?;
        let display = content
            .displays()
            .into_iter()
            .next()
            .context("no SCKit displays available")?;

        // 2. Audio-only filter. We still bind to a display because SCKit
        //    requires *some* content selection; the screen frames it emits
        //    are drained by a no-op handler below.
        let filter = SCContentFilter::create()
            .with_display(&display)
            .with_excluding_windows(&[])
            .build();

        // 3. Configure: audio enabled, exclude our own process so the
        //    recording isn't a feedback loop, 48 kHz / 2 ch to match the
        //    project's WAV format. Width/height are tiny because we don't
        //    consume the video output — SCKit still allocates them, so
        //    keeping them small minimises wasted GPU memory.
        let config = SCStreamConfiguration::new()
            .with_width(2)
            .with_height(2)
            .with_captures_audio(true)
            .with_excludes_current_process_audio(true)
            .with_sample_rate(SAMPLE_RATE as i32)
            .with_channel_count(CHANNELS as i32);

        // 4. WAV writer. Lives behind Arc<Mutex<Option<_>>> so the SCKit
        //    audio callback can grab it from the dispatch queue and
        //    `stop()` can take it out for finalisation.
        let writer = WavWriter::new(&output_path, SAMPLE_RATE, CHANNELS, BITS_PER_SAMPLE)
            .context("failed to create WAV writer for SCKit loopback")?;
        let wav_writer = Arc::new(Mutex::new(Some(writer)));

        // 5. Build handlers and start the stream.
        let audio_handler = SckitAudioHandler {
            wav_writer: wav_writer.clone(),
            pause_flag: pause_flag.clone(),
        };
        let mut stream = SCStream::new(&filter, &config);
        // Screen output: drain frames so the stream doesn't back-pressure.
        stream.add_output_handler(NoopScreenHandler, SCStreamOutputType::Screen);
        stream.add_output_handler(audio_handler, SCStreamOutputType::Audio);
        stream
            .start_capture()
            .map_err(|e| anyhow!("SCStream::start_capture failed: {e:?}"))?;

        log::info!("ScreenCaptureKit audio loopback started");

        Ok(Self {
            stream,
            wav_writer,
            output_path,
        })
    }

    /// Stop the SCKit stream and finalise the WAV header. Returns the
    /// output path the caller passed in at `try_start`.
    pub fn stop(mut self) -> Result<PathBuf> {
        // Stop capture before taking the writer — once stopped, the SCKit
        // queue is guaranteed not to fire more callbacks, so the take()
        // below races with nothing.
        if let Err(e) = self.stream.stop_capture() {
            log::warn!("SCStream::stop_capture errored (ignoring): {e:?}");
        }
        if let Some(writer) = self.wav_writer.lock().take() {
            writer.finish().context("failed to finalise SCKit WAV")?;
        }
        Ok(self.output_path)
    }
}

/// Audio sample-buffer handler. Called from SCKit's dispatch queue, not
/// from the recording thread — every field has to be `Send + Sync` and
/// the work has to be fast (a queue backup is observable as audio
/// drift). We do a single fixed-point conversion + one mutex acquire
/// per buffer; the WAV writer's `write_samples` is just `file.write_all`.
struct SckitAudioHandler {
    wav_writer: Arc<Mutex<Option<WavWriter>>>,
    pause_flag: Arc<AtomicBool>,
}

impl SCStreamOutputTrait for SckitAudioHandler {
    fn did_output_sample_buffer(&self, sample: CMSampleBuffer, of_type: SCStreamOutputType) {
        if of_type != SCStreamOutputType::Audio {
            return;
        }
        if self.pause_flag.load(Ordering::Acquire) {
            return;
        }
        let Some(list) = sample.audio_buffer_list() else {
            return;
        };

        let s16_bytes = match convert_audio_buffer_list_to_s16le(&list) {
            Some(b) if !b.is_empty() => b,
            _ => return,
        };

        let mut guard = self.wav_writer.lock();
        if let Some(writer) = guard.as_mut() {
            if let Err(e) = writer.write_samples(&s16_bytes) {
                log::warn!("SCKit WAV write failed (dropping sample): {e}");
            }
        }
    }
}

/// No-op screen handler. SCKit emits video sample buffers even when we
/// only care about audio (it's a single stream); without a drain the
/// queue eventually back-pressures.
struct NoopScreenHandler;

impl SCStreamOutputTrait for NoopScreenHandler {
    fn did_output_sample_buffer(&self, _sample: CMSampleBuffer, _of_type: SCStreamOutputType) {
        // Drop the buffer. The CMSampleBuffer's Drop releases the
        // underlying CoreVideo image buffer, so GPU memory is freed
        // every tick.
    }
}

/// Convert a SCKit `AudioBufferList` (Float32 PCM) into interleaved
/// `s16le` bytes, ready to feed `WavWriter::write_samples`.
///
/// SCKit's default audio format is `kAudioFormatLinearPCM` Float32. The
/// list's layout depends on the format flags:
///
/// - **Interleaved** (rare for SCKit, but possible): 1 buffer with N
///   channels' samples woven together — `L0 R0 L1 R1 …`.
/// - **Non-interleaved / planar** (SCKit default): N buffers, each one
///   channel's samples — `buffers[0] = L0 L1 L2 …`, `buffers[1] = R0 R1 R2 …`.
///
/// We detect by `num_buffers()`: 1 buffer ⇒ interleaved, ≥2 ⇒ planar.
/// Mono (1-channel) source just falls through the interleaved path.
fn convert_audio_buffer_list_to_s16le(list: &AudioBufferList) -> Option<Vec<u8>> {
    let n = list.num_buffers();
    if n == 0 {
        return None;
    }

    if n == 1 {
        // Interleaved (or mono). Convert f32 chunks straight to s16.
        let buf = list.get(0)?;
        let bytes = buf.data();
        let f32_count = bytes.len() / 4;
        if f32_count == 0 {
            return None;
        }
        let mut out = Vec::with_capacity(f32_count * 2);
        for chunk in bytes.chunks_exact(4) {
            let f = f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
            let i = (f.clamp(-1.0, 1.0) * f32::from(i16::MAX)) as i16;
            out.extend_from_slice(&i.to_le_bytes());
        }
        return Some(out);
    }

    // Planar: interleave the first 2 channels (matching our 2 ch config).
    // If SCKit hands us more channels, we discard the rest — the WAV
    // header was committed to 2 ch at construction.
    let l = list.get(0)?;
    let r = list.get(1)?;
    let l_bytes = l.data();
    let r_bytes = r.data();
    let samples_per_channel = l_bytes.len().min(r_bytes.len()) / 4;
    if samples_per_channel == 0 {
        return None;
    }
    let mut out = Vec::with_capacity(samples_per_channel * 4); // 2 ch × 2 bytes
    for i in 0..samples_per_channel {
        let off = i * 4;
        let lf = f32::from_le_bytes([
            l_bytes[off],
            l_bytes[off + 1],
            l_bytes[off + 2],
            l_bytes[off + 3],
        ]);
        let rf = f32::from_le_bytes([
            r_bytes[off],
            r_bytes[off + 1],
            r_bytes[off + 2],
            r_bytes[off + 3],
        ]);
        let li = (lf.clamp(-1.0, 1.0) * f32::from(i16::MAX)) as i16;
        let ri = (rf.clamp(-1.0, 1.0) * f32::from(i16::MAX)) as i16;
        out.extend_from_slice(&li.to_le_bytes());
        out.extend_from_slice(&ri.to_le_bytes());
    }
    Some(out)
}

// =============================================================================
// Runtime smoke-test checklist (for the Mac-having reviewer)
// =============================================================================
//
// 1. Build with `pnpm --filter recast-desktop tauri build` on macOS 13+.
//    Confirm Swift bridge compiles (build.rs in screencapturekit invokes
//    swiftc — Xcode CLT must be present).
//
// 2. First-run permission: launch the bundled .app and start a recording.
//    macOS prompts "Recast wants to record this computer's screen". Grant.
//    On denial, the log should show
//      `ScreenCaptureKit loopback unavailable (...) — checking for a
//       virtual loopback driver`
//    and the rest of the recording proceeds (silent unless BlackHole is
//    installed).
//
// 3. With permission granted, record while playing audio from any app
//    (Music, YouTube, etc.). Stop the recording. Open the project's
//    `.audio.wav` and verify:
//      - non-zero duration
//      - audio plays back at expected pitch (sample rate mismatch
//        symptom: chipmunked/slowed playback)
//      - both channels carry audio (mono symptom: silence in one ear)
//
// 4. Pause-mid-recording test: start recording, play audio, hit pause,
//    play more audio, hit resume, stop. The resulting WAV should
//    *concatenate* the pre-pause and post-pause audio with no audible
//    gap (the WASAPI/Unix paths produce the same behaviour).
//
// 5. Feedback-loop check: play audio from a tab in the *app's own*
//    WebView (any /audio test page works). The recorded WAV should
//    NOT contain the WebView audio — `excludes_current_process_audio`
//    is doing its job.
//
// 6. Stress: 30-minute continuous recording. Watch Activity Monitor's
//    "Recast" process — RAM should be flat, CPU under 5% for the audio
//    thread. A leak symptom is monotonic RAM growth tied to capture
//    duration.
//
// If any of (3)-(6) fails, the buffer-conversion logic in
// `convert_audio_buffer_list_to_s16le` is the first place to look —
// SCKit's exact f32 layout / channel order may differ from what we
// assume; logging `list.num_buffers()` and `buf.data().len()` for a
// few seconds will say which branch we're hitting.
