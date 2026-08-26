use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use recast_mux::{annex_b_to_avcc, split_access_units, top_level_boxes, AvcConfig, Mp4Writer, VideoFormat};

const WIDTH: u16 = 160;
const HEIGHT: u16 = 120;
const FPS: u32 = 25;
const FRAMES: u32 = 20;
/// One tick per frame keeps the arithmetic obvious; the timescale is the rate.
const TIMESCALE: u32 = 25;

/// Our own writer is the thing under test, so the oracle has to be somebody
/// else's parser. ffprobe ships beside the app already.
fn ffprobe() -> Option<PathBuf> {
    let ffmpeg = recast_testkit::ffmpeg_path()?;
    // The bundled sidecars carry the target triple, so ffprobe sits beside
    // ffmpeg under the SAME decorated name rather than a bare one.
    let name = ffmpeg.file_name()?.to_str()?.replacen("ffmpeg", "ffprobe", 1);
    let probe = ffmpeg.with_file_name(name);
    probe.exists().then_some(probe)
}

/// A real H.264 elementary stream, so the muxer is fed what an encoder actually
/// emits rather than bytes shaped to suit it.
fn elementary_stream(ffmpeg: &Path, out: &Path) -> bool {
    Command::new(ffmpeg)
        .args([
            "-y",
            "-f",
            "lavfi",
            "-i",
            &format!("testsrc=size={WIDTH}x{HEIGHT}:rate={FPS}"),
            "-frames:v",
            &FRAMES.to_string(),
            "-c:v",
            "libx264",
            "-preset",
            "ultrafast",
            // No B-frames: composition offsets are a separate concern and this
            // test is about the container.
            "-bf",
            "0",
            "-f",
            "h264",
        ])
        .arg(out)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn probe_field(ffprobe: &Path, file: &Path, entry: &str) -> String {
    let out = Command::new(ffprobe)
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            entry,
            "-of",
            "default=noprint_wrappers=1:nokey=1",
        ])
        .arg(file)
        .stdin(Stdio::null())
        .output()
        .expect("ffprobe runs");
    // ffprobe ends lines with CRLF on Windows, and a multi-field query is
    // compared line by line.
    String::from_utf8_lossy(&out.stdout)
        .replace("\r\n", "\n")
        .trim()
        .to_string()
}

struct Muxed {
    path: PathBuf,
    ffprobe: PathBuf,
    samples: usize,
}

/// Builds the MP4 once for the assertions below. `None` when the sidecars are
/// not present, which skips rather than fails, like the GPU tests.
fn muxed() -> Option<Muxed> {
    let Some(ffmpeg) = recast_testkit::ffmpeg_path() else {
        eprintln!("skipping: no ffmpeg sidecar");
        return None;
    };
    let Some(ffprobe) = ffprobe() else {
        eprintln!("skipping: no ffprobe beside {}", ffmpeg.display());
        return None;
    };
    let dir = std::env::temp_dir().join("recast-mux-tests");
    std::fs::create_dir_all(&dir).ok()?;
    let raw = dir.join("source.h264");
    if !elementary_stream(&ffmpeg, &raw) {
        eprintln!("skipping: ffmpeg could not produce an elementary stream");
        return None;
    }
    let stream = std::fs::read(&raw).ok()?;

    let mut writer = Mp4Writer::new(VideoFormat {
        width: WIDTH,
        height: HEIGHT,
        timescale: TIMESCALE,
    });
    let mut config = AvcConfig::default();
    let mut samples = 0;
    for unit in split_access_units(&stream) {
        let converted = annex_b_to_avcc(&unit);
        if !converted.config.is_empty() {
            config = converted.config;
        }
        if converted.sample.is_empty() {
            continue;
        }
        writer.push_sample(&converted.sample, 1, converted.is_sync);
        samples += 1;
    }
    writer.set_avc_config(config);

    let data = writer.finish().expect("a muxed file");
    let path = dir.join("muxed.mp4");
    std::fs::write(&path, &data).ok()?;
    Some(Muxed {
        path,
        ffprobe,
        samples,
    })
}

#[test]
fn ffprobe_reads_our_file_as_h264_at_the_right_size() {
    let Some(m) = muxed() else { return };
    assert_eq!(probe_field(&m.ffprobe, &m.path, "stream=codec_name"), "h264");
    assert_eq!(
        probe_field(&m.ffprobe, &m.path, "stream=width,height"),
        format!("{WIDTH}\n{HEIGHT}")
    );
}

/// Decoding every frame is the real proof: the sample table has to point at the
/// right bytes, and `avcC` has to describe them.
#[test]
fn every_sample_decodes_back_out() {
    let Some(m) = muxed() else { return };
    let counted = probe_field(&m.ffprobe, &m.path, "stream=nb_read_frames");
    let decoded: usize = Command::new(&m.ffprobe)
        .args([
            "-v",
            "error",
            "-count_frames",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=nb_read_frames",
            "-of",
            "default=noprint_wrappers=1:nokey=1",
        ])
        .arg(&m.path)
        .stdin(Stdio::null())
        .output()
        .ok()
        .and_then(|o| String::from_utf8_lossy(&o.stdout).trim().parse().ok())
        .unwrap_or(0);
    assert!(
        decoded > 0,
        "ffprobe decoded nothing (nb_read_frames was {counted:?})"
    );
    assert_eq!(decoded, m.samples, "not every muxed sample came back");
}

#[test]
fn the_duration_matches_the_frames_we_wrote() {
    let Some(m) = muxed() else { return };
    let duration: f64 = probe_field(&m.ffprobe, &m.path, "stream=duration")
        .parse()
        .unwrap_or(0.0);
    let expected = m.samples as f64 / FPS as f64;
    assert!(
        (duration - expected).abs() < 0.1,
        "duration {duration} against an expected {expected}"
    );
}

/// The whole reason for buffering: a player must not have to seek to the end.
#[test]
fn the_file_is_progressive_without_a_faststart_pass() {
    let Some(m) = muxed() else { return };
    let data = std::fs::read(&m.path).expect("the muxed file");
    let kinds: Vec<[u8; 4]> = top_level_boxes(&data).into_iter().map(|(k, _)| k).collect();
    assert_eq!(kinds, vec![*b"ftyp", *b"moov", *b"mdat"]);
}
