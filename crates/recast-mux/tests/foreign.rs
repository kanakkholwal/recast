use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;

use recast_mux::{Mp4Reader, TrackKind};

const WIDTH: u16 = 160;
const HEIGHT: u16 = 120;
const FPS: u32 = 25;
const FRAMES: u32 = 50;

/// Reading back what our own writer produced only proves the two agree. This
/// suite is the other half: a file laid out by somebody else, with its own
/// timescales, its own `stsc` runs and its own box order.
fn foreign_file(name: &str, args: &[&str]) -> Option<PathBuf> {
    let Some(ffmpeg) = recast_testkit::ffmpeg_path() else {
        // Loud, and never silent: a suite that skips itself looks identical to
        // one that passes, which is how the ffprobe tests here once proved
        // nothing for four green runs.
        eprintln!("skipping: no ffmpeg sidecar to build a foreign file with");
        return None;
    };
    let dir = std::env::temp_dir().join("recast-mux-tests");
    std::fs::create_dir_all(&dir).ok()?;
    let path = dir.join(name);
    let ok = Command::new(&ffmpeg)
        .args(["-y", "-f", "lavfi", "-i"])
        .arg(format!("testsrc=size={WIDTH}x{HEIGHT}:rate={FPS}"))
        .args(["-f", "lavfi", "-i", "sine=frequency=440:sample_rate=48000"])
        .args(["-frames:v", &FRAMES.to_string(), "-shortest"])
        .args(args)
        .arg(&path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !ok {
        eprintln!("skipping: ffmpeg would not write {name}");
        return None;
    }
    path.exists().then_some(path)
}

fn read(path: &Path) -> Vec<u8> {
    std::fs::read(path).expect("the file is readable")
}

/// Built once. Tests run in parallel, and four of them racing ffmpeg to write
/// the same path means one reads a file another is still truncating.
fn plain() -> Option<&'static Path> {
    static FILE: OnceLock<Option<PathBuf>> = OnceLock::new();
    FILE.get_or_init(|| {
        foreign_file(
            "foreign.mp4",
            &[
                "-c:v",
                "libx264",
                "-preset",
                "ultrafast",
                "-bf",
                "0",
                "-c:a",
                "aac",
            ],
        )
    })
    .as_deref()
}

#[test]
fn a_file_written_by_ffmpeg_reads_back() {
    let Some(path) = plain() else { return };
    let data = read(path);
    let reader = Mp4Reader::new(&data).expect("the file parses");

    let video = reader.video().expect("a video track");
    assert_eq!(&video.format, b"avc1");
    assert_eq!((video.width, video.height), (WIDTH, HEIGHT));
    assert_eq!(video.samples.len(), FRAMES as usize);
    assert!(
        !video.decoder_config.is_empty(),
        "no avcC came out of a foreign file"
    );

    let audio = reader.audio().expect("an audio track");
    assert_eq!(&audio.format, b"mp4a");
    assert_eq!(audio.sample_rate, 48_000);
    assert_eq!(audio.kind, TrackKind::Audio);
    assert!(
        audio.decoder_config.len() >= 2,
        "no AudioSpecificConfig came out of a foreign esds"
    );
}

/// Every sample has to sit inside the file and inside `mdat`. Offsets that
/// merely look plausible are what a wrong `stsc` produces.
#[test]
fn every_foreign_sample_points_somewhere_real() {
    let Some(path) = plain() else { return };
    let data = read(path);
    let reader = Mp4Reader::new(&data).expect("the file parses");
    for track in reader.tracks() {
        for (index, sample) in track.samples.iter().enumerate() {
            let bytes = reader
                .sample_data(sample)
                .unwrap_or_else(|| panic!("sample {index} points outside the file"));
            assert_eq!(bytes.len(), sample.size as usize);
        }
    }
}

/// The first NAL of an AVCC sample is length-prefixed, and the length has to
/// account for the whole payload. A sample boundary off by even one byte breaks
/// this without moving any count.
#[test]
fn foreign_video_samples_are_whole_avcc_units() {
    let Some(path) = plain() else { return };
    let data = read(path);
    let reader = Mp4Reader::new(&data).expect("the file parses");
    let video = reader.video().unwrap();
    // The length prefix width lives in the last two bits of avcC byte 4.
    let prefix = (video.decoder_config[4] & 0x03) as usize + 1;
    assert_eq!(prefix, 4, "an unusual NAL length size: {prefix}");

    for (index, sample) in video.samples.iter().enumerate() {
        let bytes = reader.sample_data(sample).expect("bytes");
        let mut at = 0usize;
        while at + prefix <= bytes.len() {
            let length = u32::from_be_bytes(bytes[at..at + 4].try_into().unwrap()) as usize;
            at += prefix + length;
        }
        assert_eq!(
            at,
            bytes.len(),
            "sample {index} did not end on a NAL boundary"
        );
    }
}

#[test]
fn decode_times_of_a_foreign_track_add_up_to_its_duration() {
    let Some(path) = plain() else { return };
    let data = read(path);
    let reader = Mp4Reader::new(&data).expect("the file parses");
    for track in reader.tracks() {
        let summed: u64 = track.samples.iter().map(|s| s.duration as u64).sum();
        // Within one sample: the last sample's duration is often an estimate.
        let slack = track.samples.last().map(|s| s.duration as u64).unwrap_or(0);
        assert!(
            summed.abs_diff(track.duration) <= slack,
            "{:?} summed to {summed} against a declared {}",
            track.kind,
            track.duration
        );
    }
}

/// B frames put decode and presentation order out of step, which is what `ctts`
/// exists for. Without it a player shows the frames scrambled.
#[test]
fn a_file_with_b_frames_carries_composition_offsets() {
    static FILE: OnceLock<Option<PathBuf>> = OnceLock::new();
    let Some(path) = FILE
        .get_or_init(|| {
            foreign_file(
                "foreign-bframes.mp4",
                &[
                    "-c:v", "libx264", "-preset", "medium", "-bf", "3", "-c:a", "aac",
                ],
            )
        })
        .as_deref()
    else {
        return;
    };
    let data = read(path);
    let reader = Mp4Reader::new(&data).expect("the file parses");
    let video = reader.video().unwrap();
    assert!(
        video.samples.iter().any(|s| s.composition_offset != 0),
        "no composition offsets in a stream encoded with B frames"
    );
    // Presentation order is a permutation of decode order, so the set of
    // presentation times has to be as large as the sample count.
    let mut times: Vec<i64> = video
        .samples
        .iter()
        .map(|s| s.presentation_time())
        .collect();
    times.sort_unstable();
    times.dedup();
    assert_eq!(times.len(), video.samples.len(), "two frames share a time");
}

/// Non-square pixels make the track header's display width twice the coded
/// width. A decoder fed the display size reads the wrong number of bytes per
/// row, so the coded one has to win.
#[test]
fn the_coded_size_wins_over_a_stretched_display_size() {
    static FILE: OnceLock<Option<PathBuf>> = OnceLock::new();
    let Some(path) = FILE
        .get_or_init(|| {
            foreign_file(
                "foreign-sar.mp4",
                &[
                    "-vf",
                    "setsar=2/1",
                    "-c:v",
                    "libx264",
                    "-preset",
                    "ultrafast",
                    "-bf",
                    "0",
                    "-an",
                ],
            )
        })
        .as_deref()
    else {
        return;
    };
    let data = read(path);
    let reader = Mp4Reader::new(&data).expect("the file parses");
    let video = reader.video().unwrap();
    assert_eq!(
        (video.width, video.height),
        (WIDTH, HEIGHT),
        "the display size leaked through instead of the coded size"
    );
}
