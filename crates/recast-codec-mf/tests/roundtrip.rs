#![cfg(windows)]

use std::path::PathBuf;

use recast_codec::{ranked, VideoCodec};
use recast_codec_mf::{enumerate_encoders, EncodeConfig, H264Encoder, VideoReader};
use recast_mux::{annex_b_to_avcc, split_access_units, AvcConfig, Mp4Writer, VideoFormat};

const WIDTH: u32 = 320;
const HEIGHT: u32 = 240;
const FRAMES: u32 = 30;
const FPS: u32 = 30;
const BAR_WIDTH: usize = 40;
/// 100 ns units, the Media Foundation clock.
const FRAME_DURATION: i64 = 10_000_000 / FPS as i64;

/// Where the bright bar starts on frame `index`. The decoder has to hand back a
/// picture whose bar is here, which no amount of correct plumbing can fake.
fn bar_start(index: u32) -> usize {
    (index as usize * 7) % (WIDTH as usize - BAR_WIDTH)
}

fn nv12_frame(index: u32) -> Vec<u8> {
    let (w, h) = (WIDTH as usize, HEIGHT as usize);
    let mut data = vec![16u8; w * h * 3 / 2];
    let bar = bar_start(index);
    for row in 0..h {
        data[row * w + bar..row * w + bar + BAR_WIDTH].fill(235);
    }
    data[w * h..].fill(128);
    data
}

/// Brightest column band of a decoded NV12 luma plane, as the midpoint of the
/// run of bright pixels on the middle row.
fn bar_centre(luma: &[u8], width: usize, height: usize) -> Option<usize> {
    let row = &luma[(height / 2) * width..(height / 2 + 1) * width];
    let first = row.iter().position(|&v| v > 160)?;
    let last = row.iter().rposition(|&v| v > 160)?;
    Some((first + last) / 2)
}

fn encoded_file() -> Option<PathBuf> {
    let found = enumerate_encoders();
    let mut encoder = None;
    for descriptor in ranked(&found, VideoCodec::H264) {
        let config = EncodeConfig {
            width: WIDTH,
            height: HEIGHT,
            frame_rate: (FPS, 1),
            // Generous, so the bar stays crisp enough to locate.
            bitrate: 8_000_000,
        };
        if let Ok(open) = H264Encoder::open(descriptor, config) {
            eprintln!("encoding with {}", descriptor.name);
            encoder = Some(open);
            break;
        }
    }
    let mut encoder = match encoder {
        Some(encoder) => encoder,
        None => {
            eprintln!("skipping: no H.264 encoder opened");
            return None;
        }
    };

    let mut writer = Mp4Writer::new(VideoFormat {
        width: WIDTH as u16,
        height: HEIGHT as u16,
        timescale: FPS,
    });
    let mut config = AvcConfig::default();
    let push = |writer: &mut Mp4Writer, config: &mut AvcConfig, data: &[u8]| {
        for unit in split_access_units(data) {
            let converted = annex_b_to_avcc(&unit);
            if !converted.config.is_empty() {
                *config = converted.config;
            }
            if !converted.sample.is_empty() {
                writer.push_sample(&converted.sample, 1, converted.is_sync);
            }
        }
    };

    for index in 0..FRAMES {
        for sample in encoder
            .encode(&nv12_frame(index), index as i64 * FRAME_DURATION, FRAME_DURATION)
            .expect("the frame encodes")
        {
            push(&mut writer, &mut config, &sample.data);
        }
    }
    for sample in encoder.finish().expect("the tail drains") {
        push(&mut writer, &mut config, &sample.data);
    }
    writer.set_avc_config(config);

    let data = writer.finish().expect("a muxed file");
    let dir = std::env::temp_dir().join("recast-codec-mf-tests");
    std::fs::create_dir_all(&dir).ok()?;
    let path = dir.join("roundtrip.mp4");
    std::fs::write(&path, &data).ok()?;
    Some(path)
}

#[test]
fn our_own_file_reports_the_size_we_encoded() {
    let Some(path) = encoded_file() else { return };
    let reader = VideoReader::open(&path).expect("the file opens");
    let info = reader.info();
    assert_eq!((info.width, info.height), (WIDTH, HEIGHT));
    assert!(info.duration > 0, "no duration in the file we wrote");
}

/// The whole loop with no FFmpeg in it: our encoder, our muxer, our reader.
#[test]
fn every_encoded_frame_decodes_back_out() {
    let Some(path) = encoded_file() else { return };
    let mut reader = VideoReader::open(&path).expect("the file opens");
    let mut count = 0;
    while reader.next_frame().expect("a read").is_some() {
        count += 1;
    }
    assert_eq!(count, FRAMES as usize, "frames went missing in the round trip");
}

/// Plumbing that returns the right NUMBER of frames can still return the wrong
/// pictures, so the moving bar has to come back where it went in.
#[test]
fn the_picture_survives_the_round_trip() {
    let Some(path) = encoded_file() else { return };
    let mut reader = VideoReader::open(&path).expect("the file opens");
    let (w, h) = (WIDTH as usize, HEIGHT as usize);

    let mut checked = 0;
    let mut index = 0;
    while let Some(frame) = reader.next_frame().expect("a read") {
        assert!(frame.data.len() >= w * h, "the luma plane is short");
        let Some(centre) = bar_centre(&frame.data, w, h) else {
            panic!("frame {index} came back with no bright bar at all");
        };
        let expected = bar_start(index) + BAR_WIDTH / 2;
        assert!(
            centre.abs_diff(expected) <= 6,
            "frame {index}: bar at {centre}, expected about {expected}"
        );
        checked += 1;
        index += 1;
    }
    assert_eq!(checked, FRAMES, "not every frame was checked");
}

/// Seeking back has to give the picture from that time, not carry on from where
/// the reader had got to.
#[test]
fn a_seek_returns_to_the_picture_at_that_time() {
    let Some(path) = encoded_file() else { return };
    let mut reader = VideoReader::open(&path).expect("the file opens");
    let (w, h) = (WIDTH as usize, HEIGHT as usize);

    // Run to the end so a stale position would be obvious.
    while reader.next_frame().expect("a read").is_some() {}

    reader.seek(0).expect("a seek");
    let first = reader.next_frame().expect("a read").expect("a frame");
    let centre = bar_centre(&first.data, w, h).expect("a bar");
    let expected = bar_start(0) + BAR_WIDTH / 2;
    assert!(
        centre.abs_diff(expected) <= 6,
        "after seeking to zero the bar was at {centre}, expected about {expected}"
    );
}
