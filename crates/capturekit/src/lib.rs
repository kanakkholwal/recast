//! Cross-platform screen, window and camera capture.
//! A screenshot and a recording are one acquisition with different lifetimes, so [`shot`] and [`capturer`] share a backend per platform.

#![deny(missing_docs)]
#![deny(unsafe_op_in_unsafe_fn)]
#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]
// 254 unsafe blocks of FFI; `platform::windows` carries the only opt-out.
#![deny(clippy::undocumented_unsafe_blocks)]

mod audio;
mod backend;
mod capturer;
// Push backends only: X11 polls the server, so a Linux build without the portal never hands a frame across threads.
#[cfg(any(windows, target_os = "macos", feature = "wayland"))]
mod deliver;
mod image;
mod platform;
mod pointer;
mod session;
mod shot;

/// A scripted capture source, for downstream tests and headless CI.
#[cfg(any(test, feature = "mock"))]
pub mod mock;

pub use audio::{AudioBuffer, AudioCapturer, AudioCapturerBuilder, AudioHandle};
pub use capturekit_core::{
    AudioDesc, AudioDevice, AudioDeviceId, AudioDirection, AudioFormat, Camera, CameraFormat,
    CameraId, Capabilities, CaptureError, ChromaSiting, ColorRange, ColorSpace, ColorSpaceRequest,
    CursorButtons, CursorSample, CursorShape, CursorShapeKind, DirtyRects, Display, DisplayId,
    ExclusionSupport, GpuHandle, LostReason, MatrixCoefficients, Pacer, Pacing, Permission,
    PermissionKind, PixelFormat, PlaneFormat, Primaries, Rect, RegionCrop, Result, Rotation,
    SampleFormat, SourceDesc, Target, Timestamp, TransferFunction, Window, WindowId,
};
pub use capturer::{CaptureHandle, Capturer, CapturerBuilder, Flow, Frame};
pub use image::Image;
pub use pointer::{Pointer, PointerCapturer, PointerSample};
pub use session::{Session, SessionAudio, SessionBuilder, SessionFrame, TrackId};
pub use shot::{CursorMode, ShotOptions, Warmup};

use platform::os;

/// What this platform's capture backend can and cannot do.
///
/// Reported as data rather than left to `cfg`, so a caller asks once and branches
/// on the answer. The API shape is identical on all three systems; only these
/// values differ. Consult it before relying on exclusion, window enumeration or
/// cursor samples, all of which one platform or another cannot provide.
#[must_use]
pub fn capabilities() -> Capabilities {
    os::capabilities()
}

/// Every monitor available to capture.
pub fn displays() -> Result<Vec<Display>> {
    os::displays()
}

/// Every window a user would recognise and could sensibly capture.
/// Excludes tool windows and title-less shell scaffolding, of which any desktop has dozens.
pub fn windows() -> Result<Vec<Window>> {
    os::windows()
}

/// Every camera available to capture.
/// Each device is opened briefly to read the modes it advertises, then shut down again, so listing cameras does not leave one powered.
pub fn cameras() -> Result<Vec<Camera>> {
    os::cameras()
}

/// Whether a capability may be used, and if not, whether asking would help.
#[must_use]
pub fn permission(kind: PermissionKind) -> Permission {
    os::permission(kind)
}

/// Prompt for a capability, where the platform has a prompt to show.
/// Blocks while the user decides. A `Denied` answer is final until they change it in system settings, which is why [`Permission::is_requestable`] exists.
pub fn request_permission(kind: PermissionKind) -> Permission {
    os::request_permission(kind)
}

/// Capture one frame: acquire, grab, release.
/// Discards stale frames first. See [`Warmup`] for why that is not optional.
pub fn shot(target: Target) -> Result<Image> {
    shot_with(target, &ShotOptions::default())
}

/// Capture one frame with explicit options.
pub fn shot_with(target: Target, opts: &ShotOptions) -> Result<Image> {
    let requested_at = os::now();
    let mut backend = os::open(&target, &platform::OpenOptions::from(opts))?;
    let image = shot::grab_one(backend.as_mut(), opts, requested_at);
    let stopped = backend.stop();
    // The frame is the point: a failed release is worth logging, not worth failing a screenshot the caller already has.
    if let Err(err) = stopped {
        log::debug!("releasing the capture source after a shot failed: {err}");
    }
    image
}

/// Every audio device available to capture, inputs and loopback alike.
pub fn audio_devices() -> Result<Vec<AudioDevice>> {
    os::audio_devices()
}

/// Open a microphone or line input.
/// Captures the system default unless [`AudioCapturerBuilder::device`] names one.
#[must_use]
pub fn audio_input() -> AudioCapturerBuilder {
    AudioCapturerBuilder::new(AudioDirection::Input)
}

/// Open a capture of what the system is playing.
///
/// A loopback device delivers nothing at all while nothing is playing, so the
/// backend inserts real silence for the gaps rather than letting the track come
/// out short. See [`AudioBuffer::is_inserted_silence`].
#[must_use]
pub fn audio_loopback() -> AudioCapturerBuilder {
    AudioCapturerBuilder::new(AudioDirection::Loopback)
}

/// Open a streaming capture of `target`.
#[must_use]
pub fn capturer(target: Target) -> CapturerBuilder {
    CapturerBuilder::new(target)
}

#[cfg(test)]
mod tests {
    use capturekit_core::Timestamp;

    use crate::mock::{MockFrame, MockSource};
    use crate::shot::{grab_one, ShotOptions, Warmup};
    use crate::{LostReason, Rect};

    /// The request lands at 1000ns; anything earlier is a frame from before it.
    const REQUESTED_AT: Timestamp = Timestamp::from_nanos(1_000);

    #[test]
    fn a_stale_first_frame_is_discarded_rather_than_returned_as_the_screenshot() {
        let mut source = MockSource::new(
            4,
            4,
            vec![MockFrame::new(500, 0xAA), MockFrame::new(1_500, 0xBB)],
        );
        let image = grab_one(&mut source, &ShotOptions::default(), REQUESTED_AT)
            .expect("the second frame is fresh");
        assert_eq!(image.bytes()[0], 0xBB, "the stale frame was returned");
        assert_eq!(source.served(), 2);
    }

    #[test]
    fn warmup_gives_up_after_its_budget_and_returns_the_newest_stale_frame() {
        let stale: Vec<_> = (0..10).map(|i| MockFrame::new(i, 0x10 + i as u8)).collect();
        let mut source = MockSource::new(4, 4, stale);
        let opts = ShotOptions {
            warmup: Warmup::UntilFresh { max_frames: 3 },
            ..ShotOptions::default()
        };
        let image =
            grab_one(&mut source, &opts, REQUESTED_AT).expect("an idle source is not an error");
        assert_eq!(source.served(), 4, "one attempt plus three discards");
        assert_eq!(image.bytes()[0], 0x13, "the newest of the stale frames");
    }

    #[test]
    fn warmup_none_takes_the_first_frame_even_when_it_is_stale() {
        let mut source = MockSource::new(
            4,
            4,
            vec![MockFrame::new(500, 0xAA), MockFrame::new(1_500, 0xBB)],
        );
        let opts = ShotOptions {
            warmup: Warmup::None,
            ..ShotOptions::default()
        };
        let image = grab_one(&mut source, &opts, REQUESTED_AT).expect("a frame");
        assert_eq!(image.bytes()[0], 0xAA);
        assert_eq!(source.served(), 1);
    }

    #[test]
    fn a_lost_source_surfaces_rather_than_being_retried_forever() {
        let mut source = MockSource::new(4, 4, vec![MockFrame::new(2_000, 0xCC)])
            .failing_at(0, LostReason::DisplayDisconnected);
        let err = grab_one(&mut source, &ShotOptions::default(), REQUESTED_AT)
            .expect_err("a disconnected display is not recoverable");
        assert!(matches!(
            err,
            crate::CaptureError::Lost(LostReason::DisplayDisconnected)
        ));
    }

    #[test]
    fn a_region_the_backend_cannot_crop_is_cropped_on_the_host() {
        let mut source = MockSource::new(16, 16, vec![MockFrame::new(2_000, 0x42)]);
        let opts = ShotOptions {
            region: Some(Rect::new(2, 2, 8, 8)),
            ..ShotOptions::default()
        };
        let image = grab_one(&mut source, &opts, REQUESTED_AT).expect("a frame");
        assert_eq!((image.width(), image.height()), (8, 8));
    }

    #[test]
    fn a_region_the_backend_already_cropped_is_not_cropped_twice() {
        let region = Rect::new(2, 2, 8, 8);
        let mut source =
            MockSource::new(16, 16, vec![MockFrame::new(2_000, 0x42)]).cropping_to(region);
        let opts = ShotOptions {
            region: Some(region),
            ..ShotOptions::default()
        };
        let image = grab_one(&mut source, &opts, REQUESTED_AT).expect("a frame");
        assert_eq!((image.width(), image.height()), (8, 8));
    }

    /// The case a size comparison gets wrong: a region as large as the display but
    /// offset looks uncropped, so inferring from the frame size would skip the crop.
    #[test]
    fn a_full_size_but_offset_region_is_still_cropped() {
        let mut source = MockSource::new(16, 16, vec![MockFrame::new(2_000, 0x42)]);
        let opts = ShotOptions {
            region: Some(Rect::new(4, 0, 16, 16)),
            ..ShotOptions::default()
        };
        let image = grab_one(&mut source, &opts, REQUESTED_AT).expect("a frame");
        assert_eq!((image.width(), image.height()), (12, 16));
    }
}
