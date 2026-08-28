//! `capabilities()` is a question, not a capture, and must stay answerable.
//!
//! It reports WinRT-backed facts, and a WinRT call needs an apartment. Audio
//! capture threads create and destroy apartments as takes come and go, so a
//! caller asking between takes must not be reading a torn-down one.

use std::time::Duration;

use capturekit::{audio_loopback, capabilities};

/// The recorder's exact sequence: open a track, ask what the platform can do
/// while it runs, and do it all again for the next take.
#[test]
fn capabilities_survive_audio_capture_starting_and_stopping() {
    for attempt in 0..3 {
        let Ok(mut capturer) = audio_loopback().build() else {
            eprintln!("skipped: no loopback endpoint on this machine");
            return;
        };
        let _ = capturer.next_buffer(Duration::from_millis(100));
        let during = capabilities();
        capturer.stop().expect("release the endpoint");
        let after = capabilities();
        assert_eq!(during.window_capture, after.window_capture);
        eprintln!("attempt {attempt}: capabilities answered on both sides");
    }
}

/// Each take runs on its own thread that then exits, which is what churns the
/// apartment the WinRT call depends on.
#[test]
fn capabilities_survive_audio_capture_on_threads_that_come_and_go() {
    for attempt in 0..3 {
        let opened = std::thread::spawn(|| {
            let Ok(mut capturer) = audio_loopback().build() else {
                return false;
            };
            let _ = capturer.next_buffer(Duration::from_millis(100));
            capturer.stop().expect("release the endpoint");
            true
        })
        .join()
        .expect("the capture thread did not panic");
        if !opened {
            eprintln!("skipped: no loopback endpoint on this machine");
            return;
        }
        let _ = capabilities();
        eprintln!("attempt {attempt}: capabilities answered after the take");
    }
}

#[test]
fn capabilities_do_not_depend_on_the_calling_thread() {
    let main = capabilities();
    let bare = std::thread::spawn(capabilities)
        .join()
        .expect("the probe thread did not panic");
    assert_eq!(
        main.window_capture, bare.window_capture,
        "window_capture flipped between the main thread and a bare one"
    );
    assert_eq!(
        main.cursor_in_frame, bare.cursor_in_frame,
        "cursor_in_frame flipped between the main thread and a bare one"
    );
}
