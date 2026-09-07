//! Live pointer reads against the real OS, `#[ignore]`d because a CI runner has no pointer.
//! These assert what the OS reports; the arithmetic is covered by `PointerCapturer`'s scripted unit tests.

use capturekit::{Capabilities, PointerCapturer, Rect, Timestamp};

/// A surface covering the whole virtual desktop, so the pointer is on it
/// wherever it happens to be sitting.
fn everywhere() -> Rect {
    Rect {
        x: -32_768,
        y: -32_768,
        width: 65_536,
        height: 65_536,
    }
}

fn caps() -> Capabilities {
    capturekit::capabilities()
}

#[test]
#[ignore = "live: needs a real pointer"]
fn the_pointer_reads_somewhere_on_the_virtual_desktop() {
    let mut cap = PointerCapturer::open(everywhere(), 1.0).expect("a pointer reader");
    let sample = cap.sample(Timestamp::ZERO).expect("a read").cursor;
    assert!(
        sample.position.is_some(),
        "a surface covering the whole desktop must contain the pointer"
    );
}

/// The reason this path exists rather than reading the cursor off a frame: a read must cost far less than a frame interval, or sampling above the frame rate is not actually possible.
#[test]
#[ignore = "live: needs a real pointer"]
fn a_read_costs_far_less_than_a_frame_interval() {
    const READS: u32 = 500;
    let mut cap = PointerCapturer::open(everywhere(), 1.0).expect("a pointer reader");
    let started = std::time::Instant::now();
    for _ in 0..READS {
        assert!(
            cap.sample(Timestamp::ZERO).is_some(),
            "the OS refused a read"
        );
    }
    let each = started.elapsed() / READS;
    // 125Hz sampling has an 8ms budget; anything near that cannot keep the rate.
    assert!(
        each < std::time::Duration::from_millis(2),
        "a pointer read took {each:?}, which cannot sustain 125Hz"
    );
}

/// Reads must be stable, not noisy: a still pointer that reported a different
/// place each call would show up as permanent jitter in a cursor track.
#[test]
#[ignore = "live: needs a real pointer, and for it to be still"]
fn a_still_pointer_reads_the_same_place_twice() {
    let mut cap = PointerCapturer::open(everywhere(), 1.0).expect("a pointer reader");
    let first = cap.sample(Timestamp::ZERO).expect("a read").cursor.position;
    let second = cap.sample(Timestamp::ZERO).expect("a read").cursor.position;
    assert_eq!(first, second);
}

#[test]
#[ignore = "live: needs a real pointer"]
fn buttons_read_as_released_when_nothing_is_held() {
    if !caps().cursor_buttons {
        return;
    }
    let mut cap = PointerCapturer::open(everywhere(), 1.0).expect("a pointer reader");
    let sample = cap.sample(Timestamp::ZERO).expect("a read").cursor;
    assert!(
        !sample.buttons.any(),
        "no button is held while this test runs, so none should read as down"
    );
}

/// A platform that says it cannot read buttons must not silently report a
/// plausible-looking `NONE` from a reader that opened anyway.
#[test]
#[ignore = "live: needs a real session"]
fn a_platform_without_button_support_refuses_the_reader() {
    if caps().cursor_buttons {
        return;
    }
    assert!(
        PointerCapturer::open(everywhere(), 1.0).is_err(),
        "a backend with no button support should refuse rather than report none held"
    );
}
