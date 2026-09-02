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

/// How long one read waits before the loop re-checks the stop flag. Bounds how long `stop()` blocks; a quiet device is not an error, so nothing else depends on it.
const POLL_TIMEOUT: Duration = Duration::from_millis(100);

/// Which endpoint a track is captured from.
/// Loopback and microphone differ only in the direction asked for and the device that may be named, so they share one capture loop.
pub(super) enum Source {
    /// What the system is playing.
    Loopback,
    /// A microphone, by id, or the system default when `None`.
    Input {
        device: Option<AudioDeviceId>,
        /// Why the device the user picked is not the one being opened, when it
        /// is not. Carried here so `open` reports it like any other notice.
        ignored: Option<String>,
    },
}

/// What a track opened with, and anything the user has to be told about it.
/// A fallback the user is not told about is the failure mode worth designing against: the recording looks fine and the wrong device is on it.
pub(super) struct Opened {
    pub(super) capturer: AudioCapturer,
    pub(super) notices: Vec<String>,
}

impl Source {
    /// The source for a picker's microphone id, resolved on the CALLER's thread: which device to open is policy, and the capture thread's job is to capture.
    /// A request this platform cannot honour becomes a notice rather than a log line, so the user hears about it.
    pub(super) fn input(requested: Option<String>) -> Self {
        let can_name = capturekit::capabilities().audio_device_enumeration;
        let (device, ignored) = named_device(requested.as_deref(), can_name);
        Self::Input {
            device: device.map(AudioDeviceId),
            ignored,
        }
    }

    const fn label(&self) -> &'static str {
        match self {
            Self::Loopback => "system-audio loopback",
            Self::Input { .. } => "microphone",
        }
    }

    const fn thread_name(&self) -> &'static str {
        match self {
            Self::Loopback => "recast-audio",
            Self::Input { .. } => "recast-microphone",
        }
    }

    fn open(&self) -> Result<Opened> {
        let (named, ignored) = match self {
            Self::Loopback => (None, None),
            Self::Input { device, ignored } => (device.as_ref(), ignored.clone()),
        };
        let mut notices: Vec<String> = ignored.into_iter().collect();
        let capturer = match named {
            None => self.build(None),
            Some(id) => match self.build(Some(id)) {
                // Unplugged or renamed since the picker saw it; the default beats nothing, if the user is told.
                Err(err) if is_missing_device(&err) => {
                    log::warn!("the chosen microphone {} is gone: {err}", id.0);
                    notices.push(MIC_GONE.to_string());
                    self.build(None)
                }
                other => other,
            },
        }
        .with_context(|| format!("failed to open the {} device", self.label()))?;
        Ok(Opened { capturer, notices })
    }

    fn build(&self, device: Option<&AudioDeviceId>) -> capturekit::Result<AudioCapturer> {
        let builder = match self {
            Self::Loopback => capturekit::audio_loopback(),
            Self::Input { .. } => capturekit::audio_input(),
        };
        match device {
            Some(id) => builder.device(id.clone()),
            None => builder,
        }
        .build()
    }
}

/// Told to the user when their chosen microphone has gone away.
const MIC_GONE: &str = "The microphone you picked is no longer available, so the recording used your default one instead.";

/// Told to the user when the platform cannot open a named input at all.
const MIC_UNNAMEABLE: &str = "This system only lets Recast record the default microphone, so the one you picked was not used.";

/// Whether an open failed because the named device is not there.
/// Distinguished from every other failure because it is the one worth falling back from: a device that is merely busy or refused is not helped by opening a different one.
fn is_missing_device(err: &CaptureError) -> bool {
    matches!(
        err,
        CaptureError::NotFoundNamed { .. } | CaptureError::NotFound { .. }
    )
}

/// The device id to open, and the notice for a request that cannot be honoured.
/// A backend that cannot enumerate cannot name either and refuses an id it did not issue, so this records from the default rather than failing the track, and says so.
fn named_device(requested: Option<&str>, can_name: bool) -> (Option<String>, Option<String>) {
    let Some(id) = requested.map(str::trim) else {
        return (None, None);
    };
    if id.is_empty() || id.eq_ignore_ascii_case("default") {
        return (None, None);
    }
    if !can_name {
        return (None, Some(MIC_UNNAMEABLE.to_string()));
    }
    (Some(id.to_string()), None)
}

/// A running capture thread writing one track's WAV.
pub(super) struct TrackSession {
    stop: Arc<AtomicBool>,
    format: AudioFormat,
    /// What the user has to be told about how this track opened.
    notices: Vec<String>,
    /// `Option` so `Drop` can take it; see the `Drop` impl.
    join: Option<JoinHandle<Result<PathBuf>>>,
}

impl TrackSession {
    /// Opens the device and starts capturing, or fails with why it would not open.
    /// The open happens on the capture thread and the outcome is handed back: WASAPI is COM, and an object made on the caller's apartment is not the capture thread's to use.
    pub(super) fn start(
        source: Source,
        output_path: PathBuf,
        pause: Arc<AtomicBool>,
        start: TrackStart,
        level: Option<super::MicLevelSink>,
    ) -> Result<Self> {
        let label = source.label();
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let (ready, opened) = mpsc::channel();
        let join = thread::Builder::new()
            .name(source.thread_name().to_string())
            .spawn(move || match open(&source, output_path, pause, start) {
                Ok((capturer, writer, notices)) => {
                    let _ = ready.send(Ok((capturer.describe().format, notices)));
                    run(capturer, writer, &flag, label, level)
                }
                Err(err) => {
                    let _ = ready.send(Err(err));
                    Err(anyhow!("the {label} device was never opened"))
                }
            })
            .with_context(|| format!("failed to spawn the {label} capture thread"))?;

        match opened.recv() {
            Ok(Ok((format, notices))) => Ok(Self {
                stop,
                format,
                notices,
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

    /// Anything the user has to be told about how this track opened.
    pub(super) fn notices(&self) -> &[String] {
        &self.notices
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
) -> Result<(AudioCapturer, TrackWriter, Vec<String>)> {
    let Opened { capturer, notices } = source.open()?;
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
    Ok((capturer, writer, notices))
}

/// Whether the loop should read again after an error; only a timeout qualifies.
/// `is_recoverable` also covers a lost device, but an audio backend reports that instantly and forever, and this loop has no reopen, so retrying spins a core for the take.
const fn keep_reading(err: &CaptureError) -> bool {
    matches!(err, CaptureError::Timeout(_))
}

fn run(
    mut capturer: AudioCapturer,
    mut writer: TrackWriter,
    stop: &AtomicBool,
    label: &'static str,
    level: Option<super::MicLevelSink>,
) -> Result<PathBuf> {
    let sample_format = capturer.describe().format.sample_format;
    // Throttle to ~15Hz: a per-buffer emit would flood the panel's event loop for a value the eye can't resolve faster.
    const LEVEL_INTERVAL: Duration = Duration::from_millis(66);
    let mut last_level = Instant::now();
    while !stop.load(Ordering::Acquire) {
        match capturer.next_buffer(POLL_TIMEOUT) {
            Ok(buffer) => {
                if buffer.is_discontinuous() {
                    log::warn!("{label} reported a break in the stream");
                }
                if let Some(sink) = &level {
                    if last_level.elapsed() >= LEVEL_INTERVAL {
                        last_level = Instant::now();
                        sink(buffer_level(buffer.bytes(), sample_format));
                    }
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

/// RMS of one interleaved PCM buffer, 0..1, for the live input meter. The panel
/// applies the perceptual curve and smoothing.
fn buffer_level(bytes: &[u8], format: capturekit::SampleFormat) -> f32 {
    let mut sum_sq = 0.0f64;
    let mut n = 0u64;
    match format {
        capturekit::SampleFormat::I16 => {
            for c in bytes.chunks_exact(2) {
                let v = f64::from(i16::from_le_bytes([c[0], c[1]])) / 32768.0;
                sum_sq += v * v;
                n += 1;
            }
        }
        capturekit::SampleFormat::I32 => {
            for c in bytes.chunks_exact(4) {
                let v = f64::from(i32::from_le_bytes([c[0], c[1], c[2], c[3]])) / 2_147_483_648.0;
                sum_sq += v * v;
                n += 1;
            }
        }
        capturekit::SampleFormat::F32 => {
            for c in bytes.chunks_exact(4) {
                let v = f64::from(f32::from_le_bytes([c[0], c[1], c[2], c[3]]));
                sum_sq += v * v;
                n += 1;
            }
        }
        // capturekit marks the enum non_exhaustive; an unknown width just yields no meter.
        _ => return 0.0,
    }
    if n == 0 {
        return 0.0;
    }
    ((sum_sq / n as f64).sqrt() as f32).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::{is_missing_device, keep_reading, named_device};
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
        let (device, notice) = named_device(Some("{0.0.1.00000000}.{abc}"), true);
        assert_eq!(device, Some("{0.0.1.00000000}.{abc}".to_string()));
        assert_eq!(notice, None, "an honoured request needs no notice");
    }

    /// A device that has gone away is the one failure worth opening a different
    /// device for. Anything else (busy, refused, no permission) is not helped by
    /// trying the default, and silently swapping would hide a real problem.
    #[test]
    fn only_a_missing_device_falls_back_to_the_default() {
        assert!(is_missing_device(&CaptureError::NotFoundNamed {
            kind: "audio input",
            id: "Blue Yeti".to_string(),
        }));
        assert!(is_missing_device(&CaptureError::NotFound {
            kind: "audio device",
            id: 0,
        }));
        assert!(!is_missing_device(&CaptureError::Lost(
            LostReason::AccessLost
        )));
        assert!(!is_missing_device(&CaptureError::Unsupported {
            backend: "test",
            operation: "open an input another session already holds",
        }));
    }

    /// The picker sends these for "no explicit choice"; forwarding either as a
    /// literal id opens nothing.
    #[test]
    fn blank_and_default_ids_mean_the_system_default() {
        for id in ["", "   ", "default", "Default"] {
            assert_eq!(named_device(Some(id), true), (None, None));
        }
        assert_eq!(named_device(None, true), (None, None));
    }

    /// macOS captures the default input and refuses any other name, so honouring
    /// a stored id there would fail the mic track instead of recording it.
    #[test]
    fn a_backend_that_cannot_enumerate_records_the_default_rather_than_failing() {
        let (device, notice) = named_device(Some("Blue Yeti"), false);
        assert_eq!(device, None, "the id it cannot open must not be forwarded");
        assert!(
            notice.is_some(),
            "falling back to the default must not be silent"
        );
    }

    #[test]
    fn buffer_level_is_zero_for_silence_and_tracks_amplitude() {
        use capturekit::SampleFormat::I16;
        assert_eq!(super::buffer_level(&[0u8; 16], I16), 0.0, "silence is 0");
        assert_eq!(super::buffer_level(&[], I16), 0.0, "empty is 0");
        // A constant half-scale i16 has RMS = amplitude = ~0.5.
        let half: Vec<u8> = (i16::MAX / 2).to_le_bytes().repeat(32);
        let level = super::buffer_level(&half, I16);
        assert!(
            (level - 0.5).abs() < 0.02,
            "half-scale RMS ~0.5, got {level}"
        );
    }
}

/// The `run` loop itself, driven by a scripted device.
/// Its timeout and error arms are the ones a real device will not perform on demand, and exactly the ones deciding whether a quiet take keeps wall time or a dead one spins a core.
#[cfg(test)]
mod run_tests {
    use super::{run, POLL_TIMEOUT};
    use crate::audio::track::TrackWriter;
    use crate::audio::wav::WavFormat;
    use crate::recording::TrackStart;
    use capturekit::mock::{MockAudio, MockAudioSource};
    use capturekit::{AudioCapturer, AudioFormat, LostReason, SampleFormat};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::time::Instant;

    const FORMAT: AudioFormat = AudioFormat {
        sample_rate: 48_000,
        channels: 2,
        sample_format: SampleFormat::I16,
    };

    struct Fixture {
        path: std::path::PathBuf,
        stop: Arc<AtomicBool>,
    }

    impl Fixture {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir().join("recast-run-loop");
            let _ = std::fs::create_dir_all(&dir);
            Self {
                path: dir.join(format!("{name}-{}.wav", std::process::id())),
                stop: Arc::new(AtomicBool::new(false)),
            }
        }

        /// Runs the real loop against `script`, stopping once the device has been read `answers` times or the loop ends on its own.
        /// Counted rather than timed, since the mock sleeps out every timeout; on its own thread so an arm that ends the take returns immediately.
        fn drive(&self, script: Vec<MockAudio>, answers: usize) -> (std::path::PathBuf, usize) {
            let source = MockAudioSource::new(FORMAT, script);
            let reads = source.reads();
            let released = source.released();
            let capturer = AudioCapturer::scripted(source);
            let writer = TrackWriter::new(
                "test",
                self.path.clone(),
                WavFormat::of(FORMAT).expect("a writable format"),
                Arc::new(AtomicBool::new(false)),
                TrackStart::new(Instant::now()),
            )
            .expect("the writer opens");

            let stop = Arc::clone(&self.stop);
            let loop_thread =
                std::thread::spawn(move || run(capturer, writer, &stop, "test", None));

            let deadline = Instant::now() + POLL_TIMEOUT * 40;
            while reads.load(Ordering::Relaxed) < answers
                && !loop_thread.is_finished()
                && Instant::now() < deadline
            {
                std::thread::sleep(core::time::Duration::from_millis(1));
            }
            self.stop.store(true, Ordering::Release);
            let path = loop_thread
                .join()
                .expect("the loop thread joins")
                .expect("the loop finishes");
            assert!(
                released.load(Ordering::Relaxed),
                "the loop returned without releasing the device"
            );
            (path, reads.load(Ordering::Relaxed))
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.path);
        }
    }

    /// The timeout arm KEEPS READING: a quiet microphone answers nothing all day, and treating that as the end would truncate the take at its first silent moment.
    /// Measuring a pause from the clock rather than from deliveries is pinned separately, on the writer.
    #[test]
    fn a_device_that_only_times_out_is_read_again_rather_than_ending_the_take() {
        let fixture = Fixture::new("timeout");
        let (path, reads) = fixture.drive(vec![], 4);
        assert!(
            reads >= 3,
            "the loop read {reads} times before stopping, so a timeout ended it"
        );
        // A file LENGTH cannot tell an empty take apart: the header predates the loop.
        assert_eq!(
            crate::audio::wav::wav_data_bytes(&path),
            Some(0),
            "a take of nothing but timeouts should hold no samples"
        );
    }

    /// A lost device ENDS the loop. capturekit calls it recoverable and reports
    /// it with no wait, so retrying would spin a core for the rest of the take.
    #[test]
    fn a_lost_device_ends_the_loop_rather_than_spinning() {
        let fixture = Fixture::new("lost");
        let started = Instant::now();
        let (path, reads) = fixture.drive(vec![MockAudio::Lost(LostReason::DeviceLost)], 40);
        let took = started.elapsed();
        assert_eq!(reads, 1, "the loop read again after the device was lost");
        assert!(
            took < POLL_TIMEOUT * 20,
            "the loop ran {took:?} after the device was lost, so it did not stop on it"
        );
        assert_eq!(
            crate::audio::wav::wav_data_bytes(&path),
            Some(0),
            "the take was not finalised into a readable WAV"
        );
    }

    /// Samples reach the file, so the two arms above are measured against a
    /// loop that does write when there is something to write.
    #[test]
    fn samples_reach_the_track() {
        let fixture = Fixture::new("samples");
        let frames = 480;
        let (path, _) = fixture.drive(
            vec![
                MockAudio::silence(0, FORMAT, frames),
                MockAudio::silence(10_000_000, FORMAT, frames),
            ],
            3,
        );
        let written = std::fs::metadata(&path).expect("the track exists").len();
        let payload = frames as u64 * FORMAT.bytes_per_frame() as u64 * 2;
        assert!(
            written >= payload,
            "wrote {written} bytes for {payload} bytes of samples"
        );
    }
}
