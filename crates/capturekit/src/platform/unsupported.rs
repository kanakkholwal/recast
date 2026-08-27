use capturekit_core::{
    Capabilities, CaptureError, Display, ExclusionSupport, Permission, PermissionKind, RegionCrop,
    Result, Target, Timestamp, Window,
};

use crate::backend::FrameSource;
use crate::platform::OpenOptions;

const BACKEND: &str = "unsupported";

/// Every entry point fails the same way, so a target capturekit has not been
/// ported to says so plainly instead of failing to build.
fn unsupported(operation: &'static str) -> CaptureError {
    CaptureError::Unsupported {
        backend: BACKEND,
        operation,
    }
}

pub(crate) fn capabilities() -> Capabilities {
    Capabilities {
        backend: BACKEND,
        exclusion: ExclusionSupport::None,
        window_capture: false,
        window_enumeration: false,
        display_enumeration: false,
        region_crop: RegionCrop::OnHost,
        cursor_in_frame: false,
        cursor_samples: false,
        dirty_rects: false,
        audio_loopback: false,
    }
}

pub(crate) fn displays() -> Result<Vec<Display>> {
    Err(unsupported("enumerate displays on this platform"))
}

pub(crate) fn windows() -> Result<Vec<Window>> {
    Err(unsupported("enumerate windows on this platform"))
}

pub(crate) fn permission(_kind: PermissionKind) -> Permission {
    Permission::Denied
}

pub(crate) fn request_permission(_kind: PermissionKind) -> Permission {
    Permission::Denied
}

pub(crate) fn now() -> Timestamp {
    Timestamp::ZERO
}

pub(crate) fn open(_target: &Target, _opts: &OpenOptions) -> Result<Box<dyn FrameSource>> {
    Err(unsupported("capture on this platform"))
}
