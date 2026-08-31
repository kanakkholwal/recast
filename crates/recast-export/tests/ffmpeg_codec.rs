//! The FFmpeg codec backend, exercised against a real binary.
//! Run on every platform on purpose: it is the export path for macOS and Linux, and a Windows-only CI would never execute a line of it.

use std::path::Path;

use recast_compositor::{PlaneData, SourceColor};
use recast_export::{FfmpegPictures, FfmpegSink, PictureSource, SourceInfo};
use recast_testkit::Scratch;

const W: u32 = 320;
const H: u32 = 180;
const FPS: f64 = 10.0;
const SECONDS: u32 = 2;

/// Skipping silently is the failure mode this file exists to prevent: with no
/// binary every test below passes in zero seconds. CI sets
/// RECAST_TESTKIT_REQUIRE_FFMPEG=1 so a missing sidecar fails the run instead.
macro_rules! ffmpeg_or_skip {
    () => {
        match recast_testkit::media::ffmpeg_path() {
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

/// A clip whose brightness climbs frame by frame, so a decoded frame says which
/// frame it is. `testsrc` would too, but only to a human reading it.
fn record(ffmpeg: &Path, path: &Path) {
    let status = std::process::Command::new(ffmpeg)
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            &format!("color=c=black:s={W}x{H}:r={FPS}:d={SECONDS}"),
            "-vf",
            // n is the frame index, so luma rises by 10 a frame from 10.
            "geq=lum='(N+1)*10':cb=128:cr=128",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            &path.to_string_lossy(),
        ])
        .status()
        .expect("ffmpeg runs");
    assert!(status.success(), "the fixture clip did not encode");
}

fn info() -> SourceInfo {
    SourceInfo {
        width: W,
        height: H,
        fps: FPS,
    }
}

/// Mean luma of the frame a source hands back at `t`.
fn luma_at(pictures: &mut FfmpegPictures, t: f64) -> f64 {
    let planes = pictures
        .picture_at(t)
        .expect("the decode runs")
        .expect("a frame");
    let PlaneData::Packed(data) = planes.data else {
        panic!("the backend produced planar data");
    };
    let luma = (W * H) as usize;
    data[..luma].iter().map(|&b| f64::from(b)).sum::<f64>() / luma as f64
}

#[test]
fn a_recording_decodes_frame_by_frame_in_order() {
    let ffmpeg = ffmpeg_or_skip!();
    let scratch = Scratch::new("ffmpeg-decode");
    let input = scratch.file("in.mp4");
    record(&ffmpeg, &input);

    let mut pictures =
        FfmpegPictures::open(&ffmpeg, &input, info(), SourceColor::default()).expect("opens");
    assert_eq!((pictures.width(), pictures.height()), (W, H));

    let first = luma_at(&mut pictures, 0.0);
    let later = luma_at(&mut pictures, 1.0);
    assert!(
        later > first + 50.0,
        "the decode did not advance: {first:.1} then {later:.1}"
    );
}

/// The loop walks the source backwards whenever a cut or a speed ramp does, and
/// a raw pipe cannot rewind, so the backend has to restart the decode.
#[test]
fn seeking_backwards_returns_the_earlier_frame_again() {
    let ffmpeg = ffmpeg_or_skip!();
    let scratch = Scratch::new("ffmpeg-seek");
    let input = scratch.file("in.mp4");
    record(&ffmpeg, &input);

    let mut pictures =
        FfmpegPictures::open(&ffmpeg, &input, info(), SourceColor::default()).expect("opens");
    let early = luma_at(&mut pictures, 0.2);
    let late = luma_at(&mut pictures, 1.6);
    let again = luma_at(&mut pictures, 0.2);

    assert!(late > early, "the decode never advanced");
    assert!(
        (again - early).abs() < 12.0,
        "the rewind landed elsewhere: {early:.1} then {again:.1}"
    );
}

/// Reading past the end must hold the last frame rather than go blank, which is
/// what the native reader does and what the frame walk relies on.
#[test]
fn reading_past_the_end_holds_the_last_frame() {
    let ffmpeg = ffmpeg_or_skip!();
    let scratch = Scratch::new("ffmpeg-tail");
    let input = scratch.file("in.mp4");
    record(&ffmpeg, &input);

    let mut pictures =
        FfmpegPictures::open(&ffmpeg, &input, info(), SourceColor::default()).expect("opens");
    let last = luma_at(&mut pictures, f64::from(SECONDS) - 0.05);
    let past = luma_at(&mut pictures, f64::from(SECONDS) + 5.0);
    assert!(
        (past - last).abs() < 12.0,
        "the tail changed: {last:.1} then {past:.1}"
    );
}

#[test]
fn a_source_with_no_frame_rate_is_refused_before_anything_spawns() {
    let ffmpeg = ffmpeg_or_skip!();
    let bad = SourceInfo { fps: 0.0, ..info() };
    let opened = FfmpegPictures::open(
        &ffmpeg,
        Path::new("nowhere.mp4"),
        bad,
        SourceColor::default(),
    );
    assert!(
        matches!(opened, Err(recast_export::FfmpegError::NoFrameRate)),
        "a zero rate cannot index frames"
    );
}

#[test]
fn rendered_frames_encode_into_a_playable_file() {
    let ffmpeg = ffmpeg_or_skip!();
    let scratch = Scratch::new("ffmpeg-encode");
    let output = scratch.file("out.mp4");

    let mut sink =
        FfmpegSink::new(&ffmpeg, &output, W, H, (10, 1), 800_000).expect("the encoder opens");
    let frames = 20u32;
    for index in 0..frames {
        // A ramp across the clip, so a decoder can tell the frames apart.
        let value = (index * 10) as u8;
        let frame = vec![value; (W * H * 4) as usize];
        sink.push(&frame).expect("the frame is written");
    }
    sink.finish().expect("the encode finishes");

    assert!(output.exists(), "no file was written");
    assert!(
        std::fs::metadata(&output).expect("metadata").len() > 1024,
        "the file is too small to hold {frames} frames"
    );

    // Decoded back through the other half of the backend, so the pair round-trips.
    let mut pictures =
        FfmpegPictures::open(&ffmpeg, &output, info(), SourceColor::default()).expect("opens");
    let first = luma_at(&mut pictures, 0.0);
    let later = luma_at(&mut pictures, 1.5);
    assert!(
        later > first + 50.0,
        "the encoded ramp did not survive: {first:.1} then {later:.1}"
    );
}

/// A frame smaller than the geometry is a torn picture, and writing it would
/// desynchronise every frame after it rather than fail.
#[test]
fn a_short_frame_is_refused_rather_than_written() {
    let ffmpeg = ffmpeg_or_skip!();
    let scratch = Scratch::new("ffmpeg-short");
    let output = scratch.file("out.mp4");

    let mut sink =
        FfmpegSink::new(&ffmpeg, &output, W, H, (10, 1), 800_000).expect("the encoder opens");
    let error = sink
        .push(&[0u8; 16])
        .expect_err("a short frame cannot be written");
    assert!(matches!(
        error,
        recast_export::FfmpegError::FrameSize { .. }
    ));
}
