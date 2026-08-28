mod audio;
mod camera;
mod content;
mod coreaudio;
mod mic;
mod pointer;
mod sample;
mod stream;

use capturekit_core::{
    AudioDevice, AudioDeviceId, AudioDirection, Capabilities, CaptureError, ExclusionSupport,
    Permission, PermissionKind, RegionCrop, Result, Target, Timestamp,
};
use objc2_core_graphics::{CGPreflightScreenCaptureAccess, CGRequestScreenCaptureAccess};

use crate::backend::{AudioSource, FrameSource};
use crate::platform::OpenOptions;

pub(crate) use content::{displays, windows};
pub(crate) use pointer::source as pointer_source;

/// What this platform can do, reported as data so callers branch on the answer
/// rather than on `cfg`.
pub(crate) fn capabilities() -> Capabilities {
    Capabilities {
        backend: content::BACKEND,
        // `SCContentFilter` takes an exclusion list of any windows at all, which
        // is the only one of the three that can hide a stranger's window.
        exclusion: ExclusionSupport::AnyWindow,
        window_capture: true,
        camera_capture: true,
        window_enumeration: true,
        display_enumeration: true,
        region_crop: RegionCrop::DuringAcquisition,
        cursor_in_frame: true,
        // Position and shape come from a separate CoreGraphics call, not with
        // the sample buffer, so they are not on the frame clock yet.
        cursor_samples: false,
        cursor_pointer: true,
        cursor_buttons: true,
        dirty_rects: false,
        audio_loopback: true,
        // CoreAudio lists them and AVFoundation opens any of them by UID.
        audio_device_enumeration: true,
    }
}

/// Screen recording is gated by TCC; the camera and microphone by their own
/// prompts, which capturekit does not drive yet.
pub(crate) fn permission(kind: PermissionKind) -> Permission {
    match kind {
        PermissionKind::Screen => {
            if CGPreflightScreenCaptureAccess() {
                Permission::Granted
            } else {
                // TCC does not distinguish "never asked" from "refused" through
                // this call, and asking again is harmless when it is the former.
                Permission::NotDetermined
            }
        }
        _ => Permission::NotDetermined,
    }
}

/// Ask TCC for screen recording.
///
/// The prompt appears once per application; afterwards this returns the standing
/// answer without showing anything, which is why a `Denied` result has to send
/// the user to System Settings rather than prompting again.
pub(crate) fn request_permission(kind: PermissionKind) -> Permission {
    match kind {
        PermissionKind::Screen => {
            if CGRequestScreenCaptureAccess() {
                Permission::Granted
            } else {
                Permission::Denied
            }
        }
        _ => Permission::NotDetermined,
    }
}

/// The current instant on the host time clock, which is what ScreenCaptureKit
/// stamps sample buffers with.
pub(crate) fn now() -> Timestamp {
    let time = unsafe { objc2_core_media::CMClock::host_time_clock().time() };
    match time.timescale {
        0 => Timestamp::ZERO,
        scale => Timestamp::from_ticks(time.value, i64::from(scale)),
    }
}

pub(crate) fn audio_devices() -> Result<Vec<AudioDevice>> {
    coreaudio::devices()
}

/// System audio through ScreenCaptureKit, inputs through AVFoundation.
///
/// ScreenCaptureKit is the only way to tap the output mix without a virtual
/// driver, but it can capture only the DEFAULT input and refuses any other name.
/// Inputs therefore go through AVFoundation, which opens the device the user
/// picked and takes the Microphone grant rather than the Screen Recording one.
pub(crate) fn open_audio(
    device: Option<&AudioDeviceId>,
    direction: AudioDirection,
) -> Result<Box<dyn AudioSource>> {
    if direction == AudioDirection::Input {
        return Ok(Box::new(mic::AvfMicSource::open(device)?));
    }
    Ok(Box::new(audio::SckAudioSource::open(device, direction)?))
}

pub(crate) fn cameras() -> Result<Vec<capturekit_core::Camera>> {
    camera::cameras()
}

pub(crate) fn open(target: &Target, opts: &OpenOptions) -> Result<Box<dyn FrameSource>> {
    // Every path checks first: ScreenCaptureKit answers a process without the
    // grant by returning an empty content list, which reads as "no displays"
    // rather than as the permission problem it is.
    if !permission(PermissionKind::Screen).is_usable() {
        return Err(CaptureError::PermissionDenied(PermissionKind::Screen));
    }
    match target {
        Target::Display(id) => Ok(Box::new(stream::SckSource::open_display(*id, opts)?)),
        Target::Region { display, rect } => {
            let opts = OpenOptions {
                region: Some(*rect),
                ..opts.clone()
            };
            Ok(Box::new(stream::SckSource::open_display(*display, &opts)?))
        }
        Target::Window(id) => Ok(Box::new(stream::SckSource::open_window(*id, opts)?)),
        Target::Camera(id) => Ok(Box::new(camera::AvfCameraSource::open(id, opts)?)),
    }
}
