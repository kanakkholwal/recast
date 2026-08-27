use core::time::Duration;

use capturekit_core::{CaptureError, ColorSpaceRequest, Rect, Result, Timestamp};

use crate::backend::ScreenBackend;
use crate::image::Image;

/// Whether the cursor is drawn into captured frames.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CursorMode {
    /// Composite the cursor into the frame.
    Include,
    /// Leave the cursor out, so it can be drawn later at any size or style.
    #[default]
    Exclude,
}

/// How many frames to throw away before trusting one.
///
/// The first frame out of a freshly opened source is routinely stale: DXGI's
/// `AcquireNextFrame` can return an accumulated frame from before
/// `DuplicateOutput`, and ScreenCaptureKit's first sample can predate the content
/// becoming current. A screenshot that takes it shows the user the previous
/// desktop, which is the bug most screenshot crates ship.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Warmup {
    /// Discard frames until one is newer than the moment of the request.
    UntilFresh {
        /// Give up discarding after this many, and take what is there.
        max_frames: u32,
    },
    /// Take the first frame offered. Correct only for a source already streaming.
    None,
}

impl Default for Warmup {
    fn default() -> Self {
        Self::UntilFresh { max_frames: 4 }
    }
}

/// What a one-shot capture should do.
#[derive(Debug, Clone, PartialEq)]
pub struct ShotOptions {
    /// Whether to draw the cursor.
    pub cursor: CursorMode,
    /// A sub-rectangle of the target, in the target's own coordinates.
    pub region: Option<Rect>,
    /// The colour space to deliver.
    pub color_space: ColorSpaceRequest,
    /// How many stale frames to discard first.
    pub warmup: Warmup,
    /// How long to wait for each frame.
    pub timeout: Duration,
}

impl Default for ShotOptions {
    fn default() -> Self {
        Self {
            cursor: CursorMode::default(),
            region: None,
            color_space: ColorSpaceRequest::default(),
            warmup: Warmup::default(),
            timeout: Duration::from_secs(2),
        }
    }
}

/// Take one frame from an open backend.
///
/// Shared by the one-shot API and by a snapshot taken mid-recording, so both get
/// the same warmup, the same crop and the same validation.
pub(crate) fn grab_one(
    backend: &mut dyn ScreenBackend,
    opts: &ShotOptions,
    requested_at: Timestamp,
) -> Result<Image> {
    let cropped_natively = backend.region().is_some();
    let image = acquire_fresh(backend, opts, requested_at)?;
    match opts.region {
        Some(region) if !cropped_natively => image.cropped(region),
        _ => Ok(image),
    }
}

fn acquire_fresh(
    backend: &mut dyn ScreenBackend,
    opts: &ShotOptions,
    requested_at: Timestamp,
) -> Result<Image> {
    let (format, color_space, width, height) = {
        let desc = backend.describe();
        (desc.format, desc.color_space, desc.width, desc.height)
    };
    let attempts = match opts.warmup {
        Warmup::None => 0,
        Warmup::UntilFresh { max_frames } => max_frames,
    };

    let mut newest = None;
    for _ in 0..=attempts {
        let frame = backend.next_frame(opts.timeout)?;
        let fresh = frame.pts >= requested_at;
        let image = Image::new(
            frame.bytes.to_vec(),
            width,
            height,
            frame.stride,
            format,
            color_space,
            frame.pts,
        )?;
        if fresh {
            return Ok(image);
        }
        newest = Some(image);
    }

    // Every frame predated the request. The source is idle rather than broken, so
    // the newest stale frame is a better answer than an error.
    newest.ok_or(CaptureError::Timeout(opts.timeout))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_warmup_discards_stale_frames() {
        assert_eq!(Warmup::default(), Warmup::UntilFresh { max_frames: 4 });
    }

    #[test]
    fn the_cursor_is_left_out_by_default() {
        assert_eq!(ShotOptions::default().cursor, CursorMode::Exclude);
    }
}
