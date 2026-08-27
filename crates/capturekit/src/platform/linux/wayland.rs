use core::time::Duration;
use std::os::fd::OwnedFd;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;

use ashpd::desktop::screencast::{CursorMode as PortalCursor, Screencast, SourceType};
use ashpd::desktop::PersistMode;
use capturekit_core::{
    CaptureError, ColorSpace, DirtyRects, Display, DisplayId, LostReason, PixelFormat, Rect, Result,
    Rotation, SourceDesc, Timestamp,
};
use pipewire::spa::param::format::{MediaSubtype, MediaType};
use pipewire::spa::param::video::{VideoFormat, VideoInfoRaw};
use pipewire::spa::pod::Pod;
use pipewire::spa::utils::Direction;
use pipewire::stream::{Stream, StreamFlags};
use pipewire::{context::Context, main_loop::MainLoop, properties::properties};

use crate::backend::{RawFrame, ScreenBackend};
use crate::platform::linux::now;
use crate::platform::OpenOptions;
use crate::shot::CursorMode;

pub(crate) const BACKEND: &str = "pipewire";

fn failed(message: impl Into<String>) -> CaptureError {
    CaptureError::backend(BACKEND, std::io::Error::other(message.into()))
}

/// What the portal handed back once the user consented.
struct Negotiated {
    node_id: u32,
    fd: OwnedFd,
    size: Option<(u32, u32)>,
}

/// Run the portal handshake to completion.
///
/// There is no way to ask the portal what it would allow: consent is granted per
/// session, by the user, in a dialog, and the answer arrives with the stream.
/// That is why this blocks, and why `permission()` reports `NotDetermined`
/// rather than pretending to know.
fn negotiate(cursor: CursorMode) -> Result<Negotiated> {
    async_std::task::block_on(async {
        let proxy = Screencast::new().await.map_err(portal_error)?;
        let session = proxy.create_session().await.map_err(portal_error)?;
        proxy
            .select_sources(
                &session,
                match cursor {
                    CursorMode::Include => PortalCursor::Embedded,
                    CursorMode::Exclude => PortalCursor::Hidden,
                },
                SourceType::Monitor | SourceType::Window,
                false,
                None,
                PersistMode::DoNot,
            )
            .await
            .map_err(portal_error)?;

        let response = proxy
            .start(&session, None)
            .await
            .map_err(portal_error)?
            .response()
            .map_err(portal_error)?;
        let stream = response
            .streams()
            .first()
            .ok_or_else(|| failed("the portal granted no streams"))?;
        let node_id = stream.pipe_wire_node_id();
        let size = stream.size().map(|(w, h)| (w as u32, h as u32));
        let fd = proxy
            .open_pipe_wire_remote(&session)
            .await
            .map_err(portal_error)?;
        Ok(Negotiated { node_id, fd, size })
    })
}

/// A cancelled dialog is a refusal, not a fault.
fn portal_error(error: ashpd::Error) -> CaptureError {
    match error {
        ashpd::Error::Response(ashpd::desktop::ResponseError::Cancelled) => {
            CaptureError::PermissionDenied(capturekit_core::PermissionKind::Screen)
        }
        other => failed(other.to_string()),
    }
}

/// One delivered frame, copied out of the PipeWire buffer.
#[derive(Default)]
struct Delivered {
    bytes: Vec<u8>,
    stride: u32,
    width: u32,
    height: u32,
    pts: Timestamp,
    sequence: u64,
}

#[derive(Default)]
struct FrameSlot {
    frame: Mutex<Delivered>,
    arrived: Condvar,
    /// Set when the stream ends before the consumer stops reading it.
    ended: AtomicBool,
}

/// The portal does not report per-monitor geometry, so a Wayland session offers
/// one nominal display standing for "whatever the user picks in the dialog".
///
/// Enumerating real outputs would need `wlr-output-management` or a compositor
/// protocol, and none is portable. Inventing entries the portal will not honour
/// would be worse than one honest placeholder.
pub(crate) fn displays() -> Result<Vec<Display>> {
    Ok(vec![Display {
        id: DisplayId(0),
        name: "Selected by the desktop portal".into(),
        bounds: Rect::from_size(0, 0),
        scale_factor: 1.0,
        refresh_hz: None,
        is_primary: true,
        rotation: Rotation::None,
    }])
}

/// Capture through xdg-desktop-portal and PipeWire.
pub(crate) struct PortalSource {
    slot: Arc<FrameSlot>,
    quit: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    desc: SourceDesc,
    last_sequence: u64,
    current: Vec<u8>,
}

impl PortalSource {
    pub(crate) fn open(
        _target: &capturekit_core::Target,
        opts: &OpenOptions,
    ) -> Result<Self> {
        let negotiated = negotiate(opts.cursor)?;
        let slot = Arc::new(FrameSlot::default());
        let quit = Arc::new(AtomicBool::new(false));

        let (ready_tx, ready_rx) = std::sync::mpsc::channel::<core::result::Result<(), String>>();
        let thread = std::thread::Builder::new()
            .name("capturekit-pipewire".into())
            .spawn({
                let slot = Arc::clone(&slot);
                let quit = Arc::clone(&quit);
                let node_id = negotiated.node_id;
                let fd = negotiated.fd;
                move || {
                    let outcome = run_stream(fd, node_id, &slot, &quit);
                    let _ = ready_tx.send(outcome.clone());
                    if let Err(message) = outcome {
                        log::error!("pipewire capture ended: {message}");
                    }
                    slot.ended.store(true, Ordering::Release);
                    slot.arrived.notify_all();
                }
            })
            .map_err(|error| CaptureError::backend(BACKEND, error))?;

        // The loop reports only a setup failure before it starts running, so a
        // timeout here means it got as far as running, which is success.
        if let Ok(Err(message)) = ready_rx.recv_timeout(Duration::from_secs(5)) {
            return Err(failed(message));
        }

        let (width, height) = negotiated.size.unwrap_or((0, 0));
        Ok(Self {
            slot,
            quit,
            thread: Some(thread),
            desc: SourceDesc {
                width,
                height,
                format: PixelFormat::Bgra8,
                color_space: ColorSpace::SRGB,
                rotation: Rotation::None,
                scale_factor: 1.0,
                frame_rate: opts.frame_rate,
                backend: BACKEND,
            },
            last_sequence: 0,
            current: Vec::new(),
        })
    }
}

/// Drive a PipeWire stream on this thread until asked to quit.
fn run_stream(
    fd: OwnedFd,
    node_id: u32,
    slot: &Arc<FrameSlot>,
    quit: &Arc<AtomicBool>,
) -> core::result::Result<(), String> {
    pipewire::init();
    let main_loop = MainLoop::new(None).map_err(|e| e.to_string())?;
    let context = Context::new(&main_loop).map_err(|e| e.to_string())?;
    let core = context
        .connect_fd(fd, None)
        .map_err(|e| format!("connecting to the portal's PipeWire remote: {e}"))?;

    let stream = Stream::new(
        &core,
        "capturekit",
        properties! {
            *pipewire::keys::MEDIA_TYPE => "Video",
            *pipewire::keys::MEDIA_CATEGORY => "Capture",
            *pipewire::keys::MEDIA_ROLE => "Screen",
        },
    )
    .map_err(|e| e.to_string())?;

    let format = Arc::new(Mutex::new(VideoInfoRaw::default()));
    let _listener = stream
        .add_local_listener_with_user_data(())
        .param_changed({
            let format = Arc::clone(&format);
            move |_stream, (), id, param| {
                let Some(param) = param else { return };
                if id != pipewire::spa::param::ParamType::Format.as_raw() {
                    return;
                }
                let Ok((media_type, media_subtype)) =
                    pipewire::spa::param::format_utils::parse_format(param)
                else {
                    return;
                };
                if media_type != MediaType::Video || media_subtype != MediaSubtype::Raw {
                    return;
                }
                if let Ok(mut info) = format.lock() {
                    let _ = info.parse(param);
                }
            }
        })
        .process({
            let slot = Arc::clone(slot);
            let format = Arc::clone(&format);
            move |stream, ()| {
                let Some(mut buffer) = stream.dequeue_buffer() else {
                    return;
                };
                let Some(data) = buffer.datas_mut().first_mut() else {
                    return;
                };
                let stride = data.chunk().stride().max(0) as u32;
                let Some(bytes) = data.data() else { return };
                let Ok(info) = format.lock() else { return };
                let (width, height) = (info.size().width, info.size().height);
                // Only the packed 32-bit layouts are handled; a compositor that
                // negotiated planar YUV would be misread as BGRA otherwise.
                if !matches!(
                    info.format(),
                    VideoFormat::BGRx | VideoFormat::BGRA | VideoFormat::RGBx | VideoFormat::RGBA
                ) || stride == 0
                    || height == 0
                {
                    return;
                }
                let len = (stride as usize).saturating_mul(height as usize);
                if bytes.len() < len {
                    return;
                }
                if let Ok(mut frame) = slot.frame.lock() {
                    frame.bytes.clear();
                    frame.bytes.extend_from_slice(&bytes[..len]);
                    frame.stride = stride;
                    frame.width = width;
                    frame.height = height;
                    frame.pts = now();
                    frame.sequence = frame.sequence.wrapping_add(1);
                    slot.arrived.notify_all();
                }
            }
        })
        .register()
        .map_err(|e| e.to_string())?;

    let pod_bytes = video_params()?;
    let pod = Pod::from_bytes(&pod_bytes).ok_or("the video format pod did not parse")?;
    stream
        .connect(
            Direction::Input,
            Some(node_id),
            StreamFlags::AUTOCONNECT | StreamFlags::MAP_BUFFERS,
            &mut [pod],
        )
        .map_err(|e| e.to_string())?;

    // Poll the quit flag from inside the loop rather than from outside it: the
    // PipeWire main loop is not safe to signal from another thread.
    let loop_ref = main_loop.loop_();
    let _timer = {
        let weak = main_loop.downgrade();
        let quit = Arc::clone(quit);
        let timer = loop_ref.add_timer(move |_| {
            if quit.load(Ordering::Acquire) {
                if let Some(main_loop) = weak.upgrade() {
                    main_loop.quit();
                }
            }
        });
        timer
            .update_timer(
                Some(Duration::from_millis(50)),
                Some(Duration::from_millis(50)),
            )
            .into_result()
            .map_err(|e| e.to_string())?;
        timer
    };

    main_loop.run();
    Ok(())
}

/// Ask for any packed 32-bit layout, at any size the compositor offers.
///
/// Offered as a choice rather than a demand: a compositor that cannot give BGRx
/// picks another from the list, and one that cannot match the size range refuses
/// the connection outright instead of sending frames nothing here can read.
fn video_params() -> core::result::Result<Vec<u8>, String> {
    use pipewire::spa::param::format::FormatProperties;
    use pipewire::spa::param::ParamType;
    use pipewire::spa::pod::serialize::PodSerializer;
    use pipewire::spa::pod::{object, property, Value};
    use pipewire::spa::utils::{Fraction, Rectangle, SpaTypes};

    let format = object! {
        SpaTypes::ObjectParamFormat,
        ParamType::EnumFormat,
        property!(FormatProperties::MediaType, Id, MediaType::Video),
        property!(FormatProperties::MediaSubtype, Id, MediaSubtype::Raw),
        property!(
            FormatProperties::VideoFormat,
            Choice,
            Enum,
            Id,
            VideoFormat::BGRx,
            VideoFormat::BGRx,
            VideoFormat::BGRA,
            VideoFormat::RGBx,
            VideoFormat::RGBA
        ),
        property!(
            FormatProperties::VideoSize,
            Choice,
            Range,
            Rectangle,
            Rectangle {
                width: 1920,
                height: 1080
            },
            Rectangle {
                width: 1,
                height: 1
            },
            Rectangle {
                width: 8192,
                height: 8192
            }
        ),
        property!(
            FormatProperties::VideoFramerate,
            Choice,
            Range,
            Fraction,
            Fraction { num: 60, denom: 1 },
            Fraction { num: 0, denom: 1 },
            Fraction {
                num: 1000,
                denom: 1
            }
        ),
    };

    PodSerializer::serialize(std::io::Cursor::new(Vec::new()), &Value::Object(format))
        .map(|(cursor, _)| cursor.into_inner())
        .map_err(|error| format!("building the video format pod: {error}"))
}

impl Drop for PortalSource {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

impl ScreenBackend for PortalSource {
    fn describe(&self) -> &SourceDesc {
        &self.desc
    }

    /// Always `None`: the portal hands over whatever the user picked, whole. A
    /// region is cropped on the host, which the shared code does for us.
    fn region(&self) -> Option<Rect> {
        None
    }

    fn next_frame(&mut self, timeout: Duration) -> Result<RawFrame<'_>> {
        let mut slot = self
            .slot
            .frame
            .lock()
            .map_err(|_| CaptureError::Lost(LostReason::AccessLost))?;
        while slot.sequence == self.last_sequence {
            if self.slot.ended.load(Ordering::Acquire) {
                return Err(CaptureError::Lost(LostReason::AccessLost));
            }
            let (next, waited) = self
                .slot
                .arrived
                .wait_timeout(slot, timeout)
                .map_err(|_| CaptureError::Lost(LostReason::AccessLost))?;
            slot = next;
            if waited.timed_out() && slot.sequence == self.last_sequence {
                return Err(CaptureError::Timeout(timeout));
            }
        }
        self.last_sequence = slot.sequence;
        // The portal reports its size before the stream negotiates one, and a
        // compositor may renegotiate mid-stream, so the frame is the authority.
        self.desc.width = slot.width;
        self.desc.height = slot.height;
        let pts = slot.pts;
        let stride = slot.stride;
        core::mem::swap(&mut self.current, &mut slot.bytes);
        drop(slot);

        Ok(RawFrame {
            pts,
            bytes: &self.current,
            stride,
            dirty: DirtyRects::unknown(),
        })
    }

    fn stop(&mut self) -> Result<()> {
        self.quit.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        Ok(())
    }
}
