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
        // `SCContentFilter` takes an exclusion list of any windows, the only one of the three that can hide a stranger's window.
        exclusion: ExclusionSupport::AnyWindow,
        window_capture: true,
        camera_capture: true,
        window_enumeration: true,
        display_enumeration: true,
        region_crop: RegionCrop::DuringAcquisition,
        cursor_in_frame: true,
        // Position and shape come from a separate CoreGraphics call, so they are not on the frame clock yet.
        cursor_samples: false,
        cursor_pointer: true,
        cursor_buttons: true,
        dirty_rects: false,
        exclusive_display_capture: false,
        audio_loopback: true,
        audio_loopback_gap_filling: false,
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
                // TCC can't distinguish never-asked from refused here, and asking again is harmless in the former case.
                Permission::NotDetermined
            }
        }
        _ => Permission::NotDetermined,
    }
}

/// Asks TCC for screen recording.
/// The prompt appears once per application and afterwards this returns the standing answer silently, which is why a `Denied` result must send the user to System Settings.
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
    // SAFETY: the host clock is a process-wide singleton and reading its time takes no arguments.
    let time = unsafe { objc2_core_media::CMClock::host_time_clock().time() };
    match time.timescale {
        0 => Timestamp::ZERO,
        scale => Timestamp::from_ticks(time.value, i64::from(scale)),
    }
}

pub(crate) fn audio_devices() -> Result<Vec<AudioDevice>> {
    coreaudio::devices()
}

/// System audio through ScreenCaptureKit, the only way to tap the output mix without a virtual driver, and inputs through AVFoundation.
/// ScreenCaptureKit captures only the DEFAULT input, so AVFoundation opens the picked device and takes the Microphone grant instead of Screen Recording.
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
    // ScreenCaptureKit answers an ungranted process with an empty content list, which reads as 'no displays'.
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
