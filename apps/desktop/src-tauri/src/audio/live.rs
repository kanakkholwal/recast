//! Runtime checks against the machine's real audio devices.
//!
//! Everything else in `audio/` runs on synthetic buffers, which cannot answer
//! the question that decides A/V sync: does a track cover exactly the wall
//! clock between its own start and the stop? These open the actual loopback and
//! microphone, so they are `#[ignore]`d and run by hand:
//!
//! ```text
//! cargo test --lib audio::live -- --ignored --nocapture
//! ```

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use super::{
    AudioCaptureConfig, AudioCaptureSession, MicrophoneCaptureConfig, MicrophoneCaptureSession,
};
use crate::audio::wav::wav_data_bytes;
use crate::recording::clock::TrackStart;

/// Past the 5 s drift floor, so a take exercises the re-declaration too.
const TAKE: Duration = Duration::from_secs(12);

/// How far a track may sit from the clock before it is a sync problem.
///
/// A stop is noticed within one `POLL_TIMEOUT` and the open costs a few
/// milliseconds either side. Still well under the ~45 ms a viewer notices.
const TOLERANCE: Duration = Duration::from_millis(300);

/// What one finished track claims about itself.
struct Track {
    name: &'static str,
    rate: u32,
    channels: u16,
    frames: u64,
    /// Wall clock from the session origin to this track's first sample.
    began: Duration,
    /// How long the samples play for, at the rate the header declares.
    covered: Duration,
}

impl Track {
    fn read(name: &'static str, path: &Path, start: &TrackStart) -> Self {
        let header = std::fs::read(path).expect("the finished WAV");
        let rate = u32::from_le_bytes(header[24..28].try_into().expect("rate field"));
        let channels = u16::from_le_bytes(header[22..24].try_into().expect("channel field"));
        let bits = u16::from_le_bytes(header[34..36].try_into().expect("depth field"));
        let block = u64::from(channels) * u64::from(bits / 8);
        let frames = wav_data_bytes(path).expect("a readable WAV") / block.max(1);
        Self {
            name,
            rate,
            channels,
            frames,
            began: Duration::from_micros(start.elapsed_us().expect("the track was marked")),
            covered: Duration::from_secs_f64(frames as f64 / f64::from(rate.max(1))),
        }
    }

    fn report(&self) {
        println!(
            "{}: {} Hz, {} ch, {} frames, begins at {:.3}s, covers {:.3}s",
            self.name,
            self.rate,
            self.channels,
            self.frames,
            self.began.as_secs_f64(),
            self.covered.as_secs_f64()
        );
    }

    /// The invariant that decides sync: sample zero sits where the track was
    /// marked, and the samples run from there to the stop. A track that fails
    /// this drifts against the picture by exactly the difference.
    fn assert_reaches(&self, stop: Duration, tolerance: Duration) {
        assert!(self.frames > 0, "{} produced no samples at all", self.name);
        let end = self.began + self.covered;
        let slack = end.abs_diff(stop);
        assert!(
            slack <= tolerance,
            "{} covers {:.3}s from {:.3}s, ending at {:.3}s, but the take ended at {:.3}s — {:.0}ms out of sync",
            self.name,
            self.covered.as_secs_f64(),
            self.began.as_secs_f64(),
            end.as_secs_f64(),
            stop.as_secs_f64(),
            slack.as_secs_f64() * 1000.0,
        );
    }
}

/// Both tracks of one take, plus how long the take actually ran.
struct Take {
    loopback: Option<Track>,
    microphone: Option<Track>,
    elapsed: Duration,
}

/// Record both tracks, running `during` while they capture.
///
/// Returns whatever opened: a machine with no microphone still exercises the
/// loopback, and a skipped half says so rather than failing.
fn record(tag: &str, during: impl FnOnce(&Arc<AtomicBool>)) -> Take {
    let dir = std::env::temp_dir().join(format!("recast-audio-live-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let origin = Instant::now();
    let pause = Arc::new(AtomicBool::new(false));
    let (loopback_start, mic_start) = (TrackStart::new(origin), TrackStart::new(origin));

    let loopback = AudioCaptureSession::start(AudioCaptureConfig {
        output_path: dir.join(format!("{tag}-system.wav")),
        pause_flag: pause.clone(),
        start: loopback_start.clone(),
    });
    if loopback.is_none() {
        println!("no loopback device on this machine; skipping that half");
    }
    let microphone = MicrophoneCaptureSession::start(MicrophoneCaptureConfig {
        output_path: dir.join(format!("{tag}-mic.wav")),
        device_id: None,
        pause_flag: pause.clone(),
        start: mic_start.clone(),
    })
    .map_err(|err| println!("no microphone on this machine ({err:#}); skipping that half"))
    .ok();

    during(&pause);
    let elapsed = origin.elapsed();

    let read = |name, stopped: Option<PathBuf>, start: &TrackStart| {
        stopped.map(|path| Track::read(name, &path, start))
    };
    let stopped_loopback = loopback.map(|s| s.stop().expect("the loopback track finished"));
    let stopped_mic = microphone.map(|s| s.stop().expect("the microphone track finished"));
    Take {
        loopback: read("system audio", stopped_loopback, &loopback_start),
        microphone: read("microphone", stopped_mic, &mic_start),
        elapsed,
    }
}

impl Take {
    fn tracks(&self) -> Vec<&Track> {
        [self.loopback.as_ref(), self.microphone.as_ref()]
            .into_iter()
            .flatten()
            .collect()
    }
}

/// The whole point of the migration, measured rather than reasoned about.
///
/// Nothing needs to be playing: an idle loopback delivers no buffers at all, so
/// a track that still covers the take is capturekit's inserted silence landing
/// in the right place.
#[test]
#[ignore = "opens the real audio devices; run with --ignored"]
fn a_take_covers_the_wall_clock_it_was_recorded_over() {
    let take = record("plain", |_| std::thread::sleep(TAKE));
    let tracks = take.tracks();
    assert!(!tracks.is_empty(), "no audio device at all on this machine");
    println!("take ran for {:.3}s", take.elapsed.as_secs_f64());
    for track in tracks {
        track.report();
        track.assert_reaches(take.elapsed, TOLERANCE);
    }
}

/// A paused stretch must leave the track shorter, not leave a hole in it: the
/// picture skips those seconds too, and a track that kept writing across them
/// puts every sound after the pause late by its length.
#[test]
#[ignore = "opens the real audio devices; run with --ignored"]
fn a_pause_shortens_the_track_by_exactly_its_length() {
    let paused_for = Duration::from_secs(4);
    let recorded_for = TAKE - paused_for;
    let take = record("paused", |pause| {
        std::thread::sleep(recorded_for / 2);
        pause.store(true, Ordering::Release);
        std::thread::sleep(paused_for);
        pause.store(false, Ordering::Release);
        std::thread::sleep(recorded_for / 2);
    });
    let tracks = take.tracks();
    assert!(!tracks.is_empty(), "no audio device at all on this machine");
    println!(
        "take ran for {:.3}s, of which {:.3}s paused",
        take.elapsed.as_secs_f64(),
        paused_for.as_secs_f64()
    );
    for track in tracks {
        track.report();
        // Two flag transitions, each noticed within a poll, on top of the
        // ordinary slack.
        track.assert_reaches(take.elapsed - paused_for, TOLERANCE * 2);
    }
}

/// A user records more than once per app run, so the devices are opened,
/// released and opened again in one process.
#[test]
#[ignore = "opens the real audio devices; run with --ignored"]
fn a_second_take_opens_the_devices_again() {
    for attempt in 0..3 {
        eprintln!("take {attempt}: opening");
        let take = record(&format!("repeat-{attempt}"), |_| {
            std::thread::sleep(Duration::from_secs(1));
        });
        eprintln!("take {attempt}: stopped");
        for track in take.tracks() {
            track.report();
            assert!(track.frames > 0, "{} produced nothing", track.name);
        }
    }
}

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("recast-audio-live-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir.join(format!("{tag}.wav"))
}

#[test]
#[ignore = "opens the real audio devices; run with --ignored"]
fn the_loopback_alone_survives_repeated_takes() {
    for attempt in 0..3 {
        eprintln!("loopback take {attempt}: opening");
        let Some(session) = AudioCaptureSession::start(AudioCaptureConfig {
            output_path: scratch(&format!("loop-only-{attempt}")),
            pause_flag: Arc::new(AtomicBool::new(false)),
            start: TrackStart::new(Instant::now()),
        }) else {
            eprintln!("skipped: no loopback endpoint");
            return;
        };
        std::thread::sleep(Duration::from_millis(400));
        session.stop().expect("released");
        eprintln!("loopback take {attempt}: released");
    }
}

#[test]
#[ignore = "opens the real audio devices; run with --ignored"]
fn the_microphone_alone_survives_repeated_takes() {
    for attempt in 0..3 {
        eprintln!("mic take {attempt}: opening");
        let session = MicrophoneCaptureSession::start(MicrophoneCaptureConfig {
            output_path: scratch(&format!("mic-only-{attempt}")),
            device_id: None,
            pause_flag: Arc::new(AtomicBool::new(false)),
            start: TrackStart::new(Instant::now()),
        });
        let Ok(session) = session else {
            eprintln!("skipped: no microphone endpoint");
            return;
        };
        std::thread::sleep(Duration::from_millis(400));
        session.stop().expect("released");
        eprintln!("mic take {attempt}: released");
    }
}
