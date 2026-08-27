mod x11;

#[cfg(feature = "wayland")]
mod wayland;

use capturekit_core::{
    CaptureError, Display, Permission, PermissionKind, Result, Target, Timestamp, Window,
};

use crate::backend::ScreenBackend;
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
        // The portal picks the window itself, through its own dialog, and does
        // not let a client enumerate them. That is the whole point of it.
        #[cfg(feature = "wayland")]
        Session::Wayland => Ok(Vec::new()),
        #[cfg(not(feature = "wayland"))]
        Session::Wayland => Err(no_wayland()),
        Session::X11 => x11::windows(),
        Session::None => Err(no_session()),
    }
}

pub(crate) fn permission(kind: PermissionKind) -> Permission {
    match (kind, session()) {
        // The portal asks every time a session is opened, and there is no way to
        // query a standing answer, so the honest report is "you will be asked".
        (PermissionKind::Screen, Session::Wayland) => Permission::NotDetermined,
        (PermissionKind::Screen, Session::X11) => Permission::NotRequired,
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

pub(crate) fn open(target: &Target, opts: &OpenOptions) -> Result<Box<dyn ScreenBackend>> {
    if let Target::Camera(_) = target {
        return Err(CaptureError::Unsupported {
            backend: "linux",
            operation: "capture a camera yet",
        });
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
            Target::Camera(_) => Err(no_session()),
        },
        Session::None => Err(no_session()),
    }
}
