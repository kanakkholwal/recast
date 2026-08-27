#![cfg(windows)]

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use recast_codec::{ranked, VideoCodec};
use recast_codec_mf::{
    enumerate_encoders, AacEncoder, AudioFormat, EncodeConfig, H264Encoder, VideoReader,
};
use recast_mux::{annex_b_to_avcc, split_access_units, AvcConfig, Mp4Writer, VideoFormat};

const WIDTH: u32 = 320;
const HEIGHT: u32 = 240;
const FPS: u32 = 30;
const SECONDS: u32 = 2;
const FRAMES: u32 = FPS * SECONDS;
const TONE_HZ: f64 = 440.0;
/// AAC-LC codes 1024 samples per frame.
const AAC_FRAME: usize = 1024;

fn nv12_frame(index: u32) -> Vec<u8> {
    let (w, h) = (WIDTH as usize, HEIGHT as usize);
    let mut data = vec![16u8; w * h * 3 / 2];
    let bar = (index as usize * 7) % (w - 40);
    for row in 0..h {
        data[row * w + bar..row * w + bar + 40].fill(235);
    }
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

fn probe(ffprobe: &Path, file: &Path, args: &[&str]) -> String {
    let out = Command::new(ffprobe)
        .args(["-v", "error"])
        .args(args)
        .args(["-of", "default=noprint_wrappers=1:nokey=1"])
        .arg(file)
        .stdin(Stdio::null())
        .output()
        .expect("ffprobe runs");
    String::from_utf8_lossy(&out.stdout)
        .replace("\r\n", "\n")
        .trim()
        .to_string()
}

/// Encodes video and audio through Media Foundation, muxes both with our own
/// writer, and returns the file. `None` skips when nothing will open.
fn muxed() -> Option<PathBuf> {
    let found = enumerate_encoders();
    let mut video = None;
    for descriptor in ranked(&found, VideoCodec::H264) {
        let config = EncodeConfig {
            width: WIDTH,
            height: HEIGHT,
            frame_rate: (FPS, 1),
            bitrate: 4_000_000,
        };
        if let Ok(open) = H264Encoder::open(descriptor, config) {
            eprintln!("video via {}", descriptor.name);
            video = Some(open);
            break;
        }
    }
    let mut video = video.or_else(|| {
        eprintln!("skipping: no H.264 encoder opened");
        None
    })?;

    let format = AudioFormat::default();
    let mut audio = match AacEncoder::open(format, 128_000) {
        Ok(encoder) => encoder,
        Err(err) => {
            eprintln!("skipping: no AAC encoder ({err})");
            return None;
        }
    };
    if audio.config().is_empty() {
        eprintln!("skipping: the AAC encoder reported no decoder config");
        return None;
    }

    let mut writer = Mp4Writer::new(VideoFormat {
        width: WIDTH as u16,
        height: HEIGHT as u16,
        timescale: FPS,
    });
    writer.set_audio_format(recast_mux::AudioFormat {
        sample_rate: format.sample_rate,
        channels: format.channels,
        config: audio.config().to_vec(),
    });

    let mut avc = AvcConfig::default();
    let frame_duration = 10_000_000i64 / FPS as i64;
    for index in 0..FRAMES {
        for sample in video
            .encode(
                &nv12_frame(index),
                index as i64 * frame_duration,
                frame_duration,
            )
            .expect("the frame encodes")
        {
            for unit in split_access_units(&sample.data) {
                let converted = annex_b_to_avcc(&unit);
                if !converted.config.is_empty() {
                    avc = converted.config;
                }
                if !converted.sample.is_empty() {
                    writer.push_sample(&converted.sample, 1, converted.is_sync);
                }
            }
        }
    }
    for sample in video.finish().expect("the video tail drains") {
        for unit in split_access_units(&sample.data) {
            let converted = annex_b_to_avcc(&unit);
            if !converted.config.is_empty() {
                avc = converted.config;
            }
            if !converted.sample.is_empty() {
                writer.push_sample(&converted.sample, 1, converted.is_sync);
            }
        }
    }
    writer.set_avc_config(avc);

    // A 440 Hz tone, fed in one AAC frame at a time.
    let channels = format.channels as usize;
    let total = format.sample_rate as usize * SECONDS as usize;
    let mut at = 0usize;
    while at < total {
        let count = AAC_FRAME.min(total - at);
        let mut block = Vec::with_capacity(count * channels);
        for i in 0..count {
            let t = (at + i) as f64 / format.sample_rate as f64;
            let value = (2.0 * std::f64::consts::PI * TONE_HZ * t).sin() as f32 * 0.5;
            for _ in 0..channels {
                block.push(value);
            }
        }
        let timestamp = at as i64 * 10_000_000 / format.sample_rate as i64;
        for sample in audio.encode(&block, timestamp).expect("the audio encodes") {
            writer.push_audio_sample(&sample.data, AAC_FRAME as u32);
        }
        at += count;
    }
    for sample in audio.finish().expect("the audio tail drains") {
        writer.push_audio_sample(&sample.data, AAC_FRAME as u32);
    }

    let data = writer.finish().expect("a muxed file");
    let dir = std::env::temp_dir().join("recast-codec-mf-tests");
    std::fs::create_dir_all(&dir).ok()?;
    let path = dir.join("av.mp4");
    std::fs::write(&path, &data).ok()?;
    Some(path)
}

#[test]
fn the_file_carries_both_streams() {
    let Some(path) = muxed() else { return };
    let Some(ffprobe) = ffprobe() else {
        eprintln!("skipping: no ffprobe sidecar");
        return;
    };
    let codecs = probe(&ffprobe, &path, &["-show_entries", "stream=codec_name"]);
    assert_eq!(codecs, "h264\naac", "streams were {codecs:?}");
}

/// Both tracks have to decode, not merely be described: a wrong `esds` or a
/// stale offset shows up here and nowhere else.
#[test]
fn both_streams_decode_back_out() {
    let Some(path) = muxed() else { return };
    let Some(ffprobe) = ffprobe() else { return };

    let video = probe(
        &ffprobe,
        &path,
        &[
            "-count_frames",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=nb_read_frames",
        ],
    );
    assert_eq!(
        video.parse::<u32>().unwrap_or(0),
        FRAMES,
        "video decoded {video} frames"
    );

    let audio = probe(
        &ffprobe,
        &path,
        &[
            "-count_frames",
            "-select_streams",
            "a:0",
            "-show_entries",
            "stream=nb_read_frames",
        ],
    );
    let expected = 48_000 * SECONDS as usize / AAC_FRAME;
    let decoded = audio.parse::<usize>().unwrap_or(0);
    assert!(
        decoded.abs_diff(expected) <= 4,
        "audio decoded {decoded} frames against about {expected}"
    );
}

/// ffmpeg will parse an AAC config out of the bitstream if the container has
/// none, so decoding proves nothing about `esds`. The extradata size is the
/// container's own answer: it is the `AudioSpecificConfig` we wrote, or zero.
#[test]
fn the_decoder_config_reaches_the_container_not_just_the_bitstream() {
    let Some(path) = muxed() else { return };
    let Some(ffprobe) = ffprobe() else { return };
    let size = probe(
        &ffprobe,
        &path,
        &[
            "-select_streams",
            "a:0",
            "-show_entries",
            "stream=extradata_size",
        ],
    );
    assert!(
        size.parse::<usize>().unwrap_or(0) >= 2,
        "esds carried {size:?} bytes of decoder config"
    );
    // And the profile read out of it has to be the one AAC-LC declares.
    let profile = probe(
        &ffprobe,
        &path,
        &["-select_streams", "a:0", "-show_entries", "stream=profile"],
    );
    assert_eq!(profile, "LC", "profile came back {profile:?}");
}

#[test]
fn the_audio_track_reports_the_rate_and_channels_it_was_given() {
    let Some(path) = muxed() else { return };
    let Some(ffprobe) = ffprobe() else { return };
    let fields = probe(
        &ffprobe,
        &path,
        &[
            "-select_streams",
            "a:0",
            "-show_entries",
            "stream=sample_rate,channels",
        ],
    );
    assert_eq!(fields, "48000\n2", "audio stream reported {fields:?}");
}

/// The video half still has to work with a second track sharing `mdat`.
#[test]
fn our_own_reader_still_walks_the_video_track() {
    let Some(path) = muxed() else { return };
    let mut reader = VideoReader::open(&path).expect("the file opens");
    let mut count = 0;
    while reader.next_frame().expect("a read").is_some() {
        count += 1;
    }
    assert_eq!(
        count, FRAMES as usize,
        "frames went missing beside the audio"
    );
}
