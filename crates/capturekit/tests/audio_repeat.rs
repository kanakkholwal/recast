//! Repeated takes holding both directions, in the shape a recorder uses them.
//!
//! Each direction gets its own thread that opens the device, reads until told to
//! stop, and releases. A recorder does this once per take, so take two must be
//! as safe as take one.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::time::Duration;

use capturekit::{audio_input, audio_loopback, AudioDirection};

const POLL: Duration = Duration::from_millis(100);

fn spawn(direction: AudioDirection, stop: Arc<AtomicBool>) -> Option<std::thread::JoinHandle<()>> {
    let (ready, opened) = mpsc::channel();
    let handle = std::thread::spawn(move || {
        let built = match direction {
            AudioDirection::Loopback => audio_loopback().build(),
            _ => audio_input().build(),
        };
        let Ok(mut capturer) = built else {
            let _ = ready.send(false);
            return;
        };
        let _ = ready.send(true);
        while !stop.load(Ordering::Acquire) {
            let _ = capturer.next_buffer(POLL);
        }
        capturer.stop().expect("release the endpoint");
    });
    match opened.recv() {
        Ok(true) => Some(handle),
        _ => {
            let _ = handle.join();
            None
        }
    }
}

#[test]
fn both_directions_survive_repeated_takes_in_the_recorder_shape() {
    for attempt in 0..3 {
        eprintln!("take {attempt}: opening loopback");
        let stop = Arc::new(AtomicBool::new(false));
        let Some(loopback) = spawn(AudioDirection::Loopback, stop.clone()) else {
            eprintln!("skipped: no loopback endpoint");
            return;
        };
        eprintln!("take {attempt}: opening microphone");
        let microphone = spawn(AudioDirection::Input, stop.clone());
        eprintln!("take {attempt}: capturing");
        std::thread::sleep(Duration::from_millis(600));
        stop.store(true, Ordering::Release);
        loopback.join().expect("loopback thread");
        if let Some(mic) = microphone {
            mic.join().expect("microphone thread");
        }
        eprintln!("take {attempt}: released");
    }
}
