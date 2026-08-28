use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use capturekit::{AudioCapturer, AudioDeviceId, AudioFormat, CaptureError};

use crate::audio::track::TrackWriter;
use crate::audio::wav::WavFormat;
use crate::recording::clock::TrackStart;

/// How long one read waits before the loop re-checks the stop flag. Bounds how
/// long `stop()` blocks; a quiet device is not an error, so nothing else
/// depends on it.
const POLL_TIMEOUT: Duration = Duration::from_millis(100);

/// Which endpoint a track is captured from.
///
/// Loopback and microphone differ only in the direction asked for and the
/// device that may be named, so they share one capture loop.
pub(super) enum Source {
    /// What the system is playing.
    Loopback,
    /// A microphone, by id, or the system default when `None`.
    Input(Option<String>),
}

impl Source {
    const fn label(&self) -> &'static str {
        match self {
            Self::Loopback => "system-audio loopback",
            Self::Input(_) => "microphone",
        }
    }

    const fn thread_name(&self) -> &'static str {
        match self {
            Self::Loopback => "recast-audio",
            Self::Input(_) => "recast-microphone",
        }
    }

    fn open(&self) -> Result<AudioCapturer> {
        let builder = match self {
            Self::Loopback => capturekit::audio_loopback(),
            Self::Input(id) => {
                let named = named_device(
                    id.as_deref(),
                    capturekit::capabilities().audio_device_enumeration,
                );
                match named {
                    Some(id) => capturekit::audio_input().device(AudioDeviceId(id)),
                    None => capturekit::audio_input(),
                }
            }
        };
        builder
            .build()
            .with_context(|| format!("failed to open the {} device", self.label()))
    }
}

/// The device id to open, or `None` for the platform default.
///
/// A backend that cannot enumerate cannot name either, and refuses an id it did
/// not issue; ignoring the request there records from the default rather than
/// failing the track outright.
fn named_device(requested: Option<&str>, can_name: bool) -> Option<String> {
    let id = requested?.trim();
    if id.is_empty() || id.eq_ignore_ascii_case("default") {
        return None;
    }
    if !can_name {
        log::info!("this platform captures only the default input; ignoring device '{id}'");
        return None;
    }
    Some(id.to_string())
}

/// A running capture thread writing one track's WAV.
pub(super) struct TrackSession {
    stop: Arc<AtomicBool>,
    format: AudioFormat,
    /// `Option` so `Drop` can take it; see the `Drop` impl.
    join: Option<JoinHandle<Result<PathBuf>>>,
}

impl TrackSession {
    /// Open the device and start capturing, or fail with why the device would
    /// not open.
    ///
    /// The device is opened on the capture thread and the outcome handed back,
    /// rather than opened here: WASAPI is COM, and an object created on a
    /// caller's apartment is not the capture thread's to use.
    pub(super) fn start(
        source: Source,
        output_path: PathBuf,
        pause: Arc<AtomicBool>,
        start: TrackStart,
    ) -> Result<Self> {
        let label = source.label();
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let (ready, opened) = mpsc::channel();
        let join = thread::Builder::new()
            .name(source.thread_name().to_string())
            .spawn(move || match open(&source, output_path, pause, start) {
                Ok((capturer, writer)) => {
                    let _ = ready.send(Ok(capturer.describe().format));
                    run(capturer, writer, &flag, label)
                }
                Err(err) => {
                    let _ = ready.send(Err(err));
                    Err(anyhow!("the {label} device was never opened"))
                }
            })
            .with_context(|| format!("failed to spawn the {label} capture thread"))?;

        match opened.recv() {
            Ok(Ok(format)) => Ok(Self {
                stop,
                format,
                join: Some(join),
            }),
            Ok(Err(err)) => Err(err),
            Err(_) => Err(anyhow!("the {label} capture thread ended before opening")),
        }
    }

    /// What the device negotiated, which is not always what was asked for.
    pub(super) const fn format(&self) -> AudioFormat {
        self.format
    }

    pub(super) fn stop(mut self) -> Result<PathBuf> {
        self.stop.store(true, Ordering::Release);
        self.join
            .take()
            .ok_or_else(|| anyhow!("audio session already stopped"))?
            .join()
            .map_err(|_| anyhow!("audio capture thread panicked"))?
    }
}

impl Drop for TrackSession {
    /// Only fires when a session is dropped without a clean `stop()` — a panic
    /// or an early return between start and the caller's stop. Without it the
    /// capture thread runs on holding the device open.
    fn drop(&mut self) {
        if let Some(join) = self.join.take() {
            self.stop.store(true, Ordering::Release);
            let _ = join.join();
        }
    }
}

fn open(
    source: &Source,
    output_path: PathBuf,
    pause: Arc<AtomicBool>,
    start: TrackStart,
) -> Result<(AudioCapturer, TrackWriter)> {
    let capturer = source.open()?;
    let desc = capturer.describe();
    let format = WavFormat::of(desc.format)?;
    log::info!(
        "{} capture opened via {}: {}Hz, {} ch, {} bits {:?}, output {}",
        source.label(),
        desc.backend,
        format.sample_rate,
        format.channels,
        format.bits_per_sample,
        format.format,
        output_path.display()
    );
    let writer = TrackWriter::new(source.label(), output_path, format, pause, start)?;
    Ok((capturer, writer))
}

/// Whether the loop should read again after an error.
///
/// Only a timeout. `CaptureError::is_recoverable` also covers a lost device,
/// but an audio backend reports that instantly and keeps reporting it, and this
/// loop has no reopen: retrying it would spin a core for the rest of the take.
const fn keep_reading(err: &CaptureError) -> bool {
    matches!(err, CaptureError::Timeout(_))
}

fn run(
    mut capturer: AudioCapturer,
    mut writer: TrackWriter,
    stop: &AtomicBool,
    label: &'static str,
) -> Result<PathBuf> {
    while !stop.load(Ordering::Acquire) {
        match capturer.next_buffer(POLL_TIMEOUT) {
            Ok(buffer) => {
                if buffer.is_discontinuous() {
                    log::warn!("{label} reported a break in the stream");
                }
                let accepted =
                    writer.accept(buffer.bytes(), buffer.is_inserted_silence(), Instant::now());
                if let Err(err) = accepted {
                    // Keep what was captured: the take is usable minus its tail.
                    log::error!("{label} WAV write failed: {err:#}");
                    break;
                }
            }
            // A quiet device, not a failure; the read already spent the timeout.
            Err(err) if keep_reading(&err) => {
                writer.tick(Instant::now());
            }
            Err(err) => {
                log::warn!("{label} capture ended early: {err}");
                break;
            }
        }
    }
    let _ = capturer.stop();
    writer.finish()
}

#[cfg(test)]
mod tests {
    use super::{keep_reading, named_device};
    use capturekit::{CaptureError, LostReason};
    use core::time::Duration;

    #[test]
    fn a_quiet_device_is_read_again() {
        assert!(keep_reading(&CaptureError::Timeout(Duration::from_millis(
            100
        ))));
    }

    /// The trap this exists to avoid: capturekit calls a lost device
    /// "recoverable", but it reports the loss with no wait at all, so a loop
    /// that retried it would spin a core until the recording stopped.
    #[test]
    fn a_lost_device_ends_the_track_even_though_capturekit_calls_it_recoverable() {
        let lost = CaptureError::Lost(LostReason::DeviceLost);
        assert!(lost.is_recoverable());
        assert!(!keep_reading(&lost));
        assert!(!keep_reading(&CaptureError::Lost(LostReason::AccessLost)));
    }

    #[test]
    fn a_named_device_is_opened_where_the_backend_can_name_one() {
        assert_eq!(
            named_device(Some("{0.0.1.00000000}.{abc}"), true),
            Some("{0.0.1.00000000}.{abc}".to_string())
        );
    }

    /// The picker sends these for "no explicit choice"; forwarding either as a
    /// literal id opens nothing.
    #[test]
    fn blank_and_default_ids_mean_the_system_default() {
        for id in ["", "   ", "default", "Default"] {
            assert_eq!(named_device(Some(id), true), None);
        }
        assert_eq!(named_device(None, true), None);
    }

    /// macOS captures the default input and refuses any other name, so honouring
    /// a stored id there would fail the mic track instead of recording it.
    #[test]
    fn a_backend_that_cannot_enumerate_records_the_default_rather_than_failing() {
        assert_eq!(named_device(Some("Blue Yeti"), false), None);
    }
}
