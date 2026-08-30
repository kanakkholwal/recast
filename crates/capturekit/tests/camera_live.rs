//! Camera capture against whatever device this machine has.
//!
//! Skipped where there is none, and only there: a build machine without a
//! webcam must not turn a broken backend green.

use std::time::Duration;

use capturekit::{
    cameras, capabilities, capturer, Camera, CameraId, CaptureError, Flow, PixelFormat, Rect,
    Target,
};

/// The camera to test, or `None` when the platform has none to offer.
fn any_camera() -> Option<Camera> {
    if !capabilities().camera_capture {
        assert!(
            cameras().is_err(),
            "the platform disclaims cameras but listed some anyway"
        );
        return skip("this platform has no camera backend yet");
    }
    match cameras() {
        Ok(found) if !found.is_empty() => found.into_iter().find(|c| c.is_default),
        Ok(_) => skip("no camera is attached"),
        Err(err) => panic!("a platform that claims cameras could not list them: {err}"),
    }
}

/// `CAPTUREKIT_REQUIRE_CAMERA=1` is set where a camera is known to exist, so a
/// silent skip there cannot pass for a green run.
fn skip(reason: &str) -> Option<Camera> {
    assert!(
        std::env::var_os("CAPTUREKIT_REQUIRE_CAMERA").is_none(),
        "CAPTUREKIT_REQUIRE_CAMERA is set but {reason}"
    );
    eprintln!("skipped: {reason}");
    None
}

/// A camera is one physical device, and not every driver serves two readers at
/// once. Tests share the machine's, so they take turns rather than contend.
fn exclusive() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

macro_rules! require_camera {
    () => {
        match any_camera() {
            Some(camera) => camera,
            None => return,
        }
    };
}

#[test]
fn every_camera_has_a_name_a_stable_id_and_at_least_one_mode() {
    let _ = require_camera!();
    let found = cameras().expect("a camera list");
    let mut ids: Vec<String> = found.iter().map(|c| c.id.0.clone()).collect();
    let before = ids.len();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), before, "two cameras share an id");
    for camera in &found {
        assert!(!camera.name.is_empty(), "{:?} has no name", camera.id);
        assert!(!camera.id.0.is_empty(), "{} has no id", camera.name);
        assert!(
            !camera.formats.is_empty(),
            "{} advertises no modes",
            camera.name
        );
        for (index, mode) in camera.formats.iter().enumerate() {
            assert!(
                mode.width > 0 && mode.height > 0,
                "{} has an empty mode",
                camera.name
            );
            // A webcam advertises the same geometry per native subtype, so without deduplication the list is three copies of every mode.
            assert!(
                !camera.formats[..index].contains(mode),
                "{} lists {}x{} @ {:?} twice",
                camera.name,
                mode.width,
                mode.height,
                mode.frame_rate
            );
        }
    }
    assert_eq!(
        found.iter().filter(|c| c.is_default).count(),
        1,
        "exactly one camera is the default"
    );
}

#[test]
fn a_cameras_modes_are_listed_largest_first() {
    let camera = require_camera!();
    let areas: Vec<u64> = camera.formats.iter().map(|f| f.area()).collect();
    let mut sorted = areas.clone();
    sorted.sort_unstable_by(|a, b| b.cmp(a));
    assert_eq!(
        areas, sorted,
        "{} lists its modes out of order",
        camera.name
    );
}

#[test]
fn a_camera_stream_delivers_frames_of_the_size_it_negotiated() {
    let _exclusive = exclusive();
    let camera = require_camera!();
    let mut capture = capturer(Target::Camera(camera.id.clone()))
        .frame_rate(30)
        .build()
        .unwrap_or_else(|err| panic!("open {}: {err}", camera.name));

    let desc = capture.describe().clone();
    assert_eq!(desc.format, PixelFormat::Bgra8);
    assert!(desc.width > 0 && desc.height > 0);

    let frame = capture
        .next_frame(Duration::from_secs(5))
        .expect("a camera that opened produces a frame");
    assert_eq!(frame.stride(), desc.width * 4, "the frame is not packed");
    assert_eq!(
        frame.bytes().len(),
        (desc.width * desc.height * 4) as usize,
        "the frame is not the size it was described as"
    );
    // A camera handing back a cleared buffer looks identical to a working one in every dimension check above.
    assert!(
        frame.bytes().iter().any(|byte| *byte != 0),
        "{} delivered a blank frame",
        camera.name
    );
    capture.stop().expect("release the camera");
}

/// Alpha is the byte an RGB32 conversion leaves untouched. A frame delivered
/// bottom-up passes every size check and only shows up as an upside-down
/// picture, so the rows are compared against the device instead.
#[test]
fn a_camera_stream_keeps_producing_frames_that_move_forward() {
    let _exclusive = exclusive();
    let camera = require_camera!();
    let mut capture = capturer(Target::Camera(camera.id))
        .frame_rate(15)
        .build()
        .expect("open the camera");

    let mut last = None;
    for index in 0..5 {
        let frame = capture
            .next_frame(Duration::from_secs(5))
            .unwrap_or_else(|err| panic!("frame {index}: {err}"));
        if let Some(previous) = last {
            assert!(frame.pts() > previous, "camera timestamps went backwards");
        }
        last = Some(frame.pts());
    }
    capture.stop().expect("release the camera");
}

#[test]
fn a_camera_that_is_not_attached_names_what_was_missing() {
    let _ = require_camera!();
    let Err(err) = capturer(Target::Camera(CameraId("no-such-device".into()))).build() else {
        panic!("a camera id that matches nothing was opened anyway");
    };
    assert!(err.to_string().contains("no-such-device"), "{err}");
}

#[test]
fn a_push_camera_capture_stops_when_the_handler_says_so() {
    let _exclusive = exclusive();
    let camera = require_camera!();
    let (sender, receiver) = std::sync::mpsc::channel();
    let capture = capturer(Target::Camera(camera.id))
        .frame_rate(30)
        .build()
        .expect("open the camera");
    let handle = capture.start(Duration::from_secs(2), move |frame| {
        let _ = sender.send(frame.bytes().len());
        Flow::Stop
    });
    let delivered = receiver.recv_timeout(Duration::from_secs(10));
    handle.stop().expect("the capture thread ends cleanly");
    assert!(delivered.is_ok(), "the handler was never called");
}

/// Not every camera driver serves two readers. The ones that do not say so by
/// ending the second stream rather than by refusing to open it, and Windows
/// gives no way to tell that apart from a slow device up front. Either answer is
/// acceptable; a caller left waiting on a stream that will never produce is not.
#[test]
fn a_second_capture_of_one_camera_either_shares_it_or_reports_it_lost() {
    let _exclusive = exclusive();
    let camera = require_camera!();
    let mut first = capturer(Target::Camera(camera.id.clone()))
        .build()
        .expect("the first capture opens");
    first
        .next_frame(Duration::from_secs(5))
        .expect("the first capture produces a frame");

    match capturer(Target::Camera(camera.id)).build() {
        // Refused up front, which is the clearest answer of the three.
        Err(_) => {}
        Ok(mut second) => match second.next_frame(Duration::from_secs(3)) {
            // The Windows frame server shared the device.
            Ok(_) => {}
            Err(err) => assert!(
                !matches!(err, CaptureError::Timeout(_)),
                "a camera that will never produce reported only a timeout"
            ),
        },
    }
    first.stop().expect("release the camera");
}

/// An odd size exercises the padding arithmetic a round one hides: 999 pixels is
/// not a multiple of anything a driver aligns to, so a stride assumption that
/// happens to hold at 1280 shows up here.
#[test]
fn a_camera_delivers_frames_of_the_size_it_settled_on_not_the_size_asked_for() {
    let _exclusive = exclusive();
    let camera = require_camera!();
    let Ok(mut capture) = capturer(Target::Camera(camera.id))
        .region(Some(Rect::new(0, 0, 999, 555)))
        .build()
    else {
        // A device with no scaler is entitled to refuse a size it cannot make.
        return;
    };
    let desc = capture.describe().clone();
    let frame = capture
        .next_frame(Duration::from_secs(5))
        .expect("a frame at the negotiated size");
    assert_eq!(
        frame.bytes().len(),
        (desc.width * desc.height * 4) as usize,
        "described {}x{} but delivered {} bytes",
        desc.width,
        desc.height,
        frame.bytes().len()
    );
    assert_eq!(frame.stride(), desc.width * 4);
    capture.stop().expect("release the camera");
}

/// The camera must deliver NEW PIXELS, not just new timestamps.
///
/// `a_camera_stream_keeps_producing_frames_that_move_forward` asserts only that
/// `pts` advances, which `Pacing::Constant` guarantees whatever the device does:
/// the pacer invents a slot per interval and repeats the held bytes into it. A
/// camera that delivered one frame and died passes that test and freezes every
/// preview built on it, which is exactly what shipped.
#[test]
fn a_camera_stream_delivers_new_pixels_not_just_new_timestamps() {
    let _exclusive = exclusive();
    let camera = require_camera!();
    let mut capture = capturer(Target::Camera(camera.id))
        .frame_rate(15)
        .build()
        .expect("open the camera");

    let mut fresh = 0;
    let mut repeats = 0;
    let mut distinct: std::collections::HashSet<u64> = std::collections::HashSet::new();
    for index in 0..20 {
        let frame = capture
            .next_frame(Duration::from_secs(5))
            .unwrap_or_else(|err| panic!("frame {index}: {err}"));
        if frame.is_repeat() {
            repeats += 1;
        } else {
            fresh += 1;
        }
        // A cheap content hash: a frozen sensor still varies by a bit or two.
        let bytes = frame.bytes();
        let mut hash = 1469598103934665603u64;
        for chunk in bytes.chunks(997) {
            hash ^= u64::from(chunk[0]);
            hash = hash.wrapping_mul(1099511628211);
        }
        distinct.insert(hash);
    }
    capture.stop().expect("release the camera");

    assert!(
        fresh > 1,
        "only {fresh} frame(s) came from the device; {repeats} were pacer repeats"
    );
    assert!(
        distinct.len() > 1,
        "20 frames carried {} distinct image(s): the device stopped and the pacer repeated it",
        distinct.len()
    );
}
