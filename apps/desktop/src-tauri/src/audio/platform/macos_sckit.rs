//! macOS system-audio loopback via ScreenCaptureKit.
//!
//! SCKit is the only built-in macOS API for system-audio capture
//! without a virtual driver (BlackHole / Soundflower / VB-Cable).
//! It shipped audio support in macOS 13; on earlier versions
//! `try_start` returns `Err` and the caller falls through to the
//! virtual-driver detection in `ffmpeg_unix::PlatformAudioSession`.
//!
//! ## How it works (when enabled)
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
//! Security → Screen Recording.
//!
//! ## Status — gated behind `sckit-loopback` Cargo feature
//!
//! **Compile-tested only, and disabled by default.** The
//! `screencapturekit` 6.x crate transitively pulls `apple-metal`, whose
//! Swift bridge unconditionally references newer SDK symbols
//! (e.g. `reactiveTextureUsage`, introduced around macOS 14.4) that
//! aren't on the GH Actions `macos-latest` runner's Xcode. Until the
//! upstream `apple-metal` packaging bug is fixed (should be
//! `#if canImport(...)` compile-time gates, not runtime `#available`
//! checks), enabling the dep would break our CI macOS compile.
//!
//! Build flow:
//! - **Default**: `try_start` returns `Err` immediately; caller falls
//!   through to BlackHole detection → silence. CI compiles green.
//! - **`--features sckit-loopback`**: real SCKit integration; requires
//!   a local Xcode SDK that has the missing symbols. Runtime smoke-test
//!   checklist lives at the bottom of this file.

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use anyhow::Result;

#[cfg(feature = "sckit-loopback")]
pub use enabled::ScKitLoopback;

#[cfg(not(feature = "sckit-loopback"))]
pub use disabled::ScKitLoopback;

#[cfg(not(feature = "sckit-loopback"))]
mod disabled {
    use super::*;
    use anyhow::anyhow;

    pub struct ScKitLoopback {
        _placeholder: (),
    }

    impl ScKitLoopback {
        /// Always returns `Err` when the `sckit-loopback` feature is off —
        /// the caller treats that as "skip SCKit, try the virtual-driver
        /// chain next". The error message is informational; it lands in
        /// the log at `info` level (not `warn`), since this is the
        /// expected default-build behaviour, not a failure mode.
        pub fn try_start(_output_path: PathBuf, _pause_flag: Arc<AtomicBool>) -> Result<Self> {
            Err(anyhow!(
                "ScreenCaptureKit loopback disabled (build with --features \
                 sckit-loopback to enable; requires Xcode with macOS 14.4+ SDK)"
            ))
        }

        /// Unreachable while `try_start` always errs. Kept so the type's
        /// public surface matches what the caller would expect once the
        /// real implementation lands.
        pub fn stop(self) -> Result<PathBuf> {
            unreachable!(
                "ScKitLoopback::stop must not be called against the placeholder — \
                 try_start always returns Err so no session ever exists"
            )
        }
    }
}

#[cfg(feature = "sckit-loopback")]
mod enabled {
    use super::*;
    use std::sync::atomic::Ordering;

    use anyhow::{anyhow, Context};
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
        pub fn try_start(output_path: PathBuf, pause_flag: Arc<AtomicBool>) -> Result<Self> {
            // 1. Permission gate. `SCShareableContent::get` performs the TCC
            //    check; denial returns an error and we fall through to
            //    BlackHole detection.
            let content = SCShareableContent::get().map_err(|e| {
                anyhow!("SCShareableContent::get failed (permission denied?): {e:?}")
            })?;
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
            //    project's WAV format.
            let config = SCStreamConfiguration::new()
                .with_width(2)
                .with_height(2)
                .with_captures_audio(true)
                .with_excludes_current_process_audio(true)
                .with_sample_rate(SAMPLE_RATE as i32)
                .with_channel_count(CHANNELS as i32);

            // 4. WAV writer.
            let writer = WavWriter::new(&output_path, SAMPLE_RATE, CHANNELS, BITS_PER_SAMPLE)
                .context("failed to create WAV writer for SCKit loopback")?;
            let wav_writer = Arc::new(Mutex::new(Some(writer)));

            // 5. Build handlers and start the stream.
            let audio_handler = SckitAudioHandler {
                wav_writer: wav_writer.clone(),
                pause_flag: pause_flag.clone(),
            };
            let mut stream = SCStream::new(&filter, &config);
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

        /// Stop the SCKit stream and finalise the WAV header.
        pub fn stop(mut self) -> Result<PathBuf> {
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
        let l = list.get(0)?;
        let r = list.get(1)?;
        let l_bytes = l.data();
        let r_bytes = r.data();
        let samples_per_channel = l_bytes.len().min(r_bytes.len()) / 4;
        if samples_per_channel == 0 {
            return None;
        }
        let mut out = Vec::with_capacity(samples_per_channel * 4);
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
}

// =============================================================================
// Runtime smoke-test checklist (for the Mac-having reviewer)
// =============================================================================
//
// Build with `cargo tauri build --features sckit-loopback` on macOS 13+
// against an Xcode SDK that has both macOS 14.4 (`reactiveTextureUsage`)
// and macOS 26 (`MTLSamplerReductionMode`). Without the feature, the
// stub branch above ships and SCKit is bypassed.
//
// 1. First-run permission: launch the .app and start a recording.
//    macOS prompts "Recast wants to record this computer's screen". Grant.
//    On denial, the log should show
//      `ScreenCaptureKit loopback unavailable (...) — checking for a
//       virtual loopback driver`
//    and the recording proceeds (silent unless BlackHole is installed).
//
// 2. Record while playing audio from any app (Music, YouTube, etc.).
//    Stop the recording. Open the project's `.audio.wav`:
//      - non-zero duration
//      - audio plays back at expected pitch (sample-rate mismatch =
//        chipmunked / slowed playback)
//      - both channels carry audio (mono in one ear = planar/interleaved
//        branch detection wrong)
//
// 3. Pause-mid-recording: start → play audio → pause → play more →
//    resume → stop. WAV should concatenate with no audible gap.
//
// 4. Feedback-loop check: play audio from a tab in the *app's own*
//    WebView. The recorded WAV should NOT contain WebView audio —
//    `excludes_current_process_audio` is doing its job.
//
// 5. Stress: 30-minute continuous recording. Activity Monitor's "Recast"
//    process should show flat RAM, CPU under 5% for the audio thread.
//
// If (2) fails, the buffer-conversion logic in
// `enabled::convert_audio_buffer_list_to_s16le` is the first place to
// look — log `list.num_buffers()` and `buf.data().len()` for a few
// seconds to see which branch we're hitting.
