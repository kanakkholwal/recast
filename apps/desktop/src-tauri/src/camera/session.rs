//! The one owner of the camera device: cameras are exclusive, so the WebView cannot hold it while a recording reads it.
//! One thread fans frames to the preview and the file, putting the camera on the same [`RecordingClock`] as the screen.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use capturekit::Target;

use super::scale::downscale_bgra_into;
use crate::encoder::{
    pack_rows, pack_rows_into, spawn_encoder_loop, EncoderConfig, RecordingQuality,
};
use crate::recording::pipeline::RecordingPipeline;
use crate::recording::TrackStart;

/// How long to wait for a frame before re-checking the stop flag.
const POLL: Duration = Duration::from_millis(250);

/// The rate the camera is opened and encoded at. One constant on purpose: the
/// captured frames carry no timestamps, so requesting one rate and declaring
/// another to the encoder rescales the whole track.
const CAMERA_FPS: u32 = 30;

/// How long one attempt waits for the first frame. A cold USB webcam negotiates
/// format and runs auto-exposure first, so this is seconds rather than millis.
const FIRST_FRAME: Duration = Duration::from_secs(4);

/// How many times to reopen before giving up.
/// A reader that missed `FIRST_FRAME` will not start by waiting longer but usually will on a fresh open, since the device is briefly held by whatever released it last.
const OPEN_ATTEMPTS: u32 = 3;

/// Pause between attempts, letting the previous holder finish releasing.
const RETRY_BACKOFF: Duration = Duration::from_millis(400);

/// Frames buffered between the camera thread and the file encoder.
const QUEUE_DEPTH: usize = 8;

/// Longest edge of a preview frame. Capture resolution over IPC is 110 MB/s at
/// 720p30; the bubble is a few hundred pixels, so this is what it can use.
const PREVIEW_MAX_DIM: u32 = 480;

/// Where preview frames go. Boxed so the camera thread does not depend on Tauri.
pub type FrameSink = Box<dyn Fn(Vec<u8>) + Send + 'static>;

/// The negotiated capture size, plus the token identifying this session.
/// The panel reopens the preview immediately, so an old window's teardown can land after the new one's open; keying stop on this token means a stale window cannot close a live camera.
#[derive(Clone, Copy, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CameraGeometry {
    pub width: u32,
    pub height: u32,
    /// The rate the capture is paced at, which the recorder MUST encode at:
    /// its frames carry no timestamps, so any other rate rescales the track.
    pub fps: u32,
    pub session: u64,
}

/// A file being written from the live camera.
struct Recorder {
    pipeline: RecordingPipeline,
    encoder: thread::JoinHandle<Result<()>>,
    stop: Arc<AtomicBool>,
    track: TrackStart,
    pause: Arc<AtomicBool>,
}

struct Running {
    session: u64,
    device: String,
    stop: Arc<AtomicBool>,
    thread: thread::JoinHandle<()>,
    geometry: CameraGeometry,
    recorder: Arc<Mutex<Option<Recorder>>>,
    /// Swapped when the preview window reopens: the old window's channel is
    /// dead, and frames sent to it would never reach the new one.
    sink: Arc<Mutex<FrameSink>>,
}

/// Monotonic session token. Zero is never handed out, so it means "no session".
fn next_session() -> u64 {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

fn slot() -> &'static Mutex<Option<Running>> {
    static SLOT: OnceLock<Mutex<Option<Running>>> = OnceLock::new();
    SLOT.get_or_init(|| Mutex::new(None))
}

/// Open `device` and start delivering preview frames to `sink`.
/// Each preview frame is `width: u32le, height: u32le` followed by BGRA rows, so the receiver needs no side channel to size itself.
pub fn start(device: &str, sink: FrameSink) -> Result<CameraGeometry> {
    let mut held = slot().lock().map_err(|_| anyhow!("camera lock poisoned"))?;
    if let Some(running) = held.as_mut() {
        if running.device == device {
            {
                let mut current = running
                    .sink
                    .lock()
                    .map_err(|_| anyhow!("camera sink lock poisoned"))?;
                *current = sink;
            }
            // A fresh token so the window this replaced cannot stop the camera.
            running.session = next_session();
            running.geometry.session = running.session;
            return Ok(running.geometry);
        }
    }
    // The old camera must let go before a different one can open.
    if let Some(previous) = held.take() {
        shut_down(previous);
    }

    if !super::supported() {
        return Err(anyhow!(
            "camera capture is not supported on this platform yet"
        ));
    }
    let cameras = super::devices().map_err(|e| anyhow!("camera enumeration failed: {e}"))?;
    let camera = super::find(&cameras, device)
        .ok_or_else(|| anyhow!("camera \"{device}\" is not available"))?;

    let (capturer, geometry, opening) = open_streaming(device, &camera.id)?;
    let session = geometry.session;

    let stop = Arc::new(AtomicBool::new(false));
    let recorder: Arc<Mutex<Option<Recorder>>> = Arc::new(Mutex::new(None));
    let sink = Arc::new(Mutex::new(sink));
    let thread = thread::Builder::new()
        .name("recast-camera".into())
        .spawn({
            let stop = stop.clone();
            let recorder = recorder.clone();
            let sink = sink.clone();
            move || pump(capturer, geometry, opening, &sink, &recorder, &stop)
        })
        .context("failed to spawn the camera thread")?;

    *held = Some(Running {
        session,
        device: device.to_string(),
        stop,
        thread,
        geometry,
        recorder,
        sink,
    });
    Ok(geometry)
}

/// Open the camera and take its first frame, reopening on a transient failure.
fn open_streaming(
    device: &str,
    id: &capturekit::CameraId,
) -> Result<(capturekit::Capturer, CameraGeometry, Vec<u8>)> {
    let mut last = None;
    for attempt in 1..=OPEN_ATTEMPTS {
        match try_open(device, id) {
            Ok(open) => return Ok(open),
            Err(e) => {
                let retryable = e
                    .downcast_ref::<capturekit::CaptureError>()
                    .is_some_and(capturekit::CaptureError::is_recoverable);
                if !retryable || attempt == OPEN_ATTEMPTS {
                    return Err(e);
                }
                log::warn!("camera \"{device}\" attempt {attempt} failed, reopening: {e:#}");
                last = Some(e);
                thread::sleep(RETRY_BACKOFF);
            }
        }
    }
    Err(last.unwrap_or_else(|| anyhow!("camera \"{device}\" could not be opened")))
}

fn try_open(
    device: &str,
    id: &capturekit::CameraId,
) -> Result<(capturekit::Capturer, CameraGeometry, Vec<u8>)> {
    let mut capturer = capturekit::capturer(Target::Camera(id.clone()))
        .frame_rate(CAMERA_FPS)
        .build()
        .with_context(|| format!("failed to open camera \"{device}\""))?;
    let first = capturer
        .next_frame(FIRST_FRAME)
        .with_context(|| format!("camera \"{device}\" produced no frames"))?;
    let geometry = CameraGeometry {
        width: first.desc().width,
        height: first.desc().height,
        fps: CAMERA_FPS,
        session: next_session(),
    };
    let opening = pack_rows(
        first.bytes(),
        first.stride(),
        geometry.width,
        geometry.height,
    );
    drop(first);
    Ok((capturer, geometry, opening))
}

/// Release the device held by `session`. Idempotent, and a no-op for a token
/// that is not the live one, so a closing window cannot stop its replacement.
pub fn stop(session: u64) {
    let taken = slot().lock().ok().and_then(|mut held| {
        let matches = held.as_ref().is_some_and(|r| r.session == session);
        matches.then(|| held.take()).flatten()
    });
    if let Some(running) = taken {
        shut_down(running);
    }
}

/// Releases the camera whatever its session token, for a preview window that is already gone.
/// The panel closes the preview with a raw `close()`, so the webview dies holding the token [`stop`] needs, and the capture thread would run forever with the light on.
pub fn release() {
    let taken = slot().lock().ok().and_then(|mut held| held.take());
    if let Some(running) = taken {
        log::info!("camera released: its preview window is gone");
        shut_down(running);
    }
}

/// How the camera's frames are encoded. The rate comes from the geometry and
/// nowhere else: the frames carry no timestamps, so declaring the screen's rate
/// here would rescale the whole camera track.
fn encoder_config(geometry: CameraGeometry, dest: PathBuf) -> EncoderConfig {
    EncoderConfig {
        width: geometry.width,
        height: geometry.height,
        fps: geometry.fps,
        crop: None,
        output_path: dest,
        quality: RecordingQuality::default(),
    }
}

/// Begin writing the live camera to `dest`. The session must already be running.
pub fn attach_recorder(dest: PathBuf, track: TrackStart, pause: Arc<AtomicBool>) -> Result<()> {
    let held = slot().lock().map_err(|_| anyhow!("camera lock poisoned"))?;
    let running = held
        .as_ref()
        .ok_or_else(|| anyhow!("no camera is open to record"))?;

    let pipeline = RecordingPipeline::new(QUEUE_DEPTH);
    let stop = Arc::new(AtomicBool::new(false));
    let encoder = spawn_encoder_loop(
        encoder_config(running.geometry, dest),
        stop.clone(),
        pipeline.clone(),
    )?;

    let mut sink = running
        .recorder
        .lock()
        .map_err(|_| anyhow!("camera recorder lock poisoned"))?;
    *sink = Some(Recorder {
        pipeline,
        encoder,
        stop,
        track,
        pause,
    });
    Ok(())
}

/// Finish the recording started by [`attach_recorder`], leaving the preview running.
pub fn detach_recorder() -> Result<()> {
    let taken = {
        let held = slot().lock().map_err(|_| anyhow!("camera lock poisoned"))?;
        let Some(running) = held.as_ref() else {
            return Ok(());
        };
        let mut sink = running
            .recorder
            .lock()
            .map_err(|_| anyhow!("camera recorder lock poisoned"))?;
        sink.take()
    };
    let Some(recorder) = taken else {
        return Ok(());
    };
    recorder.stop.store(true, Ordering::Release);
    recorder
        .encoder
        .join()
        .map_err(|_| anyhow!("the camera encoder thread panicked"))?
}

fn shut_down(running: Running) {
    if let Ok(mut sink) = running.recorder.lock() {
        if let Some(recorder) = sink.take() {
            recorder.stop.store(true, Ordering::Release);
            let _ = recorder.encoder.join();
        }
    }
    running.stop.store(true, Ordering::Release);
    let _ = running.thread.join();
}

/// Whether the pump may wait for another frame after `error`; only a timeout qualifies.
/// Every other error means the delivery worker finished, so `next_frame` returns instantly forever: continuing is a hot loop re-sending the last frame over IPC.
const fn keep_pumping(error: &capturekit::CaptureError) -> bool {
    matches!(error, capturekit::CaptureError::Timeout(_))
}

fn pump(
    mut capturer: capturekit::Capturer,
    geometry: CameraGeometry,
    opening: Vec<u8>,
    sink: &Mutex<FrameSink>,
    recorder: &Mutex<Option<Recorder>>,
    stop: &AtomicBool,
) {
    let mut packed = opening;
    loop {
        deliver(&packed, geometry, sink, recorder);
        if stop.load(Ordering::Acquire) {
            break;
        }
        let frame = match capturer.next_frame(POLL) {
            Ok(frame) => frame,
            // A USB hiccup holds the last frame rather than tearing the preview down.
            Err(error) if keep_pumping(&error) => continue,
            Err(error) => {
                log::error!("camera preview stopped: {error}");
                break;
            }
        };
        // Size is negotiated once; a short buffer must never reach the encoder.
        if frame.desc().width != geometry.width || frame.desc().height != geometry.height {
            continue;
        }
        pack_rows_into(
            &mut packed,
            frame.bytes(),
            frame.stride(),
            geometry.width,
            geometry.height,
        );
    }
    let _ = capturer.stop();
}

fn deliver(
    packed: &[u8],
    geometry: CameraGeometry,
    sink: &Mutex<FrameSink>,
    recorder: &Mutex<Option<Recorder>>,
) {
    if let Ok(held) = recorder.lock() {
        if let Some(rec) = held.as_ref() {
            if !rec.pause.load(Ordering::Acquire) {
                rec.track.mark();
                rec.pipeline.push(packed.to_vec().into());
            }
        }
    }
    // Header patched after: the scaled size is only known once `fit` has run.
    let mut message = vec![0u8; 8];
    let (w, h) = downscale_bgra_into(
        &mut message,
        packed,
        geometry.width,
        geometry.height,
        PREVIEW_MAX_DIM,
    );
    message[0..4].copy_from_slice(&w.to_le_bytes());
    message[4..8].copy_from_slice(&h.to_le_bytes());
    if let Ok(send) = sink.lock() {
        send(message);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A timeout already cost `POLL`, so looping on it cannot spin.
    #[test]
    fn a_timeout_keeps_the_preview_waiting() {
        assert!(keep_pumping(&capturekit::CaptureError::Timeout(POLL)));
    }

    /// The bug this guards: the delivery worker ends on ANY read failure, and
    /// `next_frame` then returns instantly forever. Looping burned a core and
    /// re-sent the first frame until the window closed.
    #[test]
    fn an_ended_source_stops_the_pump_instead_of_spinning() {
        for reason in [
            capturekit::LostReason::AccessLost,
            capturekit::LostReason::DeviceLost,
        ] {
            let error = capturekit::CaptureError::Lost(reason);
            assert!(
                !keep_pumping(&error),
                "{error} kept the pump running on a dead capturer"
            );
        }
    }

    /// `Lost` is `is_recoverable`, so recoverability is the wrong question here:
    /// the capturer is finished either way and only a fresh open could help.
    #[test]
    fn recoverability_does_not_keep_a_finished_capturer_alive() {
        let error = capturekit::CaptureError::Lost(capturekit::LostReason::AccessLost);
        assert!(error.is_recoverable());
        assert!(!keep_pumping(&error));
    }

    /// The whole preview path, against a real device: does the sink actually receive MOVING pictures, or one frame forever?
    /// capturekit's own live test proves the device delivers distinct frames, so a failure here is in this file.
    #[test]
    #[ignore = "live: opens the real camera"]
    fn the_preview_sink_receives_moving_pictures() {
        let Ok(cameras) = super::super::devices() else {
            return;
        };
        let Some(camera) = cameras.first() else {
            return;
        };
        let seen: Arc<Mutex<Vec<u64>>> = Arc::new(Mutex::new(Vec::new()));
        let sink: FrameSink = {
            let seen = seen.clone();
            Box::new(move |message| {
                let mut hash = 1469598103934665603u64;
                for chunk in message.chunks(997) {
                    hash ^= u64::from(chunk[0]);
                    hash = hash.wrapping_mul(1099511628211);
                }
                if let Ok(mut held) = seen.lock() {
                    held.push(hash);
                }
            })
        };
        let geometry = start(&camera.name, sink).expect("open the camera");
        thread::sleep(Duration::from_secs(2));
        stop(geometry.session);

        let held = seen.lock().expect("sink lock");
        let distinct: std::collections::HashSet<u64> = held.iter().copied().collect();
        assert!(
            held.len() > 5,
            "only {} frame(s) reached the sink",
            held.len()
        );
        assert!(
            distinct.len() > 1,
            "{} frames reached the sink but carried {} distinct image(s)",
            held.len(),
            distinct.len()
        );
    }

    /// The camera is paced at its own rate and its frames carry no timestamps,
    /// so encoding at the screen's rate played a 30 fps take back at double
    /// speed and drifted further from the audio every second.
    #[test]
    fn the_encoder_runs_at_the_rate_the_camera_was_opened_at() {
        let geometry = CameraGeometry {
            width: 1280,
            height: 720,
            fps: CAMERA_FPS,
            session: 1,
        };
        let config = encoder_config(geometry, PathBuf::from("camera.mp4"));
        assert_eq!(config.fps, CAMERA_FPS);
        assert_eq!(config.fps, geometry.fps);
        assert_ne!(
            config.fps,
            crate::recording::RECORDING_FPS,
            "the screen's rate must not be what the camera is encoded at"
        );
    }

    #[test]
    fn a_preview_frame_carries_its_own_dimensions() {
        let geometry = CameraGeometry {
            width: 2,
            height: 2,
            fps: CAMERA_FPS,
            session: 1,
        };
        let packed = vec![9u8; 2 * 2 * 4];
        let seen = Arc::new(Mutex::new(Vec::new()));
        let sink: FrameSink = {
            let seen = seen.clone();
            Box::new(move |message| seen.lock().expect("sink lock").push(message))
        };
        deliver(&packed, geometry, &Mutex::new(sink), &Mutex::new(None));

        let held = seen.lock().expect("sink lock");
        let message = held.first().expect("a frame was delivered");
        let width = u32::from_le_bytes(message[0..4].try_into().expect("width header"));
        let height = u32::from_le_bytes(message[4..8].try_into().expect("height header"));
        assert_eq!((width, height), (2, 2));
        assert_eq!(message.len(), 8 + (width as usize * height as usize * 4));
    }
}
