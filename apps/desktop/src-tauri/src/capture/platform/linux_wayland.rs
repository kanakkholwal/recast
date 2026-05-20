//! Wayland-native screen capture via xdg-desktop-portal + PipeWire.
//!
//! See `apps/desktop/docs/linux-native-recording.md` for the full design.
//! In short: we cannot read framebuffers directly on Wayland (compositor
//! security model), so the only legitimate path is to ask the portal for
//! a `ScreenCast` session, accept whatever PipeWire node ID it hands us,
//! and pull frames from that node.
//!
//! The portal handshake happens *before* the recording threads spawn,
//! inside `commands::recording::start_recording` — it has to, because the
//! portal returns the actual stream dimensions, and the encoder is
//! configured from those dimensions when the recording session is built.
//! The handshake stashes the resulting `PortalStream` in this module's
//! static slot; the capture thread (spawned later) picks it up.
//!
//! Lifecycle:
//!   1. Frontend clicks Record on a Wayland session.
//!   2. `start_recording` calls `acquire_portal_stream()` synchronously.
//!      A current-thread tokio runtime spins up, drives the ashpd portal
//!      flow (Create → Select → Start → OpenPipeWireRemote), and tears
//!      down. The user sees the system portal dialog and picks a source.
//!   3. The resulting `PortalStream` (fd + node id + size) is stashed.
//!   4. `recording::RecordingManager::start` builds a `CaptureTarget`
//!      using the portal-supplied dimensions (NOT xcap's) and spawns the
//!      capture thread.
//!   5. `create_capture_source(target)` runs on the capture thread, takes
//!      the stashed stream, and spawns a dedicated PipeWire main-loop
//!      thread that pushes BGRA frames into a crossbeam channel.
//!   6. `WaylandCaptureSource::capture_next` reads frames from that
//!      channel with the trait's timeout semantics.
//!   7. On drop, the stop flag flips, the PipeWire main loop quits, and
//!      the thread joins.

use std::os::fd::OwnedFd;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::OnceLock;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use parking_lot::Mutex;

use crate::capture::CaptureSource;
use crate::recording::CaptureTarget;

// -- Cross-thread handoff ----------------------------------------------------

/// Negotiated portal stream that survives between the
/// `start_recording` Tauri command (where it's created) and the capture
/// thread (where it's consumed). One recording is live at a time, so a
/// single global slot is enough; a Mutex<Option<…>> with take-on-consume
/// also catches the "we forgot to consume it" bug as a None at consume
/// time instead of a silent leak.
static PENDING_PORTAL_STREAM: OnceLock<Mutex<Option<PortalStream>>> = OnceLock::new();

fn pending_slot() -> &'static Mutex<Option<PortalStream>> {
    PENDING_PORTAL_STREAM.get_or_init(|| Mutex::new(None))
}

pub fn has_pending_stream() -> bool {
    pending_slot().lock().is_some()
}

pub fn stash_portal_stream(stream: PortalStream) {
    pending_slot().lock().replace(stream);
}

pub fn take_pending_stream() -> Option<PortalStream> {
    pending_slot().lock().take()
}

/// Negotiated portal stream metadata.
///
/// `fd` is the PipeWire daemon connection — opaque, just gets passed to
/// `Context::connect_fd`. `node_id` selects which node on that connection
/// is our screencast. `width`/`height` are the negotiated stream size.
pub struct PortalStream {
    pub fd: OwnedFd,
    pub node_id: u32,
    pub width: u32,
    pub height: u32,
}

// -- Portal handshake (sync wrapper around ashpd's async API) ---------------

/// Synchronously run the xdg-desktop-portal ScreenCast handshake.
///
/// Builds a current-thread tokio runtime, drives the ashpd flow to
/// completion (which blocks while the user interacts with the portal
/// dialog), and tears the runtime down. Returns `Err` if the portal is
/// unavailable, the user cancels, or the compositor refuses the stream.
///
/// Called from `commands::recording::start_recording`, which is itself a
/// sync Tauri command. That's fine: the user just clicked Record and is
/// expected to interact with the portal dialog right after; blocking the
/// command thread for the duration of that dialog is the correct UX.
pub fn acquire_portal_stream() -> Result<PortalStream> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("failed to build tokio runtime for portal handshake")?;
    rt.block_on(portal_handshake_async())
}

async fn portal_handshake_async() -> Result<PortalStream> {
    use ashpd::desktop::{
        screencast::{CursorMode, Screencast, SourceType},
        PersistMode,
    };

    let proxy = Screencast::new()
        .await
        .context("failed to connect to xdg-desktop-portal (is xdg-desktop-portal running?)")?;
    let session = proxy
        .create_session()
        .await
        .context("portal CreateSession failed")?;

    // CursorMode::Embedded: the compositor draws the cursor into the
    // captured frames. Simpler than maintaining a separate cursor track
    // on Wayland, and the editor can still composite a styled cursor on
    // top because we ALSO record cursor positions via our own tracker
    // (see crate::cursor) — so post-record cursor stylization still works.
    // Trade-off: the embedded cursor "ghost" is visible underneath the
    // editor's stylized cursor at export time. Acceptable for MVP; can
    // be flipped to CursorMode::Metadata in a later iteration to drop
    // the embedded cursor and rely solely on our own track.
    proxy
        .select_sources(
            &session,
            CursorMode::Embedded.into(),
            SourceType::Monitor | SourceType::Window,
            false,
            None,
            PersistMode::DoNot,
        )
        .await
        .context("portal SelectSources failed")?;

    // The portal dialog appears here. Blocks until the user accepts or
    // cancels. On cancel, `.response()` returns Err and we propagate.
    let response = proxy
        .start(&session, None)
        .await
        .context("portal Start (user dialog) failed")?
        .response()
        .context("user cancelled the portal source-selection dialog")?;

    let stream = response
        .streams()
        .first()
        .context("portal returned no streams (user picked nothing?)")?
        .clone();
    let node_id = stream.pipe_wire_node_id();
    let (width, height) = stream
        .size()
        .context("portal stream did not report a size")?;

    let fd = proxy
        .open_pipe_wire_remote(&session)
        .await
        .context("portal OpenPipeWireRemote failed")?;

    Ok(PortalStream {
        fd,
        node_id,
        width: width as u32,
        height: height as u32,
    })
}

// -- CaptureSource implementation -------------------------------------------

pub fn create_source(_target: &CaptureTarget) -> Result<Box<dyn CaptureSource>> {
    let stream = take_pending_stream().context(
        "no pre-negotiated portal stream available — start_recording \
         must call acquire_portal_stream() and stash_portal_stream() \
         before the capture thread spawns on Wayland",
    )?;
    let source = WaylandCaptureSource::new(stream)?;
    Ok(Box::new(source))
}

struct WaylandCaptureSource {
    // Single-reader bounded queue shared with the PipeWire callback
    // running on the pw thread. Capacity 2 lets one in-flight frame
    // overlap with one queued frame; on producer overflow we drop the
    // newest rather than block the compositor's sample loop.
    frames: Arc<crossbeam_queue::ArrayQueue<Vec<u8>>>,
    width: u32,
    height: u32,
    stop_flag: Arc<AtomicBool>,
    // Held in an Option so Drop can take + join without violating
    // ownership.
    thread_handle: Option<JoinHandle<()>>,
}

impl WaylandCaptureSource {
    fn new(stream: PortalStream) -> Result<Self> {
        let frames = Arc::new(crossbeam_queue::ArrayQueue::<Vec<u8>>::new(2));
        let stop_flag = Arc::new(AtomicBool::new(false));
        let width = stream.width;
        let height = stream.height;

        let frames_for_thread = frames.clone();
        let stop_for_thread = stop_flag.clone();
        let thread_handle = thread::Builder::new()
            .name("recast-pipewire".into())
            .spawn(move || {
                if let Err(e) =
                    pipewire_capture_loop(stream, width, height, frames_for_thread, stop_for_thread)
                {
                    log::error!("pipewire capture loop terminated: {e:#}");
                }
            })
            .context("failed to spawn pipewire capture thread")?;

        Ok(Self {
            frames,
            width,
            height,
            stop_flag,
            thread_handle: Some(thread_handle),
        })
    }
}

impl CaptureSource for WaylandCaptureSource {
    fn capture_next(&mut self, timeout: Duration) -> Result<Option<Vec<u8>>> {
        // Same shape as the DXGI path: poll non-blockingly and yield None
        // on no-frame so the pacer's drain loop can move on. We poll on a
        // tight schedule (10 ms) up to the requested timeout; PipeWire
        // delivers frames at the negotiated rate which is typically close
        // to our target fps anyway.
        let deadline = std::time::Instant::now() + timeout;
        loop {
            if let Some(frame) = self.frames.pop() {
                return Ok(Some(frame));
            }
            if std::time::Instant::now() >= deadline {
                return Ok(None);
            }
            // Bail early if the producer thread died.
            if self.stop_flag.load(Ordering::Acquire) {
                return Err(anyhow!("pipewire capture thread terminated"));
            }
            thread::sleep(Duration::from_millis(10));
        }
    }

    fn width(&self) -> u32 {
        self.width
    }

    fn height(&self) -> u32 {
        self.height
    }
}

impl Drop for WaylandCaptureSource {
    fn drop(&mut self) {
        // Signal stop. The pipewire thread polls this flag from a 100 ms
        // timer registered on the main loop and quits when it flips.
        self.stop_flag.store(true, Ordering::Release);
        if let Some(handle) = self.thread_handle.take() {
            // Best-effort join. If the thread is wedged in a pipewire
            // callback we don't want to hang the editor on shutdown.
            let _ = handle.join();
        }
    }
}

// -- PipeWire main loop ------------------------------------------------------

fn pipewire_capture_loop(
    stream: PortalStream,
    width: u32,
    height: u32,
    frame_queue: Arc<crossbeam_queue::ArrayQueue<Vec<u8>>>,
    stop_flag: Arc<AtomicBool>,
) -> Result<()> {
    // pipewire-rs 0.9 split the wrappers into a borrowed "view" type and
    // owning Rc/Box forms — the bare `MainLoop`/`Context`/`Stream` types no
    // longer have constructors. We use the Rc forms so the main loop can be
    // cloned into the stop-flag timer closure below; `StreamBox` is fine
    // for the stream itself because it lives entirely on this thread.
    use pipewire as pw;
    use pw::context::ContextRc;
    use pw::main_loop::MainLoopRc;
    use pw::properties::properties;
    use pw::stream::{StreamBox, StreamFlags};

    // RAII guard so any early return via `?` between here and the explicit
    // cleanup below still calls pw::deinit(). On the success path we
    // disable() the guard before the manual drop sequence so deinit runs
    // exactly once, after the wrappers are dropped in the required order.
    struct PwInitGuard {
        active: bool,
    }
    impl PwInitGuard {
        fn disable(&mut self) {
            self.active = false;
        }
    }
    impl Drop for PwInitGuard {
        fn drop(&mut self) {
            if self.active {
                pipewire::deinit();
            }
        }
    }

    pw::init();
    let mut pw_guard = PwInitGuard { active: true };

    let main_loop = MainLoopRc::new(None).context("failed to create pipewire main loop")?;
    let context = ContextRc::new(&main_loop, None).context("failed to create pipewire context")?;
    let core = context
        .connect_fd_rc(stream.fd, None)
        .context("failed to connect to pipewire daemon over portal fd")?;

    // The portal drives source selection and stream lifecycle, but our
    // node IS a regular pipewire node — give it sensible role properties
    // so other tools (e.g. `pw-top`) can identify it.
    let pw_stream = StreamBox::new(
        &core,
        "recast-screen",
        properties! {
            *pw::keys::MEDIA_TYPE => "Video",
            *pw::keys::MEDIA_CATEGORY => "Capture",
            *pw::keys::MEDIA_ROLE => "Screen",
        },
    )
    .context("failed to create pipewire stream")?;

    // F1 (audit Appendix A): without a `param_changed` handler, the stream
    // dimensions assumed by `process()` were the portal-reported ones —
    // but the compositor was free to negotiate a different size against
    // the Range we offered, silently dropping every frame on the
    // `slice.len() < total` check. We fix it two ways: (1) pin the size
    // in `build_format_param` so renegotiation cannot happen, and
    // (2) read the actually negotiated geometry here as a defence in
    // depth + diagnostic — if a future compositor honours the request
    // loosely, the log will say so and `process()` will still use the
    // right dims.
    #[derive(Clone, Copy)]
    struct NegotiatedFormat {
        width: u32,
        height: u32,
    }
    let negotiated: Arc<Mutex<Option<NegotiatedFormat>>> = Arc::new(Mutex::new(None));

    let portal_w: u32 = width;
    let portal_h: u32 = height;
    let nego_for_param = negotiated.clone();
    let nego_for_process = negotiated.clone();
    let queue_cb = frame_queue.clone();

    let _listener = pw_stream
        .add_local_listener_with_user_data(())
        .param_changed(move |_stream, _user_data, id, param| {
            // Stream emits several param kinds (Buffers, IO, Meta, ...);
            // only Format carries the geometry we care about.
            if id != pipewire::spa::param::ParamType::Format.as_raw() {
                return;
            }
            let Some(pod) = param else { return };
            let mut info = pipewire::spa::param::video::VideoInfoRaw::new();
            if info.parse(pod).is_err() {
                log::warn!(
                    "pipewire param_changed: failed to parse Format pod; \
                     keeping portal-reported {}x{} as the working geometry",
                    portal_w,
                    portal_h
                );
                return;
            }
            let size = info.size();
            if size.width != portal_w || size.height != portal_h {
                // With the size pinned in build_format_param this branch
                // should be unreachable. If it ever fires, the
                // compositor is renegotiating against our preference and
                // the encoder (configured for portal dims at this point)
                // will produce cropped or stretched output until the
                // next recording. Log loudly so the next iteration knows
                // to investigate the compositor's source caps.
                log::warn!(
                    "pipewire negotiated {}x{} differs from portal-reported {}x{} — \
                     encoder is already configured for the portal size; \
                     output will be cropped or stretched.",
                    size.width,
                    size.height,
                    portal_w,
                    portal_h
                );
            }
            log::info!(
                "pipewire stream format negotiated: {}x{} (format = {:?})",
                size.width,
                size.height,
                info.format()
            );
            *nego_for_param.lock() = Some(NegotiatedFormat {
                width: size.width,
                height: size.height,
            });
        })
        .process(move |stream, _user_data| {
            let Some(mut buffer) = stream.dequeue_buffer() else {
                return;
            };
            let datas = buffer.datas_mut();
            let Some(data) = datas.first_mut() else {
                return;
            };

            // Prefer the negotiated dims (set by param_changed above);
            // before the first negotiation completes, fall back to the
            // portal-reported size. With size pinned in build_format_param
            // these should always agree by the time frames flow.
            let (width, height) = {
                let g = nego_for_process.lock();
                g.as_ref()
                    .map(|n| (n.width, n.height))
                    .unwrap_or((portal_w, portal_h))
            };

            // The buffer's chunk metadata tells us the actual valid byte
            // count for this frame, which can be smaller than the mapped
            // region. `stride` is bytes-per-row including any compositor
            // padding; we always copy at width*4 because that's what the
            // encoder expects.
            let chunk = data.chunk();
            let stride = chunk.stride() as usize;
            let row_bytes = (width as usize) * 4;

            let Some(slice) = data.data() else { return };
            // If stride matches our expected row size, one big copy.
            // Otherwise copy row-by-row to drop the per-row padding.
            let frame: Vec<u8> = if stride == row_bytes {
                let total = row_bytes * (height as usize);
                if slice.len() < total {
                    return;
                }
                slice[..total].to_vec()
            } else if stride >= row_bytes {
                let mut out = Vec::with_capacity(row_bytes * (height as usize));
                for row in 0..(height as usize) {
                    let off = row * stride;
                    if off + row_bytes > slice.len() {
                        return;
                    }
                    out.extend_from_slice(&slice[off..off + row_bytes]);
                }
                out
            } else {
                // Stride smaller than expected row — should never happen,
                // means our negotiated width disagrees with the buffer.
                return;
            };

            // Best-effort enqueue. The consumer runs the pacer at fixed
            // fps and we'd rather drop a frame than block the pipewire
            // callback (which would back-pressure the compositor).
            let _ = queue_cb.push(frame);
        })
        .register()
        .context("failed to register pipewire stream listener")?;

    // Periodic timer to check the stop flag and quit the loop. PipeWire's
    // main loop only exits cleanly when called from inside its own
    // thread, so we can't just `quit()` from the consumer's drop — we
    // poll the flag here.
    let main_loop_for_timer = main_loop.clone();
    let stop_for_timer = stop_flag.clone();
    let timer = main_loop.loop_().add_timer(move |_| {
        if stop_for_timer.load(Ordering::Acquire) {
            main_loop_for_timer.quit();
        }
    });
    timer
        .update_timer(
            Some(Duration::from_millis(100)),
            Some(Duration::from_millis(100)),
        )
        .context("failed to arm pipewire stop-flag timer")?;

    let format_param_bytes = build_format_param(width, height);
    let format_pod = pipewire::spa::pod::Pod::from_bytes(&format_param_bytes)
        .context("failed to wrap format POD bytes")?;
    let mut params = [format_pod];

    pw_stream
        .connect(
            pipewire::spa::utils::Direction::Input,
            Some(stream.node_id),
            StreamFlags::AUTOCONNECT | StreamFlags::MAP_BUFFERS,
            &mut params,
        )
        .context("failed to connect pipewire stream to portal node")?;

    main_loop.run();

    // The Drop handler order matters: stream first, then core, then
    // context, then main_loop. pw::deinit must be the last call. We
    // disable the guard so its Drop impl doesn't double-deinit.
    pw_guard.disable();
    drop(_listener);
    drop(timer);
    drop(pw_stream);
    drop(core);
    drop(context);
    drop(main_loop);
    pw::deinit();

    Ok(())
}

// -- SPA POD format-param construction --------------------------------------

/// Build the EnumFormat POD that we send to pipewire as a stream-connect
/// parameter. This is the most version-sensitive part of the integration:
/// libspa POD layout is a binary protocol and minor pipewire-rs version
/// bumps occasionally rename helpers. The shape below mirrors the
/// upstream `pipewire-rs/examples/screencast.rs` example for 0.9.x.
///
/// We accept BGRA and BGRx as the only formats; the encoder downstream
/// expects packed BGRA at width*4 bytes per row. Compositors almost
/// always produce BGRA on Wayland — if we get an unexpected negotiation
/// the `process` callback's stride check will silently drop frames; we
/// can teach this builder more formats later if real-world devices need
/// it.
fn build_format_param(width: u32, height: u32) -> Vec<u8> {
    use pipewire::spa::param::format::{FormatProperties, MediaSubtype, MediaType};
    use pipewire::spa::param::video::VideoFormat;
    use pipewire::spa::param::ParamType;
    use pipewire::spa::pod::serialize::PodSerializer;
    use pipewire::spa::pod::{ChoiceValue, Object, Property, PropertyFlags, Value};
    use pipewire::spa::utils::{
        Choice, ChoiceEnum, ChoiceFlags, Fraction, Id, Rectangle, SpaTypes,
    };

    let object = Value::Object(Object {
        type_: SpaTypes::ObjectParamFormat.as_raw(),
        id: ParamType::EnumFormat.as_raw(),
        properties: vec![
            Property {
                key: FormatProperties::MediaType.as_raw(),
                flags: PropertyFlags::empty(),
                value: Value::Id(Id(MediaType::Video.as_raw())),
            },
            Property {
                key: FormatProperties::MediaSubtype.as_raw(),
                flags: PropertyFlags::empty(),
                value: Value::Id(Id(MediaSubtype::Raw.as_raw())),
            },
            Property {
                key: FormatProperties::VideoFormat.as_raw(),
                flags: PropertyFlags::empty(),
                value: Value::Choice(ChoiceValue::Id(Choice(
                    ChoiceFlags::empty(),
                    ChoiceEnum::Enum {
                        default: Id(VideoFormat::BGRA.as_raw()),
                        alternatives: vec![
                            Id(VideoFormat::BGRA.as_raw()),
                            Id(VideoFormat::BGRx.as_raw()),
                        ],
                    },
                ))),
            },
            Property {
                key: FormatProperties::VideoSize.as_raw(),
                flags: PropertyFlags::empty(),
                // Fixed (not Range): the portal already told us the
                // source dimensions, so we pin them here. With Range,
                // the compositor was free to negotiate a different
                // size — Audit F1 — which silently dropped every frame
                // on the slice.len() check downstream. Locking the size
                // means PipeWire either matches or errors out the stream
                // connection; both outcomes are observable, unlike the
                // silent failure mode.
                value: Value::Rectangle(Rectangle { width, height }),
            },
            Property {
                key: FormatProperties::VideoFramerate.as_raw(),
                flags: PropertyFlags::empty(),
                value: Value::Choice(ChoiceValue::Fraction(Choice(
                    ChoiceFlags::empty(),
                    ChoiceEnum::Range {
                        default: Fraction { num: 60, denom: 1 },
                        min: Fraction { num: 0, denom: 1 },
                        max: Fraction { num: 240, denom: 1 },
                    },
                ))),
            },
        ],
    });

    let mut buffer = std::io::Cursor::new(Vec::<u8>::new());
    PodSerializer::serialize(&mut buffer, &object)
        .expect("format pod serialization should not fail with valid inputs");
    buffer.into_inner()
}
