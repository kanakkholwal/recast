use capturekit_core::{
    CaptureError, Display, Permission, PermissionKind, Result, Target, Timestamp, Window,
};

use crate::backend::ScreenBackend;
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

pub(crate) fn open(_target: &Target, _opts: &OpenOptions) -> Result<Box<dyn ScreenBackend>> {
    Err(unsupported("capture on this platform"))
}
