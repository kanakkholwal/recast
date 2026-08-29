use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use anyhow::Result;
use crossbeam_queue::ArrayQueue;

use crate::capture::{CaptureNotice, CaptureSource, CapturedFrame};

#[derive(Clone, Default)]
pub struct PipelineStats {
    pub captured_frames: Arc<AtomicU64>,
    pub dropped_frames: Arc<AtomicU64>,
    pub encoded_frames: Arc<AtomicU64>,
}

impl PipelineStats {
    pub fn snapshot(&self) -> PipelineSnapshot {
        PipelineSnapshot {
            captured_frames: self.captured_frames.load(Ordering::Relaxed),
            dropped_frames: self.dropped_frames.load(Ordering::Relaxed),
            encoded_frames: self.encoded_frames.load(Ordering::Relaxed),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct PipelineSnapshot {
    pub captured_frames: u64,
    pub dropped_frames: u64,
    pub encoded_frames: u64,
}

/// Packed frames waiting for the encoder thread, and what that thread's
/// throughput looks like.
///
/// The frames carry no timing: the writer on the other end declares a fixed rate
/// and derives every duration from the frame count, so a timestamp here would be
/// a second answer to a question the pipe has already settled.
#[derive(Clone)]
pub struct RecordingPipeline {
    queue: Arc<ArrayQueue<Arc<[u8]>>>,
    stats: PipelineStats,
}

impl RecordingPipeline {
    pub fn new(capacity: usize) -> Self {
        Self {
            queue: Arc::new(ArrayQueue::new(capacity)),
            stats: PipelineStats::default(),
        }
    }

    pub fn push(&self, frame: Arc<[u8]>) {
        if self.queue.push(frame).is_err() {
            // The queue is full — the encoder is falling behind the
            // pacer. Increment the counter and surface the condition in
            // the log so a recording that comes out choppy has a paper
            // trail. Capacity is sized by `RecordingManager::start` based
            // on the capture resolution (≤256 MB BGRA budget), so the
            // queue holds anywhere from ~8 frames at 4K to 180 at 720p.
            //
            // We log loudly on the first drop of each session so the
            // problem is visible the moment it starts, then dampen to
            // once per ~5 s of sustained dropping (every 300th drop at
            // 60 fps) to avoid flooding the log if the encoder stays
            // behind. The atomic `fetch_add` returns the PRE-increment
            // value, so we treat `0` as "this is the first drop".
            let prev = self.stats.dropped_frames.fetch_add(1, Ordering::Relaxed);
            if prev == 0 {
                log::warn!(
                    "recording pipeline: frame queue saturated — frames are being \
                     dropped. The encoder is not keeping up with the capture rate. \
                     This will surface as choppy / time-compressed playback. Likely \
                     causes: hardware encoder unavailable (libx264 software fallback \
                     at high resolution), disk I/O contention, or CPU pressure from \
                     another app."
                );
            } else if prev % 300 == 299 {
                // prev=299 ⇒ this drop is the 300th; prev=599 ⇒ 600th; …
                log::warn!("recording pipeline: {} frames dropped total", prev + 1);
            }
        }
    }

    pub fn pop(&self) -> Option<Arc<[u8]>> {
        self.queue.pop()
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    pub fn stats(&self) -> PipelineStats {
        self.stats.clone()
    }
}

/// Where the capture loop sends a frame it has decided to emit.
///
/// The loop owns timing and recovery; the sink owns the encoder. Splitting them
/// is what lets the same loop feed FFmpeg's stdin and the GPU encoder without a
/// second copy of the pause, notice and stale-frame handling.
pub trait FrameSink: Send {
    fn accept(&mut self, frame: &CapturedFrame, pts_us: u64, width: u32, height: u32)
        -> Result<()>;

    /// Close the output. Called once, on the capture thread, after the loop stops.
    fn finish(&mut self) -> Result<()> {
        Ok(())
    }
}

/// The bounded queue the FFmpeg encoder thread drains.
pub struct QueueSink(RecordingPipeline);

impl QueueSink {
    pub const fn new(pipeline: RecordingPipeline) -> Self {
        Self(pipeline)
    }
}

impl FrameSink for QueueSink {
    fn accept(&mut self, frame: &CapturedFrame, _pts_us: u64, _: u32, _: u32) -> Result<()> {
        let CapturedFrame::Host(data) = frame else {
            anyhow::bail!("the FFmpeg encoder cannot read a frame left on the GPU");
        };
        self.0.push(Arc::clone(data));
        Ok(())
    }
}

/// How often the loop emits, which is a property of what the sink can read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cadence {
    /// Exactly `fps` frames per wall-clock second, repeating the cached frame
    /// when the source is idle. For a sink that has no timestamps and derives
    /// duration from frame count, which is every FFmpeg rawvideo pipe.
    Fixed,
    /// Only the frames the source actually produced, plus a keepalive repeat so
    /// a still desktop stays seekable. For a sink that stamps each sample, where
    /// a repeat would be a byte cost for no information.
    ///
    /// Chosen only by the Windows GPU writer today, so off Windows nothing
    /// constructs it.
    #[cfg_attr(not(windows), allow(dead_code))]
    OnChange { keepalive: Duration },
}

const STALE_FIRST_WARN: Duration = Duration::from_secs(5);
const STALE_REPEAT_WARN: Duration = Duration::from_secs(30);

fn stale_warning_due(stale_for: Duration, warnings_emitted: u32) -> bool {
    stale_for >= STALE_FIRST_WARN + STALE_REPEAT_WARN * warnings_emitted
}

/// Spawn the capture + frame-pacer loop.
///
/// Why this is a frame pacer, not a "capture as fast as DXGI delivers" loop:
/// the encoder declares the input rate to FFmpeg as `-framerate {fps}`, so
/// every frame we push contributes 1/fps seconds of *video PTS*, regardless
/// of when it was captured in wall-clock time. DXGI Desktop Duplication
/// only delivers a new frame when the desktop actually changes — for a
/// static screen that's < 1 fps. If we'd push frames at DXGI's natural
/// rate, a 10-second recording with little motion would encode as a 1-2
/// second video, while the cursor track (timestamped from a wall-clock
/// `Instant`) still spans 10 s. The editor would then race through the
/// entire cursor performance in the compressed playback duration —
/// exactly the "everything happens in 5 seconds" symptom.
///
/// To lock playback time to wall-clock time, this loop:
/// 1. Holds a single `last_frame` cache (the most recent captured texture).
/// 2. Polls DXGI non-blocking (`AcquireNextFrame(0)`) every iteration and
///    drains any new frames into the cache, so we always emit the freshest
///    pixels available at the tick instant.
/// 3. Emits exactly `target_fps` frames per real-time second to the
///    pipeline using a deadline scheduler. When DXGI has no new frame, we
///    duplicate the cached one — the video shows a still during static
///    desktop, which is correct.
///
/// Result: wall-clock seconds == video PTS seconds == cursor track
/// seconds. Preview and rendered MP4 stay in lockstep with the cursor
/// track regardless of how often the desktop redraws.
pub struct CaptureLoop {
    pub stop_flag: Arc<std::sync::atomic::AtomicBool>,
    pub pause_flag: Arc<std::sync::atomic::AtomicBool>,
    /// Where emitted frames go, and what closes the output.
    pub sink: Box<dyn FrameSink>,
    /// How often to emit, which is a property of the sink.
    pub cadence: Cadence,
    /// The session, and the only origin in here: `origin()` is what the tracks
    /// measure their first sample from, `effective_elapsed()` is what stamps
    /// each sample. A sample stamped from the raw origin would be stretched
    /// across a pause, freezing the recording for as long as the user was away.
    pub timeline: crate::recording::RecordingClock,
    /// Counts what the loop emitted, so a recording reports its own throughput
    /// whichever writer took the frames.
    pub stats: PipelineStats,
    pub target_fps: u32,
    /// Marked at the FIRST encoded frame: video t=0, which the cursor blocks on.
    pub video_start: crate::recording::TrackStart,
}

/// Hand one frame to the writer and count it.
///
/// The count lives here rather than in a sink because every writer owes the
/// same number: what the loop decided to record.
fn emit(
    sink: &mut Box<dyn FrameSink>,
    frame: &CapturedFrame,
    pts_us: u64,
    source: &dyn CaptureSource,
    stats: &PipelineStats,
) -> Result<()> {
    sink.accept(frame, pts_us, source.width(), source.height())?;
    stats.captured_frames.fetch_add(1, Ordering::Relaxed);
    Ok(())
}

/// How long an `OnChange` poll blocks for.
///
/// Short enough that stop, pause and a capture notice are still handled
/// promptly; long enough that an idle desktop parks in the backend instead of
/// spinning a core.
const ON_CHANGE_POLL: Duration = Duration::from_millis(4);

pub fn spawn_capture_loop(
    mut source: Box<dyn CaptureSource>,
    session: CaptureLoop,
    // Forwards anything the user has to be told about the capture.
    notify: impl Fn(CaptureNotice) + Send + 'static,
) -> Result<thread::JoinHandle<Result<()>>> {
    let CaptureLoop {
        stop_flag,
        pause_flag,
        mut sink,
        cadence,
        timeline,
        stats,
        target_fps,
        video_start,
    } = session;
    thread::Builder::new()
        .name("recast-capture".into())
        .spawn(move || {
            let fps = target_fps.max(1) as u64;
            // Integer nanoseconds: truncating microseconds ran 60fps 0.004% fast.
            let tick_at = |base: Instant, k: u64| -> Instant {
                base + Duration::from_nanos(k.saturating_mul(1_000_000_000) / fps)
            };

            // Capped: a source that yields nothing (no macOS Screen Recording grant) hangs stop().
            const WARMUP_TIMEOUT: Duration = Duration::from_secs(10);
            let warmup_start = Instant::now();
            let mut last_frame: CapturedFrame = loop {
                if stop_flag.load(Ordering::Acquire) {
                    return Ok(());
                }
                if warmup_start.elapsed() >= WARMUP_TIMEOUT {
                    return Err(anyhow::anyhow!(
                        "no frames captured within {}s — the screen source produced \
                         no data. On macOS, grant Screen Recording in System \
                         Settings → Privacy & Security, then record again.",
                        WARMUP_TIMEOUT.as_secs()
                    ));
                }
                match source.capture_next(Duration::from_millis(100))? {
                    Some(frame) => break frame,
                    None => continue,
                }
            };

            // One instant for both, or the cursor's zero and the video's differ.
            let at = Instant::now();
            let first_us = at.saturating_duration_since(timeline.origin()).as_micros() as u64;
            video_start.mark_at(at);
            emit(&mut sink, &last_frame, first_us, source.as_ref(), &stats)?;
            // Anchor the exact schedule at the warmup frame. `emitted` counts
            // frames pushed since `pacer_base`; tick `emitted+1` is the next
            // deadline. Both reset on resume so a paused span is excluded
            // without being "caught up" as lag.
            let mut pacer_base = Instant::now();
            let mut emitted: u64 = 0;
            let mut was_paused = false;
            let mut last_fresh_at = Instant::now();
            let mut last_emit_at = Instant::now();
            let mut stale_warnings: u32 = 0;

            loop {
                if stop_flag.load(Ordering::Acquire) {
                    break;
                }
                // Emit nothing: a span with no frames does not exist in the output.
                if pause_flag.load(Ordering::Acquire) {
                    was_paused = true;
                    thread::sleep(Duration::from_millis(20));
                    continue;
                }
                if was_paused {
                    // Restart the schedule, or the paused span is caught up as lag.
                    pacer_base = Instant::now();
                    emitted = 0;
                    was_paused = false;
                    last_emit_at = Instant::now();
                }

                // Capped: a source ignoring the timeout always answers Some; OnChange emits each.
                let (max_drain, poll) = match cadence {
                    Cadence::Fixed => (4usize, Duration::ZERO),
                    Cadence::OnChange { .. } => (1usize, ON_CHANGE_POLL),
                };
                let mut fresh = false;
                let mut failed = None;
                for _ in 0..max_drain {
                    match source.capture_next(poll) {
                        Ok(Some(frame)) => {
                            last_frame = frame;
                            last_fresh_at = Instant::now();
                            stale_warnings = 0;
                            fresh = true;
                        }
                        Ok(None) => break,
                        Err(e) => {
                            log::error!("screen capture source failed: {e}");
                            failed = Some(e);
                            break;
                        }
                    }
                }
                // Repeating the last frame made an unplugged display look alive.
                if let Some(notice) = source.take_notice() {
                    let terminal = notice.is_terminal();
                    notify(notice);
                    if terminal {
                        break;
                    }
                }
                if let Some(e) = failed {
                    // A half-written file with a real error beats no file.
                    let _ = sink.finish();
                    return Err(e);
                }

                // Repeating the cached frame forever used to look like a working recording.
                let stale_for = last_fresh_at.elapsed();
                if stale_warning_due(stale_for, stale_warnings) {
                    stale_warnings += 1;
                    log::warn!(
                        "no fresh screen frame for {}s — the recording is repeating the last frame",
                        stale_for.as_secs()
                    );
                    notify(CaptureNotice::Interrupted(format!(
                        "The screen has not changed for {}s. If this is wrong, the recording is repeating one frame.",
                        stale_for.as_secs()
                    )));
                }

                let now = Instant::now();
                match cadence {
                    Cadence::Fixed => {
                        let next_tick = tick_at(pacer_base, emitted + 1);
                        if now >= next_tick {
                            let pts = timeline.effective_elapsed().as_micros() as u64;
                            emit(&mut sink, &last_frame, pts, source.as_ref(), &stats)?;
                            emitted += 1;
                            last_emit_at = now;
                            // Emit without sleeping to catch up, or the hitch is permanent.
                            continue;
                        }
                        // Capped at 2ms so fresh frames keep arriving between ticks.
                        let until = (next_tick - now).min(Duration::from_micros(2_000));
                        thread::sleep(until);
                    }
                    Cadence::OnChange { keepalive } => {
                        // A sample that never ends cannot be seeked past.
                        if fresh || now.duration_since(last_emit_at) >= keepalive {
                            let pts = timeline.effective_elapsed().as_micros() as u64;
                            emit(&mut sink, &last_frame, pts, source.as_ref(), &stats)?;
                            emitted += 1;
                            last_emit_at = now;
                        }
                    }
                }
            }

            sink.finish()
        })
        .map_err(Into::into)
}

/// A source under the test's control, so the pacer's contract can be checked
/// without a display.
#[cfg(test)]
struct ScriptedSource {
    width: u32,
    height: u32,
    /// Frames handed out before the source goes quiet; `None` never runs dry.
    remaining: Option<usize>,
    /// Raised after `notice_after` frames, the way an unplugged display would.
    notice: Option<CaptureNotice>,
    notice_after: usize,
    served: usize,
}

#[cfg(test)]
impl ScriptedSource {
    fn new(width: u32, height: u32, remaining: Option<usize>) -> Self {
        Self {
            width,
            height,
            remaining,
            notice: None,
            notice_after: 0,
            served: 0,
        }
    }

    fn raising(mut self, notice: CaptureNotice, after: usize) -> Self {
        self.notice = Some(notice);
        self.notice_after = after;
        self
    }
}

#[cfg(test)]
impl CaptureSource for ScriptedSource {
    fn capture_next(&mut self, _timeout: Duration) -> Result<Option<CapturedFrame>> {
        if let Some(left) = self.remaining.as_mut() {
            if *left == 0 {
                return Ok(None);
            }
            *left -= 1;
        }
        self.served += 1;
        let pixels = vec![0u8; (self.width * self.height * 4) as usize];
        Ok(Some(CapturedFrame::Host(Arc::from(pixels))))
    }

    fn take_notice(&mut self) -> Option<CaptureNotice> {
        if self.served < self.notice_after {
            return None;
        }
        self.notice.take()
    }

    fn width(&self) -> u32 {
        self.width
    }

    fn height(&self) -> u32 {
        self.height
    }
}

#[cfg(test)]
mod pacer_tests {
    use super::*;
    use parking_lot::Mutex;
    use std::sync::atomic::AtomicBool;

    const FPS: u32 = 50;

    /// Runs the real loop for `run` and returns what reached the pipeline.
    fn run_loop(remaining: Option<usize>, run: Duration) -> (PipelineSnapshot, Option<u64>) {
        let source = Box::new(ScriptedSource::new(8, 8, remaining));
        let stop = Arc::new(AtomicBool::new(false));
        let pause = Arc::new(AtomicBool::new(false));
        let pipeline = RecordingPipeline::new(4096);
        let started = Instant::now();
        let video_start = crate::recording::TrackStart::new(started);
        let handle = spawn_capture_loop(
            source,
            CaptureLoop {
                stop_flag: stop.clone(),
                pause_flag: pause.clone(),
                sink: Box::new(QueueSink::new(pipeline.clone())),
                cadence: Cadence::Fixed,
                timeline: crate::recording::RecordingClock::new(started),
                stats: pipeline.stats(),
                target_fps: FPS,
                video_start: video_start.clone(),
            },
            |_| {},
        )
        .expect("the capture thread starts");
        thread::sleep(run);
        stop.store(true, Ordering::Release);
        handle.join().expect("the thread joins").expect("no error");
        (pipeline.stats().snapshot(), video_start.elapsed_us())
    }

    /// The wall-clock contract: one real second of recording is `fps` frames of
    /// video, whatever the source did. A pacer that just forwarded frames would
    /// emit hundreds here, and one that never ticked would emit one.
    #[test]
    fn a_source_faster_than_the_pace_is_still_emitted_at_the_pace() {
        let (stats, _) = run_loop(None, Duration::from_millis(400));
        let expected = FPS as u64 * 400 / 1000;
        assert!(
            (expected / 2..=expected * 2).contains(&stats.captured_frames),
            "expected about {expected} frames at {FPS}fps, got {}",
            stats.captured_frames
        );
        assert_eq!(stats.dropped_frames, 0, "the queue was big enough");
    }

    /// An idle desktop produces nothing after its first frame, and the recording
    /// still has to advance: the encoder is frame-count based, so a gap here
    /// would compress the video against the cursor track.
    #[test]
    fn a_source_that_goes_quiet_keeps_the_recording_advancing() {
        let (stats, _) = run_loop(Some(1), Duration::from_millis(300));
        let expected = FPS as u64 * 300 / 1000;
        assert!(
            stats.captured_frames >= expected / 2,
            "a still screen still owes {expected} frames, got {}",
            stats.captured_frames
        );
    }

    /// Run until the source raises `notice`, and report what the loop did.
    fn run_until_notice(notice: CaptureNotice, after: usize) -> (u64, Vec<CaptureNotice>, bool) {
        let source = Box::new(ScriptedSource::new(8, 8, None).raising(notice, after));
        let stop = Arc::new(AtomicBool::new(false));
        let pause = Arc::new(AtomicBool::new(false));
        let pipeline = RecordingPipeline::new(4096);
        let started = Instant::now();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let collector = Arc::clone(&seen);
        let handle = spawn_capture_loop(
            source,
            CaptureLoop {
                stop_flag: stop.clone(),
                pause_flag: pause.clone(),
                sink: Box::new(QueueSink::new(pipeline.clone())),
                cadence: Cadence::Fixed,
                timeline: crate::recording::RecordingClock::new(started),
                stats: pipeline.stats(),
                target_fps: FPS,
                video_start: crate::recording::TrackStart::new(started),
            },
            move |n| collector.lock().push(n),
        )
        .expect("the capture thread starts");
        thread::sleep(Duration::from_millis(250));
        let exited_on_its_own = handle.is_finished();
        stop.store(true, Ordering::Release);
        handle.join().expect("the thread joins").expect("no error");
        let notices = seen.lock().clone();
        (
            pipeline.stats().snapshot().captured_frames,
            notices,
            exited_on_its_own,
        )
    }

    /// The unplugged-display contract: the loop must STOP, not keep pushing the
    /// last frame. Repeating it to the end is what made a dead capture look
    /// like a working recording.
    #[test]
    fn a_terminal_notice_ends_the_capture_instead_of_repeating_a_frame() {
        let (frames, notices, exited) =
            run_until_notice(CaptureNotice::Ended("display gone".into()), 1);
        assert!(exited, "the loop kept running after the source ended");
        assert!(
            frames < FPS as u64 * 250 / 1000,
            "{frames} frames were pushed after the source ended"
        );
        assert!(notices.iter().any(CaptureNotice::is_terminal));
    }

    /// The user has to be told, or a frozen recording is indistinguishable
    /// from a working one until they play it back.
    #[test]
    fn a_terminal_notice_reaches_the_user_with_its_message() {
        let (_, notices, _) = run_until_notice(
            CaptureNotice::Ended("the display was disconnected".into()),
            1,
        );
        assert!(notices
            .iter()
            .any(|n| n.message().contains("the display was disconnected")));
    }

    /// An interruption is not the end: the source may come back, so the loop
    /// keeps going and the recording keeps its timeline.
    #[test]
    fn an_interruption_is_reported_without_ending_the_capture() {
        let (frames, notices, exited) =
            run_until_notice(CaptureNotice::Interrupted("reopening".into()), 1);
        assert!(!exited, "an interruption must not end the recording");
        assert!(frames > 0);
        assert!(notices.iter().all(|n| !n.is_terminal()));
    }

    /// Video t=0 is the first frame the source produced, not recording start.
    /// The cursor thread blocks on this mark, so leaving it unset would stall
    /// the cursor track entirely rather than merely misplace it.
    #[test]
    fn the_first_frame_records_when_it_arrived() {
        let (stats, offset) = run_loop(None, Duration::from_millis(200));
        assert!(stats.captured_frames > 0);
        let offset = offset.expect("the first frame marks video t=0");
        assert!(
            offset < 200_000,
            "the warmup offset should be the first frame's instant, got {offset}us"
        );
    }

    /// A source that never yields a frame must leave video t=0 UNSET, not 0:
    /// the two used to be the same value, which is why the cursor could not
    /// tell "no frame yet" from "the frame was at zero".
    #[test]
    fn a_source_that_never_delivers_leaves_video_zero_unset() {
        let (_, offset) = run_loop(Some(0), Duration::from_millis(120));
        assert_eq!(offset, None);
    }
}

#[cfg(test)]
mod stale_warning_tests {
    use super::*;

    #[test]
    fn a_healthy_source_never_warns() {
        assert!(!stale_warning_due(Duration::from_millis(16), 0));
        assert!(!stale_warning_due(Duration::from_secs(4), 0));
    }

    #[test]
    fn the_first_warning_lands_at_the_threshold() {
        assert!(stale_warning_due(STALE_FIRST_WARN, 0));
    }

    #[test]
    fn repeats_are_spaced_not_per_tick() {
        assert!(!stale_warning_due(Duration::from_secs(20), 1));
        assert!(stale_warning_due(Duration::from_secs(35), 1));
        assert!(!stale_warning_due(Duration::from_secs(60), 2));
        assert!(stale_warning_due(Duration::from_secs(65), 2));
    }
}

#[cfg(test)]
mod cadence_tests {
    use super::*;
    use crate::recording::{RecordingClock, TrackStart};
    use parking_lot::Mutex;
    use std::sync::atomic::AtomicBool;

    const FPS: u32 = 50;

    /// Keeps every timestamp the loop emitted, which is what the two cadences
    /// actually differ about.
    struct CollectSink(Arc<Mutex<Vec<u64>>>);

    impl FrameSink for CollectSink {
        fn accept(&mut self, _: &CapturedFrame, pts_us: u64, _: u32, _: u32) -> Result<()> {
            self.0.lock().push(pts_us);
            Ok(())
        }
    }

    struct Run {
        stamps: Vec<u64>,
        /// Measured, not the requested sleep: a loaded runner overshoots every
        /// `thread::sleep`, and an assertion against the nominal figure fails
        /// there for reasons that have nothing to do with the code.
        wall: Duration,
        /// How long the clock was actually paused, measured the same way.
        paused: Duration,
    }

    /// Runs the real loop with `cadence`, pausing for `pause` in the middle when
    /// one is given, and returns the timestamps that reached the sink.
    fn run(
        cadence: Cadence,
        frames: Option<usize>,
        run_for: Duration,
        pause: Option<Duration>,
    ) -> Run {
        let source = Box::new(ScriptedSource::new(8, 8, frames));
        let stop = Arc::new(AtomicBool::new(false));
        let pause_flag = Arc::new(AtomicBool::new(false));
        let stamps = Arc::new(Mutex::new(Vec::new()));
        let started = Instant::now();
        let clock = RecordingClock::new(started);
        let handle = spawn_capture_loop(
            source,
            CaptureLoop {
                stop_flag: stop.clone(),
                pause_flag: pause_flag.clone(),
                sink: Box::new(CollectSink(Arc::clone(&stamps))),
                cadence,
                timeline: clock.clone(),
                stats: PipelineStats::default(),
                target_fps: FPS,
                video_start: TrackStart::new(started),
            },
            |_| {},
        )
        .expect("the capture thread starts");

        let began = Instant::now();
        let mut paused = Duration::ZERO;
        match pause {
            Some(held) => {
                thread::sleep(run_for / 2);
                pause_flag.store(true, Ordering::Release);
                clock.pause();
                let at = Instant::now();
                thread::sleep(held);
                paused = at.elapsed();
                clock.resume();
                pause_flag.store(false, Ordering::Release);
                thread::sleep(run_for / 2);
            }
            None => thread::sleep(run_for),
        }
        stop.store(true, Ordering::Release);
        handle.join().expect("the thread joins").expect("no error");
        let wall = began.elapsed();
        let stamps = stamps.lock().clone();
        Run {
            stamps,
            wall,
            paused,
        }
    }

    /// The whole point of `OnChange`: a source that produced 3 frames owes 3
    /// samples, not one per pacer slot. `Fixed` in the same conditions emits
    /// dozens, which is what the assertion below is measured against.
    #[test]
    fn on_change_emits_what_the_source_produced_not_one_per_slot() {
        let quiet = run(
            Cadence::OnChange {
                keepalive: Duration::from_secs(60),
            },
            Some(3),
            Duration::from_millis(300),
            None,
        );
        assert!(
            quiet.stamps.len() <= 4,
            "a source with 3 frames owes at most 4 samples, got {}",
            quiet.stamps.len()
        );

        let paced = run(Cadence::Fixed, Some(3), Duration::from_millis(300), None);
        assert!(
            paced.stamps.len() > quiet.stamps.len() * 2,
            "fixed pacing must keep emitting while the source is quiet: {} vs {}",
            paced.stamps.len(),
            quiet.stamps.len()
        );
    }

    /// A sample that never ends cannot be seeked past, so a still desktop still
    /// owes one every keepalive.
    #[test]
    fn a_still_source_still_owes_a_keepalive_sample() {
        let still = run(
            Cadence::OnChange {
                keepalive: Duration::from_millis(40),
            },
            Some(1),
            Duration::from_millis(300),
            None,
        );
        assert!(
            still.stamps.len() >= 4,
            "300ms of stillness at a 40ms keepalive owes several samples, got {}",
            still.stamps.len()
        );
    }

    /// Timestamps come off the paused-aware clock, so a pause is CUT from the
    /// recording rather than held as a freeze. Stamping from the raw origin
    /// would put the last sample past the end of the recorded time.
    #[test]
    fn a_pause_is_cut_out_of_the_timestamps_rather_than_frozen_into_them() {
        let held = Duration::from_millis(200);
        let run = run(
            Cadence::OnChange {
                keepalive: Duration::from_millis(20),
            },
            None,
            Duration::from_millis(200),
            Some(held),
        );
        let last = *run.stamps.last().expect("samples were emitted");
        // Everything is measured, so this holds however far the sleeps overshot.
        let recorded = (run.wall - run.paused).as_micros() as u64;
        assert!(
            run.paused >= held / 2,
            "the pause did not happen: {:?}",
            run.paused
        );
        assert!(
            last <= recorded + 60_000,
            "the last sample is at {last}us of a {recorded}us recording (wall {:?}, \
             paused {:?}), so the pause was stamped into it",
            run.wall,
            run.paused
        );
    }
}
