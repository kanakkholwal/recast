#![cfg(windows)]

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use recast_codec::{ranked, VideoCodec};
use recast_codec_mf::{enumerate_encoders, EncodeConfig, H264Encoder};
use recast_mux::{annex_b_to_avcc, split_access_units, AvcConfig, Mp4Writer, VideoFormat};

const WIDTH: u32 = 320;
const HEIGHT: u32 = 240;
const FRAMES: u32 = 30;
const FPS: u32 = 30;

fn config() -> EncodeConfig {
    EncodeConfig {
        width: WIDTH,
        height: HEIGHT,
        frame_rate: (FPS, 1),
        bitrate: 2_000_000,
        keyframe_interval: 0,
    }
}

/// The first encoder that opens. Hardware comes first, so on a machine with a
/// GPU this exercises the asynchronous event path and on one without it falls
/// to the Microsoft software transform.
fn open_any() -> Option<H264Encoder> {
    let found = enumerate_encoders();
    for descriptor in ranked(&found, VideoCodec::H264) {
        match H264Encoder::open(descriptor, config()) {
            Ok(encoder) => {
                eprintln!("encoding with {}", descriptor.name);
                return Some(encoder);
            }
            Err(err) => {
                eprintln!("{} refused to open: {err}", descriptor.name);
                continue;
            }
        }
    }
    eprintln!("skipping: no synchronous H.264 encoder opened");
    None
}

/// A moving bar, so consecutive frames genuinely differ and the encoder cannot
/// collapse the whole clip into one tiny keyframe.
fn nv12_frame(index: u32) -> Vec<u8> {
    let (w, h) = (WIDTH as usize, HEIGHT as usize);
    let mut data = vec![16u8; w * h * 3 / 2];
    let bar = (index as usize * 7) % w;
    for row in 0..h {
        for column in bar..(bar + 40).min(w) {
            data[row * w + column] = 235;
        }
    }
    // Chroma stays neutral grey, which is 128 for both components.
    data[w * h..].fill(128);
    data
}

fn ffprobe() -> Option<PathBuf> {
    let ffmpeg = recast_testkit::ffmpeg_path()?;
    let name = ffmpeg
        .file_name()?
        .to_str()?
        .replacen("ffmpeg", "ffprobe", 1);
    let probe = ffmpeg.with_file_name(name);
    probe.exists().then_some(probe)
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
    String::from_utf8_lossy(&out.stdout)
        .replace("\r\n", "\n")
        .trim()
        .to_string()
}

/// Encodes the clip and returns the Annex B stream the transform produced.
fn encode_clip() -> Option<Vec<u8>> {
    let mut encoder = open_any()?;
    // Media Foundation counts in 100 ns units.
    let duration = 10_000_000i64 / FPS as i64;
    let mut stream = Vec::new();
    for index in 0..FRAMES {
        let frame = nv12_frame(index);
        let produced = encoder
            .encode(&frame, index as i64 * duration, duration)
            .expect("the frame encodes");
        for sample in produced {
            stream.extend_from_slice(&sample.data);
        }
    }
    for sample in encoder.finish().expect("the tail drains") {
        stream.extend_from_slice(&sample.data);
    }
    Some(stream)
}

/// Output has to come back while frames are still going in. If it only
/// appeared at `finish`, the transform would hold a whole recording internally
/// and nothing could be written to disk as it went.
#[test]
fn samples_come_back_during_the_encode_not_only_at_the_end() {
    let Some(mut encoder) = open_any() else {
        return;
    };
    let duration = 10_000_000i64 / FPS as i64;
    let mut during = 0;
    for index in 0..FRAMES {
        during += encoder
            .encode(&nv12_frame(index), index as i64 * duration, duration)
            .expect("the frame encodes")
            .len();
    }
    let tail = encoder.finish().expect("the tail drains").len();
    assert!(
        during > 0,
        "every one of the {tail} samples waited for the drain"
    );
}

#[test]
fn the_encoder_produces_a_stream_with_parameter_sets_and_a_keyframe() {
    let Some(stream) = encode_clip() else { return };
    assert!(!stream.is_empty(), "the encoder produced nothing");

    let mut config = AvcConfig::default();
    let mut keyframes = 0;
    let mut pictures = 0;
    for unit in split_access_units(&stream) {
        let converted = annex_b_to_avcc(&unit);
        if !converted.config.is_empty() {
            config = converted.config;
        }
        if converted.sample.is_empty() {
            continue;
        }
        pictures += 1;
        if converted.is_sync {
            keyframes += 1;
        }
    }
    assert!(!config.is_empty(), "no SPS/PPS in the stream");
    assert!(config.record().is_some(), "the parameter sets are unusable");
    assert!(keyframes > 0, "no keyframe in {pictures} pictures");
    assert_eq!(pictures, FRAMES as usize, "a frame went missing");
}

/// The point of the pair: what Media Foundation encodes, our muxer has to be
/// able to write, and somebody else's parser has to be able to read.
#[test]
fn the_encoded_stream_muxes_into_a_file_ffprobe_can_decode() {
    let Some(stream) = encode_clip() else { return };
    let Some(ffprobe) = ffprobe() else {
        eprintln!("skipping: no ffprobe sidecar");
        return;
    };

    let mut writer = Mp4Writer::new(VideoFormat {
        width: WIDTH as u16,
        height: HEIGHT as u16,
        timescale: FPS,
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
    let dir = std::env::temp_dir().join("recast-codec-mf-tests");
    std::fs::create_dir_all(&dir).expect("a temp dir");
    let path = dir.join("encoded.mp4");
    std::fs::write(&path, &data).expect("the file writes");

    assert_eq!(probe_field(&ffprobe, &path, "stream=codec_name"), "h264");
    assert_eq!(
        probe_field(&ffprobe, &path, "stream=width,height"),
        format!("{WIDTH}\n{HEIGHT}")
    );

    let decoded: usize = Command::new(&ffprobe)
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
        .arg(&path)
        .stdin(Stdio::null())
        .output()
        .ok()
        .and_then(|o| String::from_utf8_lossy(&o.stdout).trim().parse().ok())
        .unwrap_or(0);
    assert_eq!(decoded, samples, "not every encoded frame decoded back");
}

/// Keyframes in the stream `descriptor` produced at `interval`, over `count`
/// frames.
///
/// Counted as IDR access units in the bitstream. The transform's per-sample
/// `is_sync` flag cannot answer this: NVIDIA's MFT reports exactly one.
fn keyframes_at(
    descriptor: &recast_codec::EncoderDescriptor,
    interval: u32,
    count: u32,
) -> Option<usize> {
    let mut encoder = H264Encoder::open(
        descriptor,
        EncodeConfig {
            keyframe_interval: interval,
            ..config()
        },
    )
    .ok()?;
    let mut stream = Vec::new();
    for index in 0..count {
        let stamp = i64::from(index) * 10_000_000 / i64::from(FPS);
        for sample in encoder.encode(&nv12_frame(index), stamp, 0).ok()? {
            stream.extend_from_slice(&sample.data);
        }
    }
    for sample in encoder.finish().ok()? {
        stream.extend_from_slice(&sample.data);
    }
    Some(
        split_access_units(&stream)
            .iter()
            .filter(|unit| annex_b_to_avcc(unit).is_sync)
            .count(),
    )
}

/// Footage that will be scrubbed needs keyframes close together: a seek decodes
/// from the one before it, and NVIDIA's default is an infinite GOP — one
/// keyframe for a whole recording.
///
/// Every encoder this machine offers, not just the preferred one: the property
/// has to hold across vendors or the recorder cannot rely on it.
#[test]
fn a_short_keyframe_interval_produces_more_keyframes_than_the_default() {
    const CLIP: u32 = 90;
    const EVERY: u32 = 15;
    let found = enumerate_encoders();
    let mut checked = 0;
    for descriptor in ranked(&found, VideoCodec::H264) {
        let Some(dense) = keyframes_at(descriptor, EVERY, CLIP) else {
            eprintln!("{} did not open", descriptor.name);
            continue;
        };
        let sparse = keyframes_at(descriptor, 0, CLIP).expect("it opened once already");
        eprintln!(
            "{}: every-{EVERY} = {dense} keyframes, default = {sparse}",
            descriptor.name
        );
        assert!(
            dense >= (CLIP / EVERY) as usize - 1,
            "{} produced {dense} keyframes for a {EVERY}-frame interval over {CLIP}",
            descriptor.name
        );
        assert!(
            dense > sparse,
            "{} ignored the interval: {dense} against its default {sparse}",
            descriptor.name
        );
        checked += 1;
    }
    assert!(
        checked > 0,
        "no H.264 encoder opened, so nothing was checked"
    );
}

/// Keyframes when they are ASKED FOR per frame rather than configured as a GOP.
///
/// This is what a variable-rate writer needs: it knows the timestamps, so it
/// decides when a keyframe is due. A frame-count GOP cannot express that.
#[test]
fn a_keyframe_can_be_demanded_for_a_chosen_frame() {
    const CLIP: u32 = 60;
    const EVERY: u32 = 10;
    let found = enumerate_encoders();
    let mut checked = 0;
    for descriptor in ranked(&found, VideoCodec::H264) {
        // Default GOP: anything counted came from the request, not config.
        let Ok(mut encoder) = H264Encoder::open(descriptor, config()) else {
            continue;
        };
        let mut stream = Vec::new();
        for index in 0..CLIP {
            if index % EVERY == 0 {
                encoder.request_keyframe();
            }
            let stamp = i64::from(index) * 10_000_000 / i64::from(FPS);
            for sample in encoder
                .encode(&nv12_frame(index), stamp, 0)
                .expect("it encodes")
            {
                stream.extend_from_slice(&sample.data);
            }
        }
        for sample in encoder.finish().expect("it flushes") {
            stream.extend_from_slice(&sample.data);
        }
        let keyframes = split_access_units(&stream)
            .iter()
            .filter(|unit| annex_b_to_avcc(unit).is_sync)
            .count();
        eprintln!(
            "{}: {keyframes} keyframes for {} requests",
            descriptor.name,
            CLIP / EVERY
        );
        assert!(
            keyframes >= (CLIP / EVERY) as usize - 1,
            "{} produced {keyframes} keyframes for {} requests",
            descriptor.name,
            CLIP / EVERY
        );
        checked += 1;
    }
    assert!(
        checked > 0,
        "no H.264 encoder opened, so nothing was checked"
    );
}
