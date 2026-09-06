#![cfg(windows)]

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use recast_codec_mf::{AudioFormat, AudioReader};

const TONE_HZ: f64 = 440.0;
const SECONDS: f64 = 2.0;

/// ffmpeg is the fixture generator here, not a dependency of the code under
/// test: it writes a file with a known tone that our reader has to decode.
fn tone_file(name: &str, args: &[&str]) -> Option<PathBuf> {
    let ffmpeg = recast_testkit::ffmpeg_path()?;
    let dir = std::env::temp_dir().join("recast-codec-mf-tests");
    std::fs::create_dir_all(&dir).ok()?;
    let path = dir.join(name);
    let ok = Command::new(&ffmpeg)
        .args([
            "-y",
            "-f",
            "lavfi",
            "-i",
            &format!("sine=frequency={TONE_HZ}:duration={SECONDS}:sample_rate=44100"),
        ])
        .args(args)
        .arg(&path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !ok {
        eprintln!("skipping: ffmpeg could not write {name}");
        return None;
    }
    Some(path)
}

fn crossings(samples: &[f32], channels: usize) -> usize {
    let left: Vec<f32> = samples.iter().copied().step_by(channels).collect();
    left.windows(2)
        .filter(|w| (w[0] <= 0.0) != (w[1] <= 0.0))
        .count()
}

fn read(path: &Path, format: AudioFormat) -> Vec<f32> {
    let mut reader = AudioReader::open(path, format)
        .expect("the file opens")
        .expect("the file has audio");
    reader.read_all().expect("the audio reads")
}

/// The reader is asked for 48 kHz stereo whatever the file holds, so a 44.1 kHz
/// mono source has to come back converted rather than at its own rate.
#[test]
fn a_source_at_another_rate_is_converted_to_the_master_format() {
    let Some(path) = tone_file("tone.m4a", &["-c:a", "aac", "-ac", "1"]) else {
        return;
    };
    let format = AudioFormat::default();
    let samples = read(&path, format);

    let frames = samples.len() / format.channels as usize;
    let expected = (SECONDS * format.sample_rate as f64) as usize;
    assert!(
        frames.abs_diff(expected) < format.sample_rate as usize / 10,
        "{frames} frames against an expected {expected}"
    );
}

/// Counting samples proves plumbing; counting zero crossings proves it is the
/// actual tone and not silence or noise.
#[test]
fn the_decoded_tone_is_at_the_frequency_it_was_written_at() {
    let Some(path) = tone_file("tone.wav", &["-c:a", "pcm_s16le", "-ac", "1"]) else {
        return;
    };
    let format = AudioFormat::default();
    let samples = read(&path, format);
    assert!(!samples.is_empty(), "the reader returned nothing");

    let frames = samples.len() / format.channels as usize;
    let per_second = crossings(&samples, format.channels as usize) as f64
        * format.sample_rate as f64
        / frames as f64;
    // A sine crosses zero twice per cycle.
    let expected = TONE_HZ * 2.0;
    assert!(
        (per_second - expected).abs() < expected * 0.05,
        "decoded {per_second} crossings per second, expected about {expected}"
    );
}

#[test]
fn a_file_with_no_audio_reports_none_rather_than_failing() {
    let Some(ffmpeg) = recast_testkit::ffmpeg_path() else {
        eprintln!("skipping: no ffmpeg sidecar");
        return;
    };
    let dir = std::env::temp_dir().join("recast-codec-mf-tests");
    std::fs::create_dir_all(&dir).expect("a temp dir");
    let path = dir.join("silent.mp4");
    let ok = Command::new(&ffmpeg)
        .args([
            "-y",
            "-f",
            "lavfi",
            "-i",
            "testsrc=size=64x64:rate=10",
            "-frames:v",
            "10",
            "-c:v",
            "libx264",
            "-an",
        ])
        .arg(&path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !ok {
        eprintln!("skipping: ffmpeg could not write a silent file");
        return;
    }
    let reader = AudioReader::open(&path, AudioFormat::default()).expect("the file opens");
    assert!(reader.is_none(), "a video-only file reported audio");
}

/// `Ok(None)` is read by every caller as "this file has no audio", so a stream
/// that exists must never produce it: refusing to decode one is an error.
/// Which codecs Media Foundation covers is a property of the machine, so the
/// invariant is asserted over whatever it does cover rather than one codec.
#[test]
fn a_stream_that_exists_never_reads_as_no_audio() {
    let cases: &[(&str, &[&str])] = &[
        ("inv-opus.mp4", &["-c:a", "libopus", "-f", "mp4"]),
        ("inv-flac.flac", &["-c:a", "flac", "-f", "flac"]),
        ("inv-alac.m4a", &["-c:a", "alac", "-f", "ipod"]),
        ("inv-vorbis.ogg", &["-c:a", "libvorbis", "-f", "ogg"]),
        ("inv-aac.m4a", &["-c:a", "aac", "-f", "ipod"]),
    ];
    let mut checked = 0;
    for (name, args) in cases {
        let Some(path) = tone_file(name, args) else {
            continue;
        };
        checked += 1;
        assert!(
            !matches!(AudioReader::open(&path, AudioFormat::default()), Ok(None)),
            "{name} has an audio stream but read as having none"
        );
    }
    assert!(
        checked > 0,
        "no fixture was written, so nothing was asserted"
    );
}
