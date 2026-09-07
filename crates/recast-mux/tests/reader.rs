use recast_mux::{AudioFormat, Mp4Reader, Mp4Writer, TrackKind, VideoFormat};

const WIDTH: u16 = 320;
const HEIGHT: u16 = 240;
const TIMESCALE: u32 = 30;

/// A payload that is different for every sample, so a reader that returns the
/// right count with the wrong offsets is still caught.
fn payload(index: usize, length: usize) -> Vec<u8> {
    (0..length).map(|i| (index * 31 + i) as u8).collect()
}

fn written(video: usize, audio: usize) -> Vec<u8> {
    let mut writer = Mp4Writer::new(VideoFormat {
        width: WIDTH,
        height: HEIGHT,
        timescale: TIMESCALE,
    });
    writer.set_avc_config(recast_mux::AvcConfig {
        sps: vec![vec![0x67, 0x42, 0xc0, 0x1e]],
        pps: vec![vec![0x68, 0xce, 0x3c, 0x80]],
    });
    writer.set_audio_format(AudioFormat {
        sample_rate: 48_000,
        channels: 2,
        // A real AudioSpecificConfig: AAC-LC, 48 kHz, stereo.
        config: vec![0x11, 0x90],
    });
    for index in 0..video {
        writer.push_sample(&payload(index, 64 + index), 1, index % 10 == 0);
    }
    for index in 0..audio {
        writer.push_audio_sample(&payload(index + 500, 40), 1024);
    }
    writer.finish().expect("a muxed file")
}

#[test]
fn the_reader_finds_the_tracks_the_writer_wrote() {
    let data = written(30, 45);
    let reader = Mp4Reader::new(&data).expect("the file parses");
    assert_eq!(reader.tracks().len(), 2);

    let video = reader.video().expect("a video track");
    assert_eq!(&video.format, b"avc1");
    assert_eq!((video.width, video.height), (WIDTH, HEIGHT));
    assert_eq!(video.timescale, TIMESCALE);
    assert_eq!(video.samples.len(), 30);

    let audio = reader.audio().expect("an audio track");
    assert_eq!(&audio.format, b"mp4a");
    assert_eq!(audio.sample_rate, 48_000);
    assert_eq!(audio.channels, 2);
    assert_eq!(audio.samples.len(), 45);
    assert_eq!(audio.kind, TrackKind::Audio);
}

/// The strongest property here: every sample comes back byte for byte. A wrong
/// chunk offset, a wrong `stsc` run or a wrong size table all break this, and
/// none of them break the counts above.
#[test]
fn every_sample_reads_back_exactly_as_it_went_in() {
    let data = written(30, 45);
    let reader = Mp4Reader::new(&data).expect("the file parses");

    let video = reader.video().unwrap();
    for (index, sample) in video.samples.iter().enumerate() {
        let got = reader.sample_data(sample).expect("bytes in the file");
        assert_eq!(got, payload(index, 64 + index), "video sample {index}");
    }

    let audio = reader.audio().unwrap();
    for (index, sample) in audio.samples.iter().enumerate() {
        let got = reader.sample_data(sample).expect("bytes in the file");
        assert_eq!(got, payload(index + 500, 40), "audio sample {index}");
    }
}

#[test]
fn decode_times_run_forward_by_each_samples_own_duration() {
    let data = written(30, 45);
    let reader = Mp4Reader::new(&data).expect("the file parses");
    let video = reader.video().unwrap();
    for (index, sample) in video.samples.iter().enumerate() {
        assert_eq!(sample.decode_time, index as u64, "video sample {index}");
        assert_eq!(sample.duration, 1);
    }
    let audio = reader.audio().unwrap();
    for (index, sample) in audio.samples.iter().enumerate() {
        assert_eq!(sample.decode_time, index as u64 * 1024);
        assert_eq!(sample.duration, 1024);
    }
}

/// Sync samples are what a seek lands on. Getting these wrong makes a seek
/// deliver garbage, which no count or size check would notice.
#[test]
fn sync_samples_come_back_where_they_were_marked() {
    let data = written(30, 45);
    let reader = Mp4Reader::new(&data).expect("the file parses");
    let marked: Vec<usize> = reader
        .video()
        .unwrap()
        .samples
        .iter()
        .enumerate()
        .filter(|(_, s)| s.is_sync)
        .map(|(i, _)| i)
        .collect();
    assert_eq!(marked, vec![0, 10, 20]);

    // An audio track has no `stss`, which is how it says every sample is one.
    assert!(reader.audio().unwrap().samples.iter().all(|s| s.is_sync));
}

#[test]
fn the_audio_decoder_config_survives_the_esds_nesting() {
    let data = written(4, 4);
    let reader = Mp4Reader::new(&data).expect("the file parses");
    assert_eq!(reader.audio().unwrap().decoder_config, vec![0x11, 0x90]);
}

/// `avcC` carries the parameter sets, so a decoder that never sees it produces
/// nothing at all.
#[test]
fn the_video_decoder_config_is_the_avcc_payload() {
    let data = written(4, 0);
    let reader = Mp4Reader::new(&data).expect("the file parses");
    let config = &reader.video().unwrap().decoder_config;
    assert!(!config.is_empty());
    // configurationVersion, then the three profile bytes from the SPS.
    assert_eq!(&config[..4], &[1, 0x42, 0xc0, 0x1e]);
}

#[test]
fn a_video_only_file_has_one_track() {
    let data = written(10, 0);
    let reader = Mp4Reader::new(&data).expect("the file parses");
    assert_eq!(reader.tracks().len(), 1);
    assert!(reader.audio().is_none());
    assert_eq!(reader.video().unwrap().samples.len(), 10);
}

#[test]
fn a_track_reports_its_length_in_seconds() {
    let data = written(60, 0);
    let reader = Mp4Reader::new(&data).expect("the file parses");
    let seconds = reader.video().unwrap().seconds();
    assert!((seconds - 2.0).abs() < 1e-6, "reported {seconds}s");
}

#[test]
fn a_truncated_file_is_an_error_rather_than_a_panic() {
    let data = written(10, 10);
    for cut in [8, 64, data.len() / 3, data.len() - 1] {
        // It may parse what is there or call the file broken; what it must never do is index past the end.
        let _ = Mp4Reader::new(&data[..cut]);
    }
}
