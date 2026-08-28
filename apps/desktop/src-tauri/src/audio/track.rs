use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};

use crate::audio::wav::{measured_sample_rate, WavFormat, WavWriter};
use crate::recording::clock::TrackStart;

/// Below this the drift ratio is dominated by buffer granularity rather than by
/// the device crystal, and re-declaring the rate would shift a short clip on the
/// strength of noise.
const MIN_DRIFT_SPAN: Duration = Duration::from_secs(5);

/// Accumulates one capture's samples into a WAV, gated by the pause flag.
///
/// Split from the capture loop so everything deciding what lands in the file
/// (pause gating, first-sample marking, the drift correction) is exercised
/// without a device. The loop over it is identical on all three platforms.
pub(super) struct TrackWriter {
    label: &'static str,
    output_path: PathBuf,
    writer: WavWriter,
    format: WavFormat,
    pause: Arc<AtomicBool>,
    start: TrackStart,
    /// When the first written buffer *began*, not when it arrived: a buffer
    /// covers time before its own delivery, and dropping that would inflate the
    /// measured rate.
    span_start: Option<Instant>,
    span_end: Option<Instant>,
    /// Paused time between two written buffers. A pause still open at stop lies
    /// past `span_end` by construction, so it is never in here.
    paused_total: Duration,
    paused_since: Option<Instant>,
    /// Frames of silence capturekit inserted over an idle device. Excluded from
    /// the drift measurement: they are paced by wall clock, not by the device
    /// crystal, so counting them would mask the drift being looked for.
    inserted_frames: u64,
    /// Whether a ragged buffer has already been reported, so a device that
    /// delivers one per buffer does not fill the log.
    ragged: bool,
}

impl TrackWriter {
    pub(super) fn new(
        label: &'static str,
        output_path: PathBuf,
        format: WavFormat,
        pause: Arc<AtomicBool>,
        start: TrackStart,
    ) -> Result<Self> {
        let writer = WavWriter::new(&output_path, format)
            .with_context(|| format!("failed to create the {label} WAV writer"))?;
        Ok(Self {
            label,
            output_path,
            writer,
            format,
            pause,
            start,
            span_start: None,
            span_end: None,
            paused_total: Duration::ZERO,
            paused_since: None,
            inserted_frames: 0,
            ragged: false,
        })
    }

    /// Advance the pause accounting without a delivery.
    ///
    /// Called when the device said nothing arrived, so a pause is measured from
    /// the clock rather than from whatever the device happened to deliver
    /// across it.
    pub(super) fn tick(&mut self, now: Instant) -> bool {
        if self.pause.load(Ordering::Acquire) {
            self.paused_since.get_or_insert(now);
            return false;
        }
        if let Some(since) = self.paused_since.take() {
            self.paused_total += now.saturating_duration_since(since);
        }
        true
    }

    /// Take one delivery, unless the recording is paused.
    ///
    /// Paused samples are dropped rather than written, which is what keeps the
    /// WAV gap-free across a pause. The device is still drained by the caller,
    /// so nothing overruns while this refuses.
    pub(super) fn accept(&mut self, samples: &[u8], inserted: bool, now: Instant) -> Result<()> {
        if !self.tick(now) {
            return Ok(());
        }
        // A partial frame means a disputed channel count; writing it swaps every channel after it.
        let frames = self.frames_in(samples.len());
        let whole = frames as usize * self.format.block_align() as usize;
        if whole < samples.len() && !self.ragged {
            self.ragged = true;
            log::warn!(
                "{} delivered {} bytes, not a whole number of {}-byte frames — dropping the remainder",
                self.label,
                samples.len(),
                self.format.block_align()
            );
        }
        if frames == 0 {
            return Ok(());
        }
        // This buffer covers time before it arrived, so its samples begin here.
        let began = now
            .checked_sub(self.format.duration_of(frames))
            .unwrap_or(now);
        // Marked on any first buffer: waiting for the first sound misplaces the track.
        self.start.mark_at(began);
        self.writer.write_samples(&samples[..whole])?;
        if inserted {
            self.inserted_frames += frames;
        }
        self.span_start.get_or_insert(began);
        self.span_end = Some(now);
        Ok(())
    }

    /// Finalise the header and return the written path.
    pub(super) fn finish(mut self) -> Result<PathBuf> {
        let frames = self.writer.frames_written();
        if let Some(rate) = self.drifted_rate() {
            log::warn!(
                "{} device clock drift: declared {}Hz, delivered {rate}Hz — re-declaring so the track stays locked to the picture",
                self.label,
                self.format.sample_rate
            );
            self.writer.set_sample_rate(rate);
        }
        let rate = self.writer.sample_rate();
        self.writer
            .finish()
            .with_context(|| format!("failed to finalise the {} WAV", self.label))?;
        log::info!(
            "{} capture finished: {} ({frames} frames @ {rate}Hz)",
            self.label,
            self.output_path.display()
        );
        Ok(self.output_path)
    }

    /// The rate the device actually delivered, when it is worth re-declaring.
    ///
    /// The picture is held to wall clock by the frame pacer, so an uncorrected
    /// audio crystal drifts against it for the whole take instead of cancelling.
    fn drifted_rate(&self) -> Option<u32> {
        let (start, end) = (self.span_start?, self.span_end?);
        let span = end
            .saturating_duration_since(start)
            .saturating_sub(self.paused_total)
            .saturating_sub(self.format.duration_of(self.inserted_frames));
        if span < MIN_DRIFT_SPAN {
            return None;
        }
        let delivered = self
            .writer
            .frames_written()
            .saturating_sub(self.inserted_frames);
        measured_sample_rate(delivered, span, self.format.sample_rate)
    }

    fn frames_in(&self, bytes: usize) -> u64 {
        bytes as u64 / u64::from(self.format.block_align().max(1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::wav::{wav_data_bytes, SampleFormat};

    const RATE: u32 = 48_000;
    const FORMAT: WavFormat = WavFormat {
        sample_rate: RATE,
        channels: 2,
        bits_per_sample: 16,
        format: SampleFormat::Int,
    };
    /// 10 ms of stereo 16-bit at 48 kHz, the order a mixer delivers in.
    const BUFFER_FRAMES: usize = 480;
    /// A device running 1% slow: 10 ms of samples every 10.1 ms of wall clock.
    const SLOW_INTERVAL: f64 = 0.0101;
    const SLOW_RATE: u32 = 47_525;

    struct Fixture {
        path: PathBuf,
        pause: Arc<AtomicBool>,
        start: TrackStart,
        /// What `TrackStart` measures against, so a test can undo it.
        session_origin: Instant,
        origin: Instant,
    }

    impl Fixture {
        fn new(tag: &str) -> Self {
            let dir = std::env::temp_dir().join(format!("recast-track-{}", std::process::id()));
            std::fs::create_dir_all(&dir).expect("temp dir");
            let session_origin = Instant::now();
            Self {
                path: dir.join(format!("{tag}.wav")),
                pause: Arc::new(AtomicBool::new(false)),
                start: TrackStart::new(session_origin),
                session_origin,
                origin: Instant::now(),
            }
        }

        fn writer(&self) -> TrackWriter {
            TrackWriter::new(
                "test",
                self.path.clone(),
                FORMAT,
                self.pause.clone(),
                self.start.clone(),
            )
            .expect("writer")
        }

        fn at(&self, secs: f64) -> Instant {
            self.origin + Duration::from_secs_f64(secs)
        }

        /// Deliver `count` buffers `interval` apart, starting after `from`.
        /// Returns the moment the last one arrived.
        fn feed(
            &self,
            writer: &mut TrackWriter,
            count: usize,
            interval: f64,
            inserted: bool,
            from: f64,
        ) -> f64 {
            let buffer = vec![0u8; BUFFER_FRAMES * FORMAT.block_align() as usize];
            let mut at = from;
            for _ in 0..count {
                at += interval;
                writer
                    .accept(&buffer, inserted, self.at(at))
                    .expect("write");
            }
            at
        }
    }

    fn declared_rate(path: &std::path::Path) -> u32 {
        let bytes = std::fs::read(path).expect("wav");
        u32::from_le_bytes([bytes[24], bytes[25], bytes[26], bytes[27]])
    }

    #[test]
    fn samples_delivered_while_paused_are_dropped() {
        let fixture = Fixture::new("paused");
        let mut writer = fixture.writer();
        let at = fixture.feed(&mut writer, 100, 0.01, false, 0.0);
        fixture.pause.store(true, Ordering::Release);
        let at = fixture.feed(&mut writer, 100, 0.01, false, at);
        fixture.pause.store(false, Ordering::Release);
        fixture.feed(&mut writer, 100, 0.01, false, at);
        let path = writer.finish().expect("finish");
        let per_buffer = (BUFFER_FRAMES * FORMAT.block_align() as usize) as u64;
        assert_eq!(
            wav_data_bytes(&path),
            Some(200 * per_buffer),
            "the paused buffers reached the file"
        );
    }

    /// The offset every track is aligned by. Marking on the first *sound*
    /// rather than the first buffer would claim the track starts later than the
    /// silence already written in front of it.
    #[test]
    fn inserted_silence_still_marks_the_track_start() {
        let fixture = Fixture::new("mark-silence");
        let mut writer = fixture.writer();
        assert_eq!(fixture.start.elapsed_us(), None);
        fixture.feed(&mut writer, 1, 0.01, true, 0.0);
        assert!(fixture.start.elapsed_us().is_some());
        writer.finish().expect("finish");
    }

    /// The A/V sync bug this exists to prevent: on an idle loopback the first
    /// delivery is capturekit's 100 ms silence chunk, and marking its ARRIVAL
    /// puts the whole system-audio track 100 ms late against the picture.
    #[test]
    fn the_track_starts_where_its_first_buffer_began_not_where_it_arrived() {
        let fixture = Fixture::new("mark-at");
        let mut writer = fixture.writer();
        let chunk = RATE as usize / 10;
        writer
            .accept(
                &vec![0u8; chunk * FORMAT.block_align() as usize],
                true,
                fixture.at(1.0),
            )
            .expect("write");
        writer.finish().expect("finish");
        let marked = fixture.start.elapsed_us().expect("marked");
        let origin_skew = fixture.origin.duration_since(fixture.session_origin);
        let began_us = marked - origin_skew.as_micros() as u64;
        assert!(
            (899_000..=901_000).contains(&began_us),
            "marked at {began_us}us, expected the 100ms buffer to start at 900ms"
        );
    }

    #[test]
    fn nothing_is_marked_or_written_before_the_first_delivery() {
        let fixture = Fixture::new("empty");
        let path = fixture.writer().finish().expect("finish");
        assert_eq!(fixture.start.elapsed_us(), None);
        assert_eq!(wav_data_bytes(&path), Some(0));
        assert_eq!(declared_rate(&path), RATE);
    }

    /// A muted device can deliver nothing at all across a pause, and the pause
    /// would then never be measured: without this the whole paused span is
    /// charged to the device and reads as drift.
    #[test]
    fn a_pause_a_silent_device_delivered_nothing_across_is_still_measured() {
        let fixture = Fixture::new("tick-pause");
        let mut writer = fixture.writer();
        let at = fixture.feed(&mut writer, 1_000, SLOW_INTERVAL, false, 0.0);
        fixture.pause.store(true, Ordering::Release);
        // The loop ticks on every timeout, so two of them bound the pause.
        assert!(!writer.tick(fixture.at(at)));
        fixture.pause.store(false, Ordering::Release);
        assert!(writer.tick(fixture.at(at + 10.1)));
        fixture.feed(&mut writer, 1_000, SLOW_INTERVAL, false, at + 10.1);
        let path = writer.finish().expect("finish");
        assert_eq!(declared_rate(&path), SLOW_RATE);
    }

    /// A backend and a format disagreeing about the channel count. Writing the
    /// odd bytes swaps every channel after them, which sounds like a broken
    /// device rather than like a bug.
    #[test]
    fn a_buffer_that_is_not_whole_frames_is_truncated_rather_than_written() {
        let fixture = Fixture::new("ragged");
        let mut writer = fixture.writer();
        let align = FORMAT.block_align() as usize;
        writer
            .accept(&vec![0u8; align * 3 + 2], false, fixture.at(1.0))
            .expect("write");
        let path = writer.finish().expect("finish");
        assert_eq!(wav_data_bytes(&path), Some(align as u64 * 3));
    }

    /// Nothing to align a track to, so the start must stay unmarked.
    #[test]
    fn a_buffer_shorter_than_one_frame_writes_nothing() {
        let fixture = Fixture::new("sub-frame");
        let mut writer = fixture.writer();
        writer
            .accept(&[0u8; 1], false, fixture.at(1.0))
            .expect("write");
        let path = writer.finish().expect("finish");
        assert_eq!(wav_data_bytes(&path), Some(0));
        assert_eq!(fixture.start.elapsed_us(), None);
    }

    #[test]
    fn a_slow_device_is_redeclared_at_the_rate_it_delivered() {
        let fixture = Fixture::new("drift");
        let mut writer = fixture.writer();
        fixture.feed(&mut writer, 1_000, SLOW_INTERVAL, false, 0.0);
        let path = writer.finish().expect("finish");
        assert_eq!(declared_rate(&path), SLOW_RATE);
    }

    /// Inserted silence is paced by wall clock, so counting it would make every
    /// idle loopback look like a perfectly accurate device.
    #[test]
    fn inserted_silence_is_left_out_of_the_drift_measurement() {
        let fixture = Fixture::new("drift-silence");
        let mut writer = fixture.writer();
        let at = fixture.feed(&mut writer, 1_000, SLOW_INTERVAL, false, 0.0);
        fixture.feed(&mut writer, 500, 0.01, true, at);
        let path = writer.finish().expect("finish");
        assert_eq!(
            declared_rate(&path),
            SLOW_RATE,
            "the inserted silence moved the measurement"
        );
    }

    /// A paused span delivers nothing to the file, so leaving it in the
    /// denominator reads as a device running far too slow to correct at all.
    #[test]
    fn a_paused_span_is_left_out_of_the_drift_measurement() {
        let fixture = Fixture::new("drift-pause");
        let mut writer = fixture.writer();
        let at = fixture.feed(&mut writer, 1_000, SLOW_INTERVAL, false, 0.0);
        fixture.pause.store(true, Ordering::Release);
        let at = fixture.feed(&mut writer, 1_000, SLOW_INTERVAL, false, at);
        fixture.pause.store(false, Ordering::Release);
        fixture.feed(&mut writer, 1_000, SLOW_INTERVAL, false, at);
        let path = writer.finish().expect("finish");
        assert_eq!(declared_rate(&path), SLOW_RATE);
    }

    /// A pause still running at stop is past the last write, so it must not be
    /// charged against the span the samples were delivered over.
    #[test]
    fn a_pause_still_open_at_stop_does_not_move_the_rate() {
        let fixture = Fixture::new("open-pause");
        let mut writer = fixture.writer();
        let at = fixture.feed(&mut writer, 1_000, SLOW_INTERVAL, false, 0.0);
        fixture.pause.store(true, Ordering::Release);
        fixture.feed(&mut writer, 1_000, SLOW_INTERVAL, false, at);
        let path = writer.finish().expect("finish");
        assert_eq!(declared_rate(&path), SLOW_RATE);
    }

    /// A clip too short to measure: buffer granularity alone clears the drift
    /// threshold, and the correction would be audible where the drift is not.
    #[test]
    fn a_short_take_is_not_redeclared() {
        let fixture = Fixture::new("short");
        let mut writer = fixture.writer();
        fixture.feed(&mut writer, 400, SLOW_INTERVAL, false, 0.0);
        let path = writer.finish().expect("finish");
        assert_eq!(declared_rate(&path), RATE);
    }

    #[test]
    fn a_device_within_tolerance_keeps_its_declared_rate() {
        let fixture = Fixture::new("no-drift");
        let mut writer = fixture.writer();
        fixture.feed(&mut writer, 3_000, 0.010_001, false, 0.0);
        let path = writer.finish().expect("finish");
        assert_eq!(declared_rate(&path), RATE);
    }
}
