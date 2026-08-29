//! Audio capture against the real devices on this machine.
//!
//! Skipped where there is no device to open, which is a headless CI runner
//! rather than a regression.

use std::time::Duration;

use capturekit::{
    audio_devices, audio_input, audio_loopback, AudioDirection, AudioFormat, SampleFormat,
};

/// One audio device, many tests. Opening the same endpoint from several tests
/// at once is contention the harness creates and a recorder never would, and it
/// makes an otherwise green suite fail every few runs.
fn exclusive() -> std::sync::MutexGuard<'static, ()> {
    static DEVICE: std::sync::Mutex<()> = std::sync::Mutex::new(());
    // A test that fails while holding the lock must not fail every later one.
    DEVICE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

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
    let _device = exclusive();
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
    let mut first_pts: Option<capturekit::Timestamp> = None;
    let mut run_start: Option<capturekit::Timestamp> = None;
    let mut run_frames = 0u64;
    let mut breaks = 0u32;
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

                // Every buffer must begin exactly where the samples before it
                // ended, measured from the START OF ITS RUN rather than from the
                // previous buffer. Summing per-buffer durations rounds on every
                // step and drifts, which is the very thing a sample-counted
                // timeline exists to avoid, so it cannot also be the yardstick.
                // This also catches accumulated drift the pairwise check never
                // could.
                //
                // A run ends where the source SAYS it broke. Continuity is the
                // contract between declared breaks, not across them.
                if buffer.is_discontinuous() {
                    breaks += 1;
                    run_start = None;
                    run_frames = 0;
                } else if let Some(start) = run_start {
                    assert_eq!(
                        buffer.pts(),
                        start.saturating_add(format.duration_of(run_frames)),
                        "a {} buffer left an undeclared hole after {run_frames} frames",
                        if buffer.is_inserted_silence() {
                            "silence"
                        } else {
                            "sample"
                        }
                    );
                }
                run_start.get_or_insert(buffer.pts());
                run_frames += buffer.frames() as u64;
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

    assert!(first_pts.is_some(), "no buffers arrived at all");
    // A stream that kept up cannot deliver more audio than time has passed,
    // plus whatever was buffered when it opened. Catches a device delivering at
    // a rate other than the one it declared, which the continuity check above
    // cannot see because it is measured in that same wrong rate. Skipped where
    // the source declared a break, since then it is not the same stream.
    let real_time = started.elapsed();
    assert!(
        covered <= real_time + Duration::from_millis(300),
        "{covered:?} of samples from {real_time:?} of capture ({breaks} break(s),          {inserted_silence} inserted). More audio than time means the source is          inventing it, which no continuity check can see: they are all measured          in the same wrong rate."
    );
    // A break is a source saying it lost samples; an idle desktop loses none.
    assert!(
        breaks <= 2,
        "the loopback declared {breaks} break(s) in {real_time:?} of an idle desktop"
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
    let _device = exclusive();
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
    let _device = exclusive();
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

/// A recorder opens the devices once per take, so a second take must be as safe
/// as the first.
#[test]
fn a_loopback_can_be_opened_and_released_repeatedly_on_one_thread() {
    let _device = exclusive();
    for attempt in 0..3 {
        let Ok(mut capturer) = audio_loopback().build() else {
            eprintln!("skipped: no loopback endpoint on this machine");
            return;
        };
        let _ = capturer.next_buffer(Duration::from_millis(200));
        capturer.stop().expect("release the endpoint");
        eprintln!("attempt {attempt} released cleanly");
    }
}

/// The recorder opens each take on a fresh capture thread that then exits, which
/// is where an unbalanced COM apartment shows up.
#[test]
fn a_loopback_can_be_opened_on_a_fresh_thread_each_time() {
    let _device = exclusive();
    for attempt in 0..3 {
        let joined = std::thread::spawn(|| {
            let Ok(mut capturer) = audio_loopback().build() else {
                return false;
            };
            let _ = capturer.next_buffer(Duration::from_millis(200));
            capturer.stop().expect("release the endpoint");
            true
        })
        .join()
        .expect("the capture thread did not panic");
        if !joined {
            eprintln!("skipped: no loopback endpoint on this machine");
            return;
        }
        eprintln!("thread {attempt} released cleanly");
    }
}

/// A recording holds both directions at once, and does so again on the next
/// take. Two endpoints open concurrently is the case one alone does not cover.
#[test]
fn both_directions_can_be_held_at_once_over_repeated_takes() {
    let _device = exclusive();
    for attempt in 0..3 {
        let takes = [AudioDirection::Loopback, AudioDirection::Input].map(|direction| {
            std::thread::spawn(move || {
                let built = match direction {
                    AudioDirection::Loopback => audio_loopback().build(),
                    _ => audio_input().build(),
                };
                let Ok(mut capturer) = built else {
                    return false;
                };
                for _ in 0..5 {
                    let _ = capturer.next_buffer(Duration::from_millis(100));
                }
                capturer.stop().expect("release the endpoint");
                true
            })
        });
        let opened: Vec<bool> = takes
            .into_iter()
            .map(|t| t.join().expect("no capture thread panicked"))
            .collect();
        eprintln!("attempt {attempt}: opened {opened:?}");
    }
}

/// Enumeration from many threads that each open and close a COM apartment.
///
/// The apartment is process-global state: when the last reference goes away the
/// MTA is torn down and the activation factories windows-rs caches in process
/// globals dangle, so the next call FAULTS rather than failing. That is an
/// access violation, not a test failure, which is how it reached CI as
/// `STATUS_ACCESS_VIOLATION` with no name attached to it.
///
/// A CANARY, not a proof: on a machine that has audio endpoints the apartment
/// is kept alive by something else, so this passes with the fix and without it.
/// It only has teeth where the fault actually happens, which so far is CI.
#[test]
fn enumerating_from_many_short_lived_threads_does_not_tear_the_apartment_down() {
    let threads: Vec<_> = (0..8)
        .map(|_| {
            std::thread::spawn(|| {
                for _ in 0..20 {
                    // The result does not matter; surviving the call does.
                    let _ = audio_devices();
                }
            })
        })
        .collect();
    for thread in threads {
        thread
            .join()
            .expect("no thread faulted enumerating devices");
    }
}

/// A failed open must release its interfaces BEFORE closing the apartment.
///
/// `open` binds the enumerator and its `ComScope`; if those are two locals they
/// drop in reverse and `CoUninitialize` runs first, so the release that follows
/// lands in a torn-down apartment and FAULTS. A refused device is the shortest
/// path to that early return, and a device-less machine takes it for every open.
///
/// On its own thread so this is the only apartment reference in play. Also a
/// canary: reinstating the inverted drop order does NOT make it fail here, so
/// it guards the CI environment rather than proving the fix on this one.
#[test]
fn a_refused_open_releases_its_interfaces_before_closing_the_apartment() {
    std::thread::spawn(|| {
        for _ in 0..50 {
            // Refusal is the expected answer; surviving it is what is asserted.
            let _ = audio_input()
                .device(capturekit::AudioDeviceId("no-such-endpoint".into()))
                .build();
        }
    })
    .join()
    .expect("no thread faulted on a refused open");
}
