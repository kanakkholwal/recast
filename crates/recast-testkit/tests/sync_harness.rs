use std::path::PathBuf;

use recast_testkit::{audio, media, timecode, SourceSpec};

struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("recast-testkit-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch dir");
        Self(dir)
    }

    fn file(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Skipping silently is the failure mode this harness exists to prevent, so CI
/// sets RECAST_TESTKIT_REQUIRE_FFMPEG=1 and a missing binary fails the run.
macro_rules! ffmpeg_or_skip {
    () => {
        match media::ffmpeg_path() {
            Some(path) => path,
            None if std::env::var("RECAST_TESTKIT_REQUIRE_FFMPEG").as_deref() == Ok("1") => {
                panic!("RECAST_TESTKIT_REQUIRE_FFMPEG=1 but no ffmpeg was found")
            }
            None => {
                eprintln!("skipping: no ffmpeg (set RECAST_FFMPEG to run this)");
                return;
            }
        }
    };
}

#[test]
fn every_decoded_frame_carries_its_own_index() {
    let ffmpeg = ffmpeg_or_skip!();
    let scratch = Scratch::new("frame-index");
    let out = scratch.file("source.mp4");
    let spec = SourceSpec {
        width: 640,
        height: 360,
        fps: 30,
        duration_secs: 3.0,
        ..Default::default()
    };

    media::write_source(&ffmpeg, spec, &out).expect("encode");
    let frames = media::read_frames(&ffmpeg, &out, spec.width, spec.height).expect("decode");

    assert_eq!(
        frames.len() as u64,
        spec.frame_count(),
        "decoded {} frames, expected {}",
        frames.len(),
        spec.frame_count()
    );

    for (index, frame) in frames.iter().enumerate() {
        let decoded = timecode::decode_frame(frame, spec.width, spec.height);
        assert_eq!(
            decoded,
            Some(index as u64),
            "frame at position {index} decoded as {decoded:?}"
        );
    }
}

#[test]
fn a_three_second_source_is_three_seconds_of_frames() {
    let ffmpeg = ffmpeg_or_skip!();
    let scratch = Scratch::new("duration");
    let out = scratch.file("source.mp4");
    let spec = SourceSpec {
        fps: 30,
        duration_secs: 3.0,
        ..Default::default()
    };

    media::write_source(&ffmpeg, spec, &out).expect("encode");
    let frames = media::read_frames(&ffmpeg, &out, spec.width, spec.height).expect("decode");

    let seconds = frames.len() as f64 / spec.fps as f64;
    assert!(
        (seconds - spec.duration_secs).abs() < 1.0 / spec.fps as f64,
        "video is {seconds:.3}s, expected {:.3}s",
        spec.duration_secs
    );
}

#[test]
fn clicks_stay_on_the_second_through_an_encode() {
    let ffmpeg = ffmpeg_or_skip!();
    let scratch = Scratch::new("audio-sync");
    let out = scratch.file("source.mp4");
    let spec = SourceSpec {
        duration_secs: 4.0,
        ..Default::default()
    };

    media::write_source(&ffmpeg, spec, &out).expect("encode");
    let samples =
        media::read_samples(&ffmpeg, &out, spec.sample_rate, spec.channels).expect("decode audio");
    let clicks = audio::detect_clicks(&samples, spec.sample_rate, spec.channels);

    let expected = spec.expected_clicks();
    assert_eq!(
        clicks.len(),
        expected.len(),
        "clicks {clicks:?}, expected {expected:?}"
    );

    let drift = audio::worst_click_drift(&clicks, &expected).expect("same count");
    assert!(drift < 0.030, "worst click drift {drift:.4}s exceeds 30ms");
}

#[test]
fn audio_and_video_agree_on_how_long_the_source_is() {
    let ffmpeg = ffmpeg_or_skip!();
    let scratch = Scratch::new("av-length");
    let out = scratch.file("source.mp4");
    let spec = SourceSpec {
        fps: 30,
        duration_secs: 3.0,
        ..Default::default()
    };

    media::write_source(&ffmpeg, spec, &out).expect("encode");
    let frames = media::read_frames(&ffmpeg, &out, spec.width, spec.height).expect("decode video");
    let samples =
        media::read_samples(&ffmpeg, &out, spec.sample_rate, spec.channels).expect("decode audio");

    let video_secs = frames.len() as f64 / spec.fps as f64;
    let audio_secs = samples.len() as f64 / (spec.sample_rate as f64 * spec.channels as f64);
    assert!(
        (video_secs - audio_secs).abs() < 0.050,
        "video {video_secs:.3}s vs audio {audio_secs:.3}s"
    );
}

#[test]
fn a_dropped_frame_is_detected_rather_than_averaged_away() {
    let ffmpeg = ffmpeg_or_skip!();
    let scratch = Scratch::new("drop-detect");
    let out = scratch.file("source.mp4");
    let spec = SourceSpec {
        fps: 30,
        duration_secs: 2.0,
        ..Default::default()
    };

    media::write_source(&ffmpeg, spec, &out).expect("encode");
    let mut frames = media::read_frames(&ffmpeg, &out, spec.width, spec.height).expect("decode");

    frames.remove(10);
    let mismatch = frames
        .iter()
        .enumerate()
        .find(|(index, frame)| {
            timecode::decode_frame(frame, spec.width, spec.height) != Some(*index as u64)
        })
        .map(|(index, _)| index);

    assert_eq!(
        mismatch,
        Some(10),
        "the harness must catch a single dropped frame"
    );
}
