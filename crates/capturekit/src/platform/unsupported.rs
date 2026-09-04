use capturekit_core::{
    AudioDevice, AudioDeviceId, AudioDirection, Capabilities, CaptureError, Display,
    ExclusionSupport, Permission, PermissionKind, RegionCrop, Result, Target, Timestamp, Window,
};

use crate::backend::{AudioSource, FrameSource};
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
        camera_capture: false,
        window_enumeration: false,
        display_enumeration: false,
        region_crop: RegionCrop::OnHost,
        cursor_in_frame: false,
        cursor_samples: false,
        cursor_pointer: false,
        cursor_buttons: false,
        dirty_rects: false,
        audio_loopback: false,
        audio_loopback_gap_filling: false,
        audio_device_enumeration: false,
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

/// Audio devices are not enumerated on this platform yet.
/// Cameras are not enumerated on this platform yet.
pub(crate) fn cameras() -> Result<Vec<capturekit_core::Camera>> {
    Err(CaptureError::Unsupported {
        backend: "unsupported",
        operation: "enumerate cameras yet",
    })
}

pub(crate) fn audio_devices() -> Result<Vec<AudioDevice>> {
    Err(CaptureError::Unsupported {
        backend: BACKEND,
        operation: "enumerate audio devices yet",
    })
}

/// Audio capture is not implemented on this platform yet.
pub(crate) fn open_audio(
    _device: Option<&AudioDeviceId>,
    _direction: AudioDirection,
) -> Result<Box<dyn AudioSource>> {
    Err(CaptureError::Unsupported {
        backend: BACKEND,
        operation: "capture audio yet",
    })
}

pub(crate) fn open(_target: &Target, _opts: &OpenOptions) -> Result<Box<dyn FrameSource>> {
    Err(unsupported("capture on this platform"))
}

pub(crate) fn pointer_source() -> Result<Box<dyn crate::pointer::PointerSource>> {
    Err(unsupported("read the pointer on this platform"))
}
