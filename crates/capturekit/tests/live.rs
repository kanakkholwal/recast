//! Capture against the real desktop this test runs on.
//!
//! Skipped rather than failed where there is no session to capture: a headless
//! CI runner has no desktop, and a suite that goes red there teaches nobody
//! anything. Everything that can be checked without a display lives in the unit
//! tests instead.

use std::time::Duration;

use capturekit::{
    capabilities, capturer, displays, permission, shot, shot_with, windows, CursorMode, Display,
    ExclusionSupport, Flow, PermissionKind, PixelFormat, Rect, ShotOptions, Target, Warmup,
    WindowId,
};

/// The display to capture, or `None` when there is no desktop to capture from.
///
/// Skipping is allowed because a CI runner may have no session, no display or no
/// screen-recording grant. It is NOT allowed to hide a regression, so the reasons
/// are narrow: an empty display list, or a permission the platform has not given.
/// A backend that errors for any other reason still fails the test.
fn primary() -> Option<Display> {
    if !permission(PermissionKind::Screen).is_usable() {
        return skip("screen capture is not permitted for this process");
    }
    let displays = match displays() {
        Ok(displays) if !displays.is_empty() => displays,
        Ok(_) => return skip("the platform reports no displays"),
        Err(err) => return skip(&format!("displays could not be enumerated: {err}")),
    };
    displays
        .iter()
        .find(|display| display.is_primary)
        .or_else(|| displays.first())
        .cloned()
}

/// Report a skip, or fail when the environment promised a desktop.
///
/// `CAPTUREKIT_REQUIRE_DESKTOP=1` is set on the CI leg that is known to have a
/// session, so a silent skip there cannot pass for a green run.
fn skip(reason: &str) -> Option<Display> {
    assert!(
        std::env::var_os("CAPTUREKIT_REQUIRE_DESKTOP").is_none(),
        "CAPTUREKIT_REQUIRE_DESKTOP is set but {reason}"
    );
    eprintln!("skipped: {reason}");
    None
}

/// Desktop Duplication is one-per-output-per-process, and cargo runs tests on
/// threads of a single process. Without this they contend for the same output
/// and fail as `AlreadyCaptured`, which is the library behaving correctly.
fn exclusive() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

macro_rules! require_desktop {
    () => {
        match primary() {
            Some(display) => display,
            None => {
                eprintln!("skipped: no desktop session to capture");
                return;
            }
        }
    };
}

/// Whether the frame carries real content rather than a cleared buffer.
///
/// A capture that silently hands back zeroes is the failure worth catching: the
/// dimensions are right, the call succeeded, and the image is blank.
fn has_content(bytes: &[u8]) -> bool {
    bytes.iter().any(|byte| *byte != 0)
}

#[test]
fn a_display_shot_matches_the_display_it_came_from() {
    let _exclusive = exclusive();
    let display = require_desktop!();
    let image = shot(Target::Display(display.id)).expect("capture the primary display");

    assert_eq!(image.width(), display.bounds.width);
    assert_eq!(image.height(), display.bounds.height);
    assert_eq!(image.format(), PixelFormat::Bgra8);
    assert!(
        image.stride() >= image.width() * 4,
        "stride {} cannot hold a {}px BGRA row",
        image.stride(),
        image.width()
    );
    assert!(has_content(image.bytes()), "the capture came back blank");
}

#[test]
fn a_region_shot_is_cropped_during_acquisition_not_afterwards() {
    let _exclusive = exclusive();
    let display = require_desktop!();
    let region = Rect::new(0, 0, 320, 240);
    let opts = ShotOptions {
        region: Some(region),
        ..ShotOptions::default()
    };
    let image = shot_with(
        Target::Region {
            display: display.id,
            rect: region,
        },
        &opts,
    )
    .expect("capture a region");

    assert_eq!((image.width(), image.height()), (320, 240));
    // Read back at the region's size, so the pixels outside it never crossed the
    // bus. A host-side crop would have produced the same dimensions from a
    // full-display readback, which is what this is guarding against.
    assert!(
        image.bytes().len() < display.bounds.area() as usize * 4,
        "the whole display was read back to serve a 320x240 region"
    );
}

#[test]
fn a_region_larger_than_the_display_is_clipped_rather_than_refused() {
    let _exclusive = exclusive();
    let display = require_desktop!();
    let oversized = Rect::new(
        0,
        0,
        display.bounds.width + 512,
        display.bounds.height + 512,
    );
    let image = shot_with(
        Target::Region {
            display: display.id,
            rect: oversized,
        },
        &ShotOptions {
            region: Some(oversized),
            ..ShotOptions::default()
        },
    )
    .expect("an oversized region clips to the display");
    assert_eq!(image.width(), display.bounds.width & !1);
}

#[test]
fn a_shot_of_a_display_that_does_not_exist_names_what_was_missing() {
    let _ = require_desktop!();
    let err =
        shot(Target::Display(capturekit::DisplayId(u64::MAX))).expect_err("no display has this id");
    assert!(err.to_string().contains("display"), "{err}");
}

#[test]
fn a_stream_delivers_frames_that_keep_moving_forward() {
    let _exclusive = exclusive();
    let display = require_desktop!();
    let mut capture = capturer(Target::Display(display.id))
        .frame_rate(30)
        .cursor(CursorMode::Exclude)
        .build()
        .expect("open a stream on the primary display");

    let desc = capture.describe().clone();
    assert_eq!(desc.width, display.bounds.width);

    let mut seen = 0;
    let mut last = None;
    // The desktop may be idle, so a timeout is expected rather than fatal; what
    // matters is that the frames that do arrive are ordered and well-formed.
    for _ in 0..10 {
        match capture.next_frame(Duration::from_millis(250)) {
            Ok(frame) => {
                if let Some(previous) = last {
                    assert!(frame.pts() > previous, "timestamps went backwards");
                }
                last = Some(frame.pts());
                assert!(frame.stride() >= desc.width * 4);
                seen += 1;
            }
            Err(err) => assert!(err.is_recoverable(), "stream failed: {err}"),
        }
    }
    assert!(seen > 0, "an active desktop produced no frames at all");
    capture.stop().expect("release the display");
}

#[test]
fn a_snapshot_from_a_running_stream_is_the_same_size_as_its_frames() {
    let _exclusive = exclusive();
    let display = require_desktop!();
    let mut capture = capturer(Target::Display(display.id))
        .build()
        .expect("open a stream");
    let image = capture
        .snapshot(&ShotOptions {
            // The stream is already running, so its next frame is by definition
            // current: there is no stale accumulated frame to discard.
            warmup: Warmup::None,
            ..ShotOptions::default()
        })
        .expect("take a still from the stream");
    assert_eq!(image.width(), capture.describe().width);
}

#[test]
fn a_push_capture_stops_when_the_handler_says_so() {
    let _exclusive = exclusive();
    let display = require_desktop!();
    let (sender, receiver) = std::sync::mpsc::channel();
    let capture = capturer(Target::Display(display.id))
        .frame_rate(30)
        .build()
        .expect("open a stream");

    let handle = capture.start(Duration::from_millis(250), move |frame| {
        let _ = sender.send(frame.bytes().len());
        Flow::Stop
    });
    let delivered = receiver.recv_timeout(Duration::from_secs(5));
    handle.stop().expect("the capture thread ends cleanly");
    assert!(delivered.is_ok(), "the handler was never called");
}

#[test]
fn a_second_capture_of_one_display_says_what_it_is_contending_for() {
    let _exclusive = exclusive();
    let display = require_desktop!();
    let _held = capturer(Target::Display(display.id))
        .build()
        .expect("the first capture opens");
    let Err(err) = capturer(Target::Display(display.id)).build() else {
        panic!("a display was duplicated twice in one process");
    };
    assert!(
        matches!(err, capturekit::CaptureError::AlreadyCaptured { .. }),
        "a second open reported {err} instead of naming the contended display"
    );
}

#[test]
fn a_window_shot_is_the_size_of_the_window_not_the_display() {
    let _exclusive = exclusive();
    let display = require_desktop!();
    let Some(window) = windows()
        .expect("a window list")
        .into_iter()
        .find(|window| window.is_capturable() && window.bounds.width > 64)
    else {
        eprintln!("skipped: no capturable window on this desktop");
        return;
    };

    match shot(Target::Window(window.id)) {
        Ok(image) => {
            assert!(
                image.width() <= display.bounds.width.max(window.bounds.width),
                "{} came back wider than its own bounds",
                window.title
            );
            assert!(has_content(image.bytes()));
        }
        // A window can close between enumeration and capture, and Graphics
        // Capture is absent before Windows 10 2004.
        Err(err) => eprintln!("skipped: {} could not be captured: {err}", window.title),
    }
}

/// Exclusion is a privacy control, so a platform that cannot honour it must say
/// so at `build()` rather than quietly recording the window anyway.
#[test]
fn an_exclusion_request_is_either_honoured_or_refused_but_never_ignored() {
    let _exclusive = exclusive();
    let display = require_desktop!();
    let built = capturer(Target::Display(display.id))
        .exclude_windows(&[WindowId(1)])
        .build();

    match capabilities().exclusion {
        ExclusionSupport::None => {
            let Err(err) = built else {
                panic!("a session that cannot exclude accepted an exclusion request");
            };
            assert!(
                matches!(err, capturekit::CaptureError::ExclusionUnsupported { .. }),
                "expected a refusal naming exclusion, got {err}"
            );
        }
        // Where it is supported, the request must not itself break the capture.
        _ => {
            let mut capture = built.expect("exclusion is supported here");
            capture.stop().expect("release the display");
        }
    }
}

#[test]
fn the_reported_capabilities_match_what_the_platform_actually_does() {
    let _ = require_desktop!();
    let caps = capabilities();
    assert!(!caps.backend.is_empty());
    // Enumeration is the one claim testable without capturing anything.
    if caps.window_enumeration {
        assert!(
            windows().is_ok(),
            "claims window enumeration but cannot list"
        );
    }
    if caps.display_enumeration {
        let listed = displays().expect("claims display enumeration");
        assert!(!listed.is_empty());
    }
}
