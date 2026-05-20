//! macOS system-audio loopback via ScreenCaptureKit.
//!
//! ScreenCaptureKit (macOS 13.0+) provides the only built-in path for
//! capturing system audio on macOS without a virtual driver. This is
//! what Loom, CleanShot X, OBS, Riverside, and every other modern
//! macOS recorder use. The Rust wrapper crate (`screencapturekit`)
//! exposes the underlying Objective-C types; we configure an audio-only
//! `SCStream`, attach an output handler, and copy each
//! `CMSampleBuffer`'s PCM payload into the existing `WavWriter`.
//!
//! ## Permissions
//!
//! SCKit — even audio-only — requires the **Screen Recording**
//! entitlement on macOS 13+. The first system-audio attempt prompts
//! the user via TCC; we surface a clear error if denied so the caller
//! falls through to the BlackHole / silence chain.
//!
//! ## Sample format
//!
//! SCKit delivers Float32 interleaved by default at the system's mix
//! sample rate (typically 48 kHz). We pin the request to 48 kHz / 2 ch
//! / Float32 and write WAV `bits=32` so the format chunk declares
//! `WAVE_FORMAT_IEEE_FLOAT` instead of integer PCM.
//!
//! ## Excludes-current-process
//!
//! We set `excludes_current_process_audio = true` so the recorded
//! track does not include Recast's own audio output (e.g. UI sound
//! effects, preview playback). This is the recommended default for any
//! recorder that also plays audio.
//!
//! ## API-drift notes
//!
//! The `screencapturekit` crate has moved fast across 0.2/0.3
//! releases. The integration below targets the 0.3 series and uses the
//! high-level types so a minor version bump should be a localised
//! patch rather than a structural rewrite. If a future bump breaks
//! compilation, the symbols most likely to need rename are
//! `SCContentFilter::new(InitParams::Display(…))`, the output handler
//! trait method, and `CMSampleBuffer::get_av_audio_buffer_list()`.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use parking_lot::Mutex;

use crate::audio::wav::WavWriter;

const SAMPLE_RATE: u32 = 48_000;
const CHANNELS: u16 = 2;
// SCKit emits IEEE float32 PCM. Setting bits=32 tells our WavWriter to
// stamp the WAV format chunk as `WAVE_FORMAT_IEEE_FLOAT` instead of
// integer PCM — matches what the Windows WASAPI backend does for its
// 32-bit-float mix format path.
const BITS: u16 = 32;

pub struct ScKitLoopback {
    stream: screencapturekit::sc_stream::SCStream,
    // The WAV writer is shared between the output-handler callback
    // thread (where SCKit invokes us) and this struct's `stop()` (the
    // recording's owning thread). `Option<…>` so `stop()` can `take()`
    // and finalise it exactly once, even if the callback fires again
    // after stop is called.
    writer: Arc<Mutex<Option<WavWriter>>>,
    output_path: PathBuf,
}

impl ScKitLoopback {
    /// Try to start a ScreenCaptureKit audio stream. Returns `Err` on
    /// any failure (macOS < 13, no displays, Screen Recording denied,
    /// SCKit framework error) so the caller can fall through to the
    /// BlackHole / silence chain.
    pub fn try_start(output_path: PathBuf, pause_flag: Arc<AtomicBool>) -> Result<Self> {
        use screencapturekit::{
            sc_content_filter::{InitParams, SCContentFilter},
            sc_error_handler::StreamErrorHandler,
            sc_output_handler::{SCStreamOutputType, StreamOutput},
            sc_shareable_content::SCShareableContent,
            sc_stream::SCStream,
            sc_stream_configuration::SCStreamConfiguration,
        };

        // SCShareableContent enumerates capturable resources. On
        // macOS < 13 or when Screen Recording is not granted, the
        // underlying SCKit call returns nil and the wrapper produces
        // an empty content struct — we'll fall through on the
        // "no displays" branch below, which matches the runtime
        // signature we want.
        let shareable = SCShareableContent::current();
        let display = shareable
            .displays
            .into_iter()
            .next()
            .ok_or_else(|| {
                anyhow!(
                    "SCKit reported no displays — macOS < 13, or Screen Recording \
                     permission has not been granted to Recast in \
                     System Settings → Privacy & Security."
                )
            })?;
        // The SCKit audio tap is attached to a content filter. For
        // full-system audio we filter on a whole display; SCKit will
        // capture every process's audio output (minus our own — see
        // excludes_current_process_audio below).
        let filter = SCContentFilter::new(InitParams::Display(display));

        let mut config = SCStreamConfiguration::default();
        config.captures_audio = true;
        // Don't capture Recast's own audio (UI sounds, preview
        // playback) — every modern recorder defaults this on.
        config.excludes_current_process_audio = true;
        // Request 48 kHz stereo to match the WAV we'll write. SCKit
        // resamples if the system mix differs.
        config.sample_rate = SAMPLE_RATE as i64;
        config.channel_count = CHANNELS as i64;

        let writer = Arc::new(Mutex::new(Some(
            WavWriter::new(&output_path, SAMPLE_RATE, CHANNELS, BITS)
                .context("failed to create SCKit loopback WAV writer")?,
        )));

        struct ErrHandler;
        impl StreamErrorHandler for ErrHandler {
            fn on_error(&self) {
                // SCKit fires this asynchronously; we can't fail the
                // recording from here. The WAV will simply stop
                // growing — the editor's silence-trim / duration
                // detector will spot the gap downstream.
                log::error!("ScreenCaptureKit audio stream errored");
            }
        }

        struct AudioHandler {
            writer: Arc<Mutex<Option<WavWriter>>>,
            pause_flag: Arc<AtomicBool>,
        }
        impl StreamOutput for AudioHandler {
            fn did_output_sample_buffer(
                &self,
                sample_buffer: screencapturekit::cm_sample_buffer::CMSampleBuffer,
                output_type: SCStreamOutputType,
            ) {
                if output_type != SCStreamOutputType::Audio {
                    return;
                }
                if self.pause_flag.load(Ordering::Acquire) {
                    // Pause-aware (matches WASAPI/FFmpeg path): drop
                    // the sample buffer on the floor so the WAV stays
                    // gap-free across recording pauses. SCKit will
                    // keep delivering buffers — we just don't write
                    // them.
                    return;
                }
                // Extract PCM bytes from the audio buffer list. The
                // SCKit Rust wrapper exposes the underlying
                // CoreMedia `CMSampleBufferGetAudioBufferList…` via a
                // safe accessor; each `AudioBuffer` carries a raw
                // `&[u8]` of interleaved float32 stereo for one
                // delivery window (~10 ms by default).
                let buffer_list = match sample_buffer.get_av_audio_buffer_list() {
                    Ok(b) => b,
                    Err(e) => {
                        log::warn!("SCKit: failed to access audio buffer list: {e:?}");
                        return;
                    }
                };
                let mut slot = self.writer.lock();
                let Some(writer) = slot.as_mut() else { return };
                for audio_buffer in buffer_list.buffers() {
                    let data = audio_buffer.data();
                    if let Err(e) = writer.write_samples(data) {
                        log::warn!("SCKit: WAV write failed: {e}");
                        break;
                    }
                }
            }
        }

        let handler = AudioHandler {
            writer: writer.clone(),
            pause_flag: pause_flag.clone(),
        };

        let mut stream = SCStream::new(&filter, &config, ErrHandler);
        stream.add_output_handler(handler, SCStreamOutputType::Audio);
        stream.start_capture().context(
            "SCKit start_capture failed — ensure Screen Recording is granted to Recast \
             in System Settings → Privacy & Security",
        )?;

        log::info!(
            "ScreenCaptureKit loopback started ({} Hz, {} ch, float32), output: {}",
            SAMPLE_RATE,
            CHANNELS,
            output_path.display()
        );

        Ok(Self {
            stream,
            writer,
            output_path,
        })
    }

    pub fn stop(self) -> Result<PathBuf> {
        // `stop_capture` blocks until SCKit acknowledges; subsequent
        // `did_output_sample_buffer` calls won't fire. We then take
        // the writer out of the shared slot and finalise it. The
        // double-step (stop SCKit, then finalise WAV) avoids the
        // race where a late callback writes to a half-finalised file.
        let _ = self.stream.stop_capture();
        let mut slot = self.writer.lock();
        if let Some(writer) = slot.take() {
            writer
                .finish()
                .context("failed to finalise SCKit loopback WAV")?;
        }
        log::info!(
            "ScreenCaptureKit loopback finished: {}",
            self.output_path.display()
        );
        Ok(self.output_path)
    }
}
