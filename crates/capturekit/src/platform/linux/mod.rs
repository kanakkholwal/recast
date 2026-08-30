mod pointer;
mod v4l2;
mod x11;

#[cfg(feature = "pipewire-audio")]
mod audio;

#[cfg(feature = "wayland")]
mod wayland;

use capturekit_core::{
    AudioDevice, AudioDeviceId, AudioDirection, Capabilities, CaptureError, Display,
    ExclusionSupport, Permission, PermissionKind, RegionCrop, Result, Target, Timestamp, Window,
};

use crate::backend::{AudioSource, FrameSource};
use crate::platform::OpenOptions;

/// Which display server this session actually runs on.
///
/// `WAYLAND_DISPLAY` is checked first because XWayland sets `DISPLAY` too. That
/// ordering matters more here than anywhere else: X11 capture under XWayland
/// succeeds and returns a black or XWayland-only image, which looks like a
/// capture bug rather than the wrong backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Session {
    Wayland,
    X11,
    None,
}

fn session() -> Session {
    if std::env::var_os("WAYLAND_DISPLAY").is_some() {
        Session::Wayland
    } else if std::env::var_os("DISPLAY").is_some() {
        Session::X11
    } else {
        Session::None
    }
}

fn no_session() -> CaptureError {
    CaptureError::Unsupported {
        backend: "linux",
        operation: "capture without a WAYLAND_DISPLAY or DISPLAY session",
    }
}

#[cfg(not(feature = "wayland"))]
fn no_wayland() -> CaptureError {
    CaptureError::Unsupported {
        backend: "linux",
        operation: "capture a Wayland session; this build has the wayland feature off",
    }
}

pub(crate) fn displays() -> Result<Vec<Display>> {
    match session() {
        #[cfg(feature = "wayland")]
        Session::Wayland => wayland::displays(),
        #[cfg(not(feature = "wayland"))]
        Session::Wayland => Err(no_wayland()),
        Session::X11 => x11::displays(),
        Session::None => Err(no_session()),
    }
}

pub(crate) fn windows() -> Result<Vec<Window>> {
    match session() {
        // The portal picks the window through its own dialog and lets no client enumerate them, which is its whole point.
        #[cfg(feature = "wayland")]
        Session::Wayland => Ok(Vec::new()),
        #[cfg(not(feature = "wayland"))]
        Session::Wayland => Err(no_wayland()),
        Session::X11 => x11::windows(),
        Session::None => Err(no_session()),
    }
}

/// What this session can do, reported as data so callers branch on the answer
/// rather than on `cfg`.
///
/// The two Linux sessions differ more from each other than macOS differs from
/// Windows, so this is per-session rather than per-OS.
pub(crate) fn capabilities() -> Capabilities {
    match session() {
        Session::Wayland => Capabilities {
            backend: "pipewire",
            // The compositor owns the frame and there is no portal API for excluding a window, so a request is refused, not dropped.
            exclusion: ExclusionSupport::None,
            window_capture: true,
            camera_capture: true,
            // The portal runs its own picker; a client never sees the list.
            window_enumeration: false,
            display_enumeration: false,
            region_crop: RegionCrop::OnHost,
            cursor_in_frame: true,
            cursor_samples: false,
            cursor_pointer: false,
            cursor_buttons: false,
            dirty_rects: false,
            audio_loopback: true,
            audio_device_enumeration: cfg!(feature = "pipewire-audio"),
        },
        _ => Capabilities {
            backend: "x11",
            // X11 has no notion of hiding a window from a `GetImage` of the root.
            exclusion: ExclusionSupport::None,
            window_capture: true,
            camera_capture: true,
            window_enumeration: true,
            display_enumeration: true,
            // `GetImage` crops server-side, so nothing outside the rectangle crosses the socket.
            region_crop: RegionCrop::DuringAcquisition,
            // The root window image never contains the pointer; XFixes reports it alongside instead.
            cursor_in_frame: false,
            cursor_samples: true,
            cursor_pointer: true,
            cursor_buttons: true,
            dirty_rects: false,
            audio_loopback: true,
            audio_device_enumeration: cfg!(feature = "pipewire-audio"),
        },
    }
}

pub(crate) fn permission(kind: PermissionKind) -> Permission {
    match (kind, session()) {
        // The portal asks every time a session opens and exposes no standing answer, so the honest report is 'you will be asked'.
        (PermissionKind::Screen, Session::Wayland) => Permission::NotDetermined,
        (PermissionKind::Screen, Session::X11) => Permission::NotRequired,
        (PermissionKind::Camera, _) => v4l2::permission(),
        _ => Permission::NotDetermined,
    }
}

/// There is nothing to request ahead of time on either session: X11 gates
/// nothing, and the portal prompts as part of opening the capture itself.
pub(crate) fn request_permission(kind: PermissionKind) -> Permission {
    permission(kind)
}

/// `CLOCK_MONOTONIC`, which is what PipeWire stamps buffers with and the only
/// clock X11 capture can be timed against.
pub(crate) fn now() -> Timestamp {
    let mut spec = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: a well-formed timespec, and CLOCK_MONOTONIC is always available.
    if unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut spec) } != 0 {
        return Timestamp::ZERO;
    }
    Timestamp::from_nanos(spec.tv_sec.saturating_mul(1_000_000_000) + spec.tv_nsec)
}

/// Stop a PipeWire main loop from outside it.
///
/// The loop is not safe to signal from another thread, so the flag is polled
/// from a timer inside it instead. `interval` is both the first firing and the
/// repeat, so a caller that only wants a deadline passes the deadline.
#[cfg(feature = "pipewire-audio")]
pub(crate) fn quit_timer<'l>(
    main_loop: &'l pipewire::main_loop::MainLoopRc,
    flag: &std::sync::Arc<std::sync::atomic::AtomicBool>,
    interval: core::time::Duration,
) -> core::result::Result<pipewire::loop_::TimerSource<'l>, String> {
    use std::sync::atomic::Ordering;

    let weak = main_loop.downgrade();
    let flag = std::sync::Arc::clone(flag);
    let timer = main_loop.loop_().add_timer(move |_| {
        if flag.load(Ordering::Acquire) {
            if let Some(main_loop) = weak.upgrade() {
                main_loop.quit();
            }
        }
    });
    timer
        .update_timer(Some(interval), Some(interval))
        .into_result()
        .map_err(|e| e.to_string())?;
    Ok(timer)
}

pub(crate) fn cameras() -> Result<Vec<capturekit_core::Camera>> {
    v4l2::cameras()
}

#[cfg(feature = "pipewire-audio")]
pub(crate) fn audio_devices() -> Result<Vec<AudioDevice>> {
    audio::devices()
}

/// Built without PipeWire, which is the only audio backend this platform has.
#[cfg(not(feature = "pipewire-audio"))]
pub(crate) fn audio_devices() -> Result<Vec<AudioDevice>> {
    Err(no_pipewire_audio())
}

#[cfg(feature = "pipewire-audio")]
pub(crate) fn open_audio(
    device: Option<&AudioDeviceId>,
    direction: AudioDirection,
) -> Result<Box<dyn AudioSource>> {
    Ok(Box::new(audio::PipewireAudioSource::open(
        device, direction,
    )?))
}

#[cfg(not(feature = "pipewire-audio"))]
pub(crate) fn open_audio(
    _device: Option<&AudioDeviceId>,
    _direction: AudioDirection,
) -> Result<Box<dyn AudioSource>> {
    Err(no_pipewire_audio())
}

#[cfg(not(feature = "pipewire-audio"))]
fn no_pipewire_audio() -> CaptureError {
    CaptureError::Unsupported {
        backend: "linux",
        operation: "capture audio without the pipewire-audio feature",
    }
}

pub(crate) fn open(target: &Target, opts: &OpenOptions) -> Result<Box<dyn FrameSource>> {
    // Ahead of the session check: a camera is a device node, not a surface.
    if let Target::Camera(id) = target {
        return Ok(Box::new(v4l2::V4l2CameraSource::open(id, opts)?));
    }
    match session() {
        #[cfg(feature = "wayland")]
        Session::Wayland => Ok(Box::new(wayland::PortalSource::open(target, opts)?)),
        #[cfg(not(feature = "wayland"))]
        Session::Wayland => Err(no_wayland()),
        Session::X11 => match target {
            Target::Display(id) => Ok(Box::new(x11::X11Source::open_display(*id, opts)?)),
            Target::Region { display, rect } => {
                let opts = OpenOptions {
                    region: Some(*rect),
                    ..opts.clone()
                };
                Ok(Box::new(x11::X11Source::open_display(*display, &opts)?))
            }
            Target::Window(id) => Ok(Box::new(x11::X11Source::open_window(*id, opts)?)),
            // Taken above, before the display server was ever consulted.
            Target::Camera(id) => Ok(Box::new(v4l2::V4l2CameraSource::open(id, opts)?)),
        },
        Session::None => Err(no_session()),
    }
}

/// The pointer reader for this session, which Wayland does not have.
pub(crate) fn pointer_source() -> Result<Box<dyn crate::pointer::PointerSource>> {
    match session() {
        Session::X11 => pointer::source(),
        Session::Wayland => Err(pointer::unavailable()),
        Session::None => Err(no_session()),
    }
}
