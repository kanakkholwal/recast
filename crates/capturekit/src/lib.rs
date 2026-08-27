//! Cross-platform screen, window and camera capture.
//!
//! A screenshot and a recording are the same acquisition with different
//! lifetimes, so [`shot`] and the streaming capturer drive one backend per
//! platform. A fix to stale frames, cropping, colour or permissions lands on both.
//!
//! Vocabulary types live in [`capturekit_core`] and are re-exported here.

#![deny(missing_docs)]
#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]

mod backend;
mod image;
mod shot;

#[cfg(test)]
mod mock;

pub use capturekit_core::{
    Camera, CameraId, CaptureError, ChromaSiting, ColorRange, ColorSpace, ColorSpaceRequest,
    DirtyRects, Display, DisplayId, LostReason, MatrixCoefficients, Permission, PermissionKind,
    PixelFormat, PlaneFormat, Primaries, Rect, Result, Rotation, SourceDesc, Target,
    TransferFunction, Window, WindowId,
};
pub use image::Image;
pub use shot::{CursorMode, ShotOptions, Warmup};

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
        let image = grab_one(&mut source, &opts, REQUESTED_AT).expect("an idle source is not an error");
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
