//! Audio capture against the real devices on this machine.
//!
//! Skipped where there is no device to open, which is a headless CI runner
//! rather than a regression.

use std::time::Duration;

use capturekit::{
    audio_devices, audio_input, audio_loopback, AudioDirection, AudioFormat, SampleFormat,
};

fn devices_or_skip() -> Option<Vec<capturekit::AudioDevice>> {
    match audio_devices() {
        Ok(devices) if !devices.is_empty() => Some(devices),
        Ok(_) => {
            eprintln!("skipped: no audio endpoints on this machine");
            None
        }
        Err(err) => {
            eprintln!("skipped: audio devices could not be enumerated: {err}");
            None
        }
    }
}

#[test]
fn every_device_reports_a_format_something_could_actually_read() {
    let Some(devices) = devices_or_skip() else {
        return;
    };
    for device in &devices {
        let format = device.format;
        assert!(
            format.sample_rate >= 8_000 && format.sample_rate <= 768_000,
            "{} reports {}Hz",
            device.name,
            format.sample_rate
        );
        assert!(format.channels > 0, "{} reports no channels", device.name);
        assert!(format.bytes_per_frame() > 0);
    }
}

#[test]
fn device_ids_are_unique_and_each_direction_has_at_most_one_default() {
    let Some(devices) = devices_or_skip() else {
        return;
    };
    let mut ids: Vec<&str> = devices.iter().map(|d| d.id.0.as_str()).collect();
    let before = ids.len();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), before, "two endpoints share an id");

    for direction in [AudioDirection::Input, AudioDirection::Loopback] {
        let defaults = devices
            .iter()
            .filter(|d| d.direction == direction && d.is_default)
            .count();
        assert!(defaults <= 1, "{direction:?} has {defaults} defaults");
    }
}

/// Loopback is the one that matters most here: it delivers nothing at all while
/// nothing is playing, which is exactly the case a naive capture gets wrong.
///
/// The property under test is that the TIMELINE is continuous, not merely that
/// bytes arrive: every buffer must start where the previous one ended, so the
/// samples can be concatenated without drift.
#[test]
fn loopback_delivers_a_continuous_timeline_even_when_nothing_is_playing() {
    let Some(_) = devices_or_skip() else {
        return;
    };
    let mut capture = match audio_loopback().build() {
        Ok(capture) => capture,
        Err(err) => {
            eprintln!("skipped: no loopback endpoint to open: {err}");
            return;
        }
    };

    let format = capture.describe().format;
    let mut frames = 0u64;
    let mut inserted_silence = 0u64;
    let mut first_pts = None;
    let mut expected_next: Option<capturekit::Timestamp> = None;
    let mut worst_wait = Duration::ZERO;

    let started = std::time::Instant::now();
    while started.elapsed() < Duration::from_millis(1_200) {
        let asked = std::time::Instant::now();
        match capture.next_buffer(Duration::from_millis(200)) {
            Ok(buffer) => {
                worst_wait = worst_wait.max(asked.elapsed());
                assert_eq!(buffer.format(), format, "the format changed mid-stream");
                format
                    .validate_buffer(buffer.bytes().len())
                    .expect("a buffer that is not whole sample frames");

                // Every buffer must begin exactly where the last one ended. A
                // timeline that does not advance over inserted silence fails
                // here, and so does one that double-counts it.
                if let Some(expected) = expected_next {
                    assert_eq!(
                        buffer.pts(),
                        expected,
                        "a {} buffer left a hole in the timeline",
                        if buffer.is_inserted_silence() {
                            "silence"
                        } else {
                            "sample"
                        }
                    );
                }
                expected_next = Some(buffer.pts().saturating_add(buffer.duration()));
                first_pts.get_or_insert(buffer.pts());

                if buffer.is_inserted_silence() {
                    inserted_silence += buffer.frames() as u64;
                }
                frames += buffer.frames() as u64;
            }
            Err(err) => assert!(err.is_recoverable(), "loopback failed: {err}"),
        }
    }
    capture.stop().expect("release the endpoint");

    // A silent desktop still owes a second of samples. Without gap filling this
    // comes back near zero and every later sound lands early.
    let covered = format.duration_of(frames);
    assert!(
        covered >= Duration::from_millis(700),
        "1.2s of loopback produced only {covered:?} of samples ({inserted_silence} inserted)"
    );

    // The timestamps must span the same length the samples do, or the two
    // disagree about how long the recording is.
    let (Some(first), Some(end)) = (first_pts, expected_next) else {
        panic!("no buffers arrived at all");
    };
    let spanned = end.saturating_since(first);
    assert_eq!(
        spanned, covered,
        "the timeline and the sample count disagree"
    );

    // Silence must arrive promptly, not only when a read times out: an encoder
    // fed one buffer per timeout stalls its own pipeline.
    assert!(
        worst_wait < Duration::from_millis(180),
        "a buffer took {worst_wait:?}, so silence is only produced on timeout"
    );
}

/// An input endpoint delivers real packets whether or not anyone is speaking,
/// so this is where the real-buffer half of the timeline gets exercised. The
/// loopback test cannot reach it on a silent desktop, where no packet arrives.
#[test]
fn an_input_device_delivers_samples_on_a_continuous_timeline() {
    let Some(devices) = devices_or_skip() else {
        return;
    };
    if !devices.iter().any(|d| d.direction == AudioDirection::Input) {
        eprintln!("skipped: no input device on this machine");
        return;
    }
    let mut capture = match audio_input().build() {
        Ok(capture) => capture,
        Err(err) => {
            eprintln!("skipped: the default input would not open: {err}");
            return;
        }
    };

    let mut real_frames = 0u64;
    let mut expected_next: Option<capturekit::Timestamp> = None;
    let started = std::time::Instant::now();
    while started.elapsed() < Duration::from_millis(900) {
        let Ok(buffer) = capture.next_buffer(Duration::from_millis(200)) else {
            continue;
        };
        if let Some(expected) = expected_next {
            assert_eq!(
                buffer.pts(),
                expected,
                "a {} buffer left a hole in the input timeline",
                if buffer.is_inserted_silence() {
                    "silence"
                } else {
                    "sample"
                }
            );
        }
        expected_next = Some(buffer.pts().saturating_add(buffer.duration()));
        if !buffer.is_inserted_silence() {
            real_frames += buffer.frames() as u64;
        }
    }
    capture.stop().expect("release the endpoint");

    assert!(
        expected_next.is_some(),
        "the input device produced nothing at all"
    );
    assert!(
        real_frames > 0,
        "every buffer was inserted silence, so the device path never ran"
    );
}

#[test]
fn opening_a_device_that_does_not_exist_is_refused() {
    let Some(_) = devices_or_skip() else {
        return;
    };
    let built = audio_input()
        .device(capturekit::AudioDeviceId("not-a-real-endpoint".into()))
        .build();
    assert!(built.is_err(), "a made-up endpoint id opened a device");
}

#[test]
fn the_stereo_48k_constant_matches_what_it_claims() {
    assert_eq!(AudioFormat::STEREO_48K.sample_rate, 48_000);
    assert_eq!(AudioFormat::STEREO_48K.channels, 2);
    assert_eq!(AudioFormat::STEREO_48K.sample_format, SampleFormat::F32);
}
