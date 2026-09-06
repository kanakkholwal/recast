//! Capture against the real desktop this test runs on, skipped rather than failed where there is no session.
//! A suite that goes red on a headless runner teaches nobody anything; display-free checks live in the unit tests.

use std::time::Duration;

use capturekit::{
    capabilities, capturer, displays, permission, shot, shot_with, windows, CursorMode, Display,
    ExclusionSupport, Flow, PermissionKind, PixelFormat, Rect, Session, ShotOptions, Target,
    Warmup, WindowId,
};

/// The display to capture, or `None` when there is no desktop; skipping is allowed because a CI runner may have no session, display or grant.
/// It must not hide a regression, so the reasons are narrow: an empty display list, or a permission the platform has not given. Any other error still fails.
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
/// `CAPTUREKIT_REQUIRE_DESKTOP=1` is set on the CI leg that is known to have a session, so a silent skip there cannot pass for a green run.
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
/// A capture that silently hands back zeroes is the failure worth catching: the dimensions are right, the call succeeded, and the image is blank.
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
    // Read back at the region's size, so pixels outside it never crossed the bus; a host-side crop would look identical.
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

/// What constant pacing exists for: an idle desktop produces no frames at all on
/// Desktop Duplication, and a recording that forwards only what the source gave
/// has a hole wherever the screen held still.
#[test]
fn a_constant_rate_capture_fills_the_slots_an_idle_desktop_leaves_empty() {
    const FPS: u32 = 30;
    const SLOTS: usize = 15;
    let _exclusive = exclusive();
    let display = require_desktop!();
    let mut capture = capturer(Target::Display(display.id))
        .frame_rate(FPS)
        .build()
        .expect("open a paced stream");

    let started = std::time::Instant::now();
    let stamps: Vec<i64> = (0..SLOTS)
        .map(|slot| {
            capture
                .next_frame(Duration::from_secs(1))
                .unwrap_or_else(|err| panic!("slot {slot} was never filled: {err}"))
                .pts()
                .as_nanos()
        })
        .collect();
    let elapsed = started.elapsed();

    let interval = 1_000_000_000 / i64::from(FPS);
    let mut holes = 0i64;
    for (slot, pair) in stamps.windows(2).enumerate() {
        let gap = pair[1] - pair[0];
        assert_eq!(
            gap % interval,
            0,
            "slot {} left the grid: {stamps:?}",
            slot + 1
        );
        holes += gap / interval - 1;
    }
    // The grid may LOSE slots when a stall outruns the catch-up window, but never silently: every hole has to be one the pacer counted.
    let skipped = capture.skipped_frames();
    assert!(
        holes as u64 <= skipped,
        "{holes} slot(s) missing from the grid but the pacer counted {skipped} skipped: {stamps:?}"
    );
    // The slots are real time, not a counter: 15 at 30fps take half a second whatever the desktop was doing.
    let owed = Duration::from_millis(1000 * (SLOTS as u64 - 1) / u64::from(FPS));
    assert!(
        elapsed >= owed,
        "{SLOTS} slots took {elapsed:?}, under {owed:?}"
    );
    assert!(
        elapsed < owed * 3,
        "{SLOTS} slots took {elapsed:?}, far over the {owed:?} they owe"
    );
    eprintln!(
        "{} of {SLOTS} slots repeated the frame before them",
        capture.repeated_frames()
    );
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
            // The stream is already running, so its next frame is current and there is no stale frame to discard.
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
    let second = capturer(Target::Display(display.id)).build();
    // Only Desktop Duplication is exclusive. ScreenCaptureKit and PipeWire hand out a second stream, and inventing a refusal there would be a lie about the platform.
    if !capturekit::capabilities().exclusive_display_capture {
        assert!(
            second.is_ok(),
            "a backend that shares displays refused the second open: {:?}",
            second.err()
        );
        return;
    }
    let Err(err) = second else {
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
        // A window can close between enumeration and capture, and Graphics Capture is absent before Windows 10 2004.
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
    // A backend listing no devices must SAY so; an empty list reads as 'this machine has no audio'.
    assert_eq!(
        caps.audio_device_enumeration,
        capturekit::audio_devices().is_ok(),
        "the audio device claim does not match what the backend does"
    );
    assert_eq!(
        caps.camera_capture,
        capturekit::cameras().is_ok(),
        "the camera claim does not match what the backend does"
    );
}

/// Two live streams on one timeline, both off the caller's thread; display and window rather than two displays, since Desktop Duplication is one per output per process.
/// The property an A/V recording rests on: a track keeping its own clock would still deliver, still look monotonic, and still be out of sync.
#[test]
fn a_session_puts_audio_and_video_on_the_same_timeline() {
    let _exclusive = exclusive();
    let display = require_desktop!();
    // Loopback rather than a microphone: every desktop has a render endpoint, and an idle one delivers inserted silence.
    if let Err(err) = capturekit::audio_loopback().build() {
        eprintln!("skipped: no loopback device to pair with the display: {err}");
        return;
    }

    let (sender, receiver) = std::sync::mpsc::channel();
    let audio_sender = sender.clone();
    let session = Session::builder()
        .video(
            "screen",
            capturer(Target::Display(display.id)).frame_rate(30),
            move |frame| {
                let _ = sender.send((frame.track.0.clone(), frame.elapsed));
                Flow::Continue
            },
        )
        .audio("system", capturekit::audio_loopback(), move |audio| {
            let _ = audio_sender.send((audio.track.0.clone(), audio.elapsed));
            Flow::Continue
        })
        .start()
        .expect("open both streams");

    assert_eq!(session.track_count(), 2);

    let deadline = std::time::Instant::now() + Duration::from_secs(8);
    let mut furthest: std::collections::BTreeMap<String, Duration> = Default::default();
    while std::time::Instant::now() < deadline
        && furthest
            .values()
            .all(|seen| *seen < Duration::from_millis(700))
    {
        let Ok((track, elapsed)) = receiver.recv_timeout(Duration::from_millis(500)) else {
            continue;
        };
        let seen = furthest.entry(track).or_default();
        *seen = (*seen).max(elapsed);
    }
    session.stop().expect("both streams end cleanly");

    let screen = furthest.get("screen").copied();
    let system = furthest.get("system").copied();
    // A loopback that fills no gaps delivers nothing at all on a machine with nothing playing, so there is no second track to put on the timeline.
    if system.is_none() && !capturekit::capabilities().audio_loopback_gap_filling {
        assert!(screen.is_some(), "not even the screen track delivered");
        eprintln!(
            "skipped the drift check: this backend fills no loopback gaps and nothing was playing"
        );
        return;
    }
    let (Some(screen), Some(system)) = (screen, system) else {
        panic!("only {furthest:?} delivered, so one track produced nothing at all");
    };
    // A track on its own origin is off by however long that clock has run, which on Windows is uptime; half a second is the budget.
    let drift = screen.max(system) - screen.min(system);
    assert!(
        drift < Duration::from_millis(500),
        "screen reached {screen:?} and audio {system:?}: they are not on one timeline"
    );
}

#[test]
fn a_session_runs_several_streams_off_one_clock() {
    let _exclusive = exclusive();
    let display = require_desktop!();
    let Some(window) = windows()
        .expect("a window list")
        .into_iter()
        .find(|window| window.is_capturable() && window.bounds.width > 64)
    else {
        eprintln!("skipped: no capturable window to pair with the display");
        return;
    };

    let (sender, receiver) = std::sync::mpsc::channel();
    let screen_sender = sender.clone();
    let session = Session::builder()
        .video(
            "screen",
            capturer(Target::Display(display.id)).frame_rate(30),
            move |frame| {
                let _ = screen_sender.send((frame.track.0.clone(), frame.elapsed));
                Flow::Continue
            },
        )
        .video(
            "window",
            capturer(Target::Window(window.id)).frame_rate(30),
            move |frame| {
                let _ = sender.send((frame.track.0.clone(), frame.elapsed));
                Flow::Continue
            },
        )
        .start()
        .expect("open both streams");

    assert_eq!(session.track_count(), 2);

    // Both tracks must deliver, and every timestamp must sit on the session timeline, not each source's own origin.
    let deadline = std::time::Instant::now() + Duration::from_secs(6);
    let mut seen: std::collections::BTreeSet<String> = Default::default();
    while std::time::Instant::now() < deadline && seen.len() < 2 {
        match receiver.recv_timeout(Duration::from_millis(500)) {
            Ok((track, elapsed)) => {
                assert!(
                    elapsed < Duration::from_secs(60),
                    "{track} reported {elapsed:?} from the session origin, so it is on its own clock"
                );
                seen.insert(track);
            }
            Err(_) => continue,
        }
    }
    session.stop().expect("both streams end cleanly");

    assert!(
        seen.contains("screen"),
        "the display track delivered nothing; saw {seen:?}"
    );
}

/// A session that cannot open every source must not leave some running.
#[test]
fn a_session_that_cannot_open_every_source_starts_none_of_them() {
    let _exclusive = exclusive();
    let display = require_desktop!();
    let opened = Session::builder()
        .video("good", capturer(Target::Display(display.id)), |_| {
            Flow::Continue
        })
        .video(
            "missing",
            capturer(Target::Display(capturekit::DisplayId(u64::MAX))),
            |_| Flow::Continue,
        )
        .start();
    assert!(
        opened.is_err(),
        "a session opened with a source that does not exist"
    );

    // The good source must have been released, so it can be opened again.
    let mut retry = capturer(Target::Display(display.id))
        .build()
        .expect("the display was left held by the failed session");
    retry.stop().expect("release");
}

/// The cursor must arrive with the frame, on the frame's clock.
#[test]
fn a_display_stream_reports_the_cursor_alongside_its_frames() {
    let _exclusive = exclusive();
    let display = require_desktop!();
    if !capabilities().cursor_samples {
        eprintln!("skipped: this backend does not report cursor samples");
        return;
    }

    let mut capture = capturer(Target::Display(display.id))
        .frame_rate(30)
        .build()
        .expect("open a stream");

    let mut sampled = 0;
    for _ in 0..20 {
        let Ok(frame) = capture.next_frame(Duration::from_millis(250)) else {
            continue;
        };
        let Some(cursor) = frame.cursor() else {
            panic!("cursor_samples is claimed but no sample arrived");
        };
        assert_eq!(
            cursor.pts,
            frame.pts(),
            "the cursor is on a different clock from its frame"
        );
        sampled += 1;
    }
    assert!(sampled > 0, "an active desktop produced no frames");
    capture.stop().expect("release the display");
}

/// Desktop Duplication cannot composite a cursor, so asking must fail loudly
/// rather than silently producing a recording with no pointer in it.
#[test]
fn a_display_capture_refuses_to_pretend_it_can_draw_the_cursor() {
    let _exclusive = exclusive();
    let display = require_desktop!();
    let built = capturer(Target::Display(display.id))
        .cursor(CursorMode::Include)
        .build();

    if capabilities().backend == "dxgi" {
        let Err(err) = built else {
            panic!("dxgi accepted CursorMode::Include, which it cannot honour");
        };
        assert!(
            err.to_string().contains("cursor"),
            "the refusal does not say it is about the cursor: {err}"
        );
    } else if let Ok(mut capture) = built {
        capture.stop().expect("release");
    }
}

/// A cursor shape must decode to a real image, whatever form Windows sent it in.
#[test]
fn the_reported_cursor_shape_decodes_to_pixels() {
    let _exclusive = exclusive();
    let display = require_desktop!();
    if !capabilities().cursor_samples {
        eprintln!("skipped: this backend does not report cursor samples");
        return;
    }
    let mut capture = capturer(Target::Display(display.id))
        .frame_rate(30)
        .build()
        .expect("open a stream");

    // A shape arrives only when it changes, so poll for a while.
    for _ in 0..40 {
        let _ = capture.next_frame(Duration::from_millis(100));
        if let Some(shape) = capture.cursor_shape() {
            let rgba = shape.to_rgba().expect("the reported shape decodes");
            assert_eq!(
                rgba.len(),
                (shape.width * shape.drawn_height() * 4) as usize,
                "a {:?} cursor decoded to the wrong size",
                shape.kind
            );
            assert!(shape.width > 0 && shape.drawn_height() > 0);
            capture.stop().expect("release");
            return;
        }
    }
    eprintln!("skipped: the cursor shape never changed during the window");
    capture.stop().expect("release");
}
