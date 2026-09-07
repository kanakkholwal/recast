//! The spilled payload store against the in-memory one it replaces for a large
//! export. Byte-identical is the bar: the file must not depend on where its
//! samples waited.

use recast_mux::{AudioFormat, Mp4Writer, VideoFormat};

const W: u16 = 320;
const H: u16 = 180;
const FPS: u32 = 30;

/// SPS/PPS the writer needs before it will emit anything, plus a ramp of
/// samples big enough that ordering mistakes show up as a byte difference.
fn build(spill: Option<&std::path::Path>) -> Option<Vec<u8>> {
    let mut writer = Mp4Writer::new(VideoFormat {
        width: W,
        height: H,
        timescale: FPS,
    });
    if let Some(dir) = spill {
        writer.spill_to(dir).expect("the spill files open");
    }
    writer.set_avc_config(recast_mux::AvcConfig {
        sps: vec![vec![0x67, 0x42, 0x00, 0x0a, 0xf8, 0x41, 0xa2]],
        pps: vec![vec![0x68, 0xce, 0x38, 0x80]],
    });
    writer.set_audio_format(AudioFormat {
        sample_rate: 48_000,
        channels: 2,
        config: vec![0x11, 0x90],
    });
    for index in 0..60u32 {
        let video = vec![(index % 251) as u8; 700 + index as usize];
        writer.push_sample(&video, 1, index % 30 == 0);
        let audio = vec![(index % 97) as u8; 190 + index as usize];
        writer.push_audio_sample(&audio, 1024);
    }
    writer.finish()
}

/// Where the bytes waited must not change the file, or the spill is a second
/// muxer rather than the same one with its buffers moved.
#[test]
fn a_spilled_mux_writes_the_same_file_as_an_in_memory_one() {
    let dir = std::env::temp_dir().join(format!("recast-mux-spill-{}", std::process::id()));
    let in_memory = build(None).expect("the in-memory mux writes a file");
    let spilled = build(Some(&dir)).expect("the spilled mux writes a file");
    assert_eq!(
        in_memory.len(),
        spilled.len(),
        "the two files are different sizes"
    );
    assert!(
        in_memory == spilled,
        "the spilled mux laid its samples out differently"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// A cancelled export must not leave gigabytes of samples in the temp directory.
#[test]
fn the_spill_files_are_gone_once_the_writer_is() {
    let dir = std::env::temp_dir().join(format!("recast-mux-spill-drop-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    {
        let mut writer = Mp4Writer::new(VideoFormat {
            width: W,
            height: H,
            timescale: FPS,
        });
        writer.spill_to(&dir).expect("the spill files open");
        writer.push_sample(&[0u8; 128], 1, true);
        let left = std::fs::read_dir(&dir).expect("the dir exists").count();
        assert_eq!(left, 2, "one spill file per track while the writer lives");
    }
    let left = std::fs::read_dir(&dir).expect("the dir exists").count();
    assert_eq!(left, 0, "the spill files outlived the writer");
    let _ = std::fs::remove_dir_all(&dir);
}
