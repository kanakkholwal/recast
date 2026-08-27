//! The one owner of the camera device.
//!
//! Cameras are exclusive: a second reader gets nothing (verified on Windows,
//! where a concurrent open times out). So the WebView cannot hold the device
//! with `getUserMedia` while a recording reads it. One thread owns the camera
//! and fans frames out to the live preview and, while recording, to a file.
//!
//! That also puts the camera on the same [`RecordingClock`] as the screen, so
//! the A/V offset is measured rather than reported over IPC in wall-clock time.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use capturekit::Target;

use super::scale::downscale_bgra;
use crate::encoder::{spawn_encoder_loop, EncoderConfig, RecordingQuality};
use crate::recording::pipeline::{RecordingPipeline, VideoFrame};
use crate::recording::{RecordingClock, TrackStart};

/// How long to wait for a frame before re-checking the stop flag.
const POLL: Duration = Duration::from_millis(250);

/// How long one attempt waits for the first frame. A cold USB webcam negotiates
/// format and runs auto-exposure first, so this is seconds rather than millis.
const FIRST_FRAME: Duration = Duration::from_secs(4);

/// How many times to reopen before giving up.
///
/// A source reader that has not delivered in `FIRST_FRAME` will not start by
/// being waited on longer, but usually will on a fresh open: the device is
/// briefly held by whatever released it last (a WebView probe, a closing
/// preview, another app). Reopening beats a single long wait.
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

/// The negotiated capture size, plus the token that identifies this session.
///
/// The panel closes the preview window and immediately opens a new one, so the
/// old window's teardown can land after the new window's open. Stopping is
/// keyed on this token, which means a stale window cannot close a live camera.
#[derive(Clone, Copy, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CameraGeometry {
    pub width: u32,
    pub height: u32,
    pub session: u64,
}

/// A file being written from the live camera.
struct Recorder {
    pipeline: RecordingPipeline,
    encoder: thread::JoinHandle<Result<()>>,
    stop: Arc<AtomicBool>,
    clock: RecordingClock,
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
///
/// Each preview frame is `width: u32le, height: u32le` followed by BGRA rows, so
/// the receiver needs no side channel to size itself.
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
        .frame_rate(30)
        .build()
        .with_context(|| format!("failed to open camera \"{device}\""))?;
    let first = capturer
        .next_frame(FIRST_FRAME)
        .with_context(|| format!("camera \"{device}\" produced no frames"))?;
    let geometry = CameraGeometry {
        width: first.desc().width,
        height: first.desc().height,
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

/// Begin writing the live camera to `dest`. The session must already be running.
pub fn attach_recorder(
    dest: PathBuf,
    fps: u32,
    clock: RecordingClock,
    track: TrackStart,
    pause: Arc<AtomicBool>,
) -> Result<()> {
    let held = slot().lock().map_err(|_| anyhow!("camera lock poisoned"))?;
    let running = held
        .as_ref()
        .ok_or_else(|| anyhow!("no camera is open to record"))?;

    let pipeline = RecordingPipeline::new(QUEUE_DEPTH);
    let stop = Arc::new(AtomicBool::new(false));
    let encoder = spawn_encoder_loop(
        EncoderConfig {
            width: running.geometry.width,
            height: running.geometry.height,
            fps,
            crop: None,
            output_path: dest,
            quality: RecordingQuality::default(),
        },
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
        clock,
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
            Err(_) => continue,
        };
        // Size is negotiated once; a short buffer must never reach the encoder.
        if frame.desc().width != geometry.width || frame.desc().height != geometry.height {
            continue;
        }
        packed = pack_rows(
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
                rec.pipeline.push(VideoFrame {
                    timestamp_us: rec.clock.effective_elapsed().as_micros() as u64,
                    width: geometry.width,
                    height: geometry.height,
                    data: packed.to_vec().into(),
                });
            }
        }
    }
    let (small, w, h) = downscale_bgra(packed, geometry.width, geometry.height, PREVIEW_MAX_DIM);
    let mut message = Vec::with_capacity(8 + small.len());
    message.extend_from_slice(&w.to_le_bytes());
    message.extend_from_slice(&h.to_le_bytes());
    message.extend_from_slice(&small);
    if let Ok(send) = sink.lock() {
        send(message);
    }
}

/// Copy rows into a tightly packed buffer.
///
/// Media Foundation pads rows to the driver's stride, and FFmpeg's `rawvideo`
/// demuxer expects `width * 4` per row with no gaps.
fn pack_rows(bytes: &[u8], stride: u32, width: u32, height: u32) -> Vec<u8> {
    let row_bytes = width as usize * 4;
    let stride = stride as usize;
    if stride == row_bytes {
        return bytes.to_vec();
    }
    let mut packed = Vec::with_capacity(row_bytes * height as usize);
    for row in 0..height as usize {
        let start = row * stride;
        let Some(line) = bytes.get(start..start + row_bytes) else {
            break;
        };
        packed.extend_from_slice(line);
    }
    packed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packing_a_frame_without_padding_copies_it_verbatim() {
        let bytes: Vec<u8> = (0..16u8).collect();
        assert_eq!(pack_rows(&bytes, 8, 2, 2), bytes);
    }

    #[test]
    fn packing_drops_the_padding_between_rows() {
        // 1px wide at a stride of 6: 2 padding bytes per row the encoder must not see.
        let bytes = vec![1, 2, 3, 4, 0xFF, 0xFF, 5, 6, 7, 8, 0xFF, 0xFF];
        assert_eq!(pack_rows(&bytes, 6, 1, 2), vec![1, 2, 3, 4, 5, 6, 7, 8]);
    }

    #[test]
    fn a_short_buffer_yields_the_rows_it_actually_has() {
        let bytes = vec![1, 2, 3, 4, 0xFF, 0xFF, 5, 6];
        assert_eq!(pack_rows(&bytes, 6, 1, 2), vec![1, 2, 3, 4]);
    }

    #[test]
    fn a_preview_frame_carries_its_own_dimensions() {
        let geometry = CameraGeometry {
            width: 2,
            height: 2,
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
