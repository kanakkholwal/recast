use core::time::Duration;
use std::os::fd::OwnedFd;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use ashpd::desktop::screencast::{CursorMode as PortalCursor, Screencast, SourceType};
use ashpd::desktop::PersistMode;
use capturekit_core::{
    CaptureError, ColorSpace, DirtyRects, Display, DisplayId, PixelFormat, Rect, Result, Rotation,
    SourceDesc,
};
use pipewire::spa::param::format::{MediaSubtype, MediaType};
use pipewire::spa::param::video::{VideoFormat, VideoInfoRaw};
use pipewire::spa::pod::Pod;
use pipewire::spa::utils::Direction;
use pipewire::stream::{StreamFlags, StreamRc};
use pipewire::{context::ContextRc, main_loop::MainLoopRc, properties::properties};

use crate::backend::{FrameSource, RawFrame};
use crate::deliver::{Delivered, FrameSlot};
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

/// Runs the portal handshake to completion, which is why it blocks.
/// There is no way to ask the portal what it would allow: consent is granted per session in a dialog, and the answer arrives with the stream.
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

/// The portal reports no per-monitor geometry, so a Wayland session offers one nominal display standing for whatever the user picks in the dialog.
/// Enumerating real outputs needs a compositor protocol and none is portable; inventing entries the portal will not honour would be worse than one honest placeholder.
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
    seen: u64,
    current: Vec<u8>,
}

impl PortalSource {
    pub(crate) fn open(_target: &capturekit_core::Target, opts: &OpenOptions) -> Result<Self> {
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
                    slot.end();
                }
            })
            .map_err(|error| CaptureError::backend(BACKEND, error))?;

        // The loop reports only a setup failure before running, so a timeout here means it got as far as running.
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
                frame_rate: opts.frame_rate(),
                backend: BACKEND,
            },
            seen: 0,
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
    // The owned handles are the `*Rc` types; the bare `MainLoop`, `Context` and `Stream` are the borrowed views.
    let main_loop = MainLoopRc::new(None).map_err(|e| e.to_string())?;
    let context = ContextRc::new(&main_loop, None).map_err(|e| e.to_string())?;
    let core = context
        .connect_fd_rc(fd, None)
        .map_err(|e| format!("connecting to the portal's PipeWire remote: {e}"))?;

    // Takes the core by value and keeps it alive for the stream's lifetime.
    let stream = StreamRc::new(
        core,
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
                // Only packed 32-bit layouts are handled; a compositor negotiating planar YUV would be misread as BGRA.
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
                slot.publish(
                    Delivered {
                        pts: now(),
                        stride,
                        width,
                        height,
                    },
                    &bytes[..len],
                );
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

    // Poll the quit flag inside the loop: the PipeWire main loop isn't safe to signal from another thread.
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

/// Asks for any packed 32-bit layout at any size the compositor offers, as a choice rather than a demand.
/// One that cannot give BGRx picks another from the list, and one that cannot match the size range refuses outright instead of sending frames nothing here can read.
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

impl FrameSource for PortalSource {
    fn describe(&self) -> &SourceDesc {
        &self.desc
    }

    /// Always `None`: the portal hands over whatever the user picked, whole. A
    /// region is cropped on the host, which the shared code does for us.
    fn region(&self) -> Option<Rect> {
        None
    }

    fn next_frame(&mut self, timeout: Duration) -> Result<RawFrame<'_>> {
        let meta = self.slot.take(timeout, &mut self.seen, &mut self.current)?;
        // The portal reports size before the stream negotiates one, and a compositor may renegotiate, so the frame is the authority.
        self.desc.width = meta.width;
        self.desc.height = meta.height;
        Ok(RawFrame {
            pts: meta.pts,
            bytes: &self.current,
            stride: meta.stride,
            dirty: DirtyRects::unknown(),
            cursor: None,
            gpu: None,
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
