//! One-shot captures, over capturekit.
//!
//! The same backends the recorder streams from, so a picker thumbnail, an agent
//! screenshot and a recording cannot disagree about what a display or window
//! looks like. capturekit discards stale frames before returning one, which is
//! the bug a plain "grab the front buffer" screenshot ships with: the first
//! frame after opening a duplication can predate the open.

use std::time::Duration;

use anyhow::{Context, Result};
use capturekit::{Rect, ShotOptions, Target, Warmup};
use image::RgbaImage;

/// How long a picker thumbnail waits for a frame.
///
/// A picker opens one capture per row and does it serially, so the screenshot
/// budget would make a ten-window list take twenty seconds. A row that misses
/// this shows no thumbnail, which is what a Wayland row does anyway.
const THUMBNAIL_TIMEOUT: Duration = Duration::from_millis(300);

/// One frame of `target`, as RGBA at the size the backend delivers.
pub fn grab(target: Target) -> Result<RgbaImage> {
    grab_region(target, None)
}

/// One frame for a picker row, given up on quickly rather than waited for.
pub fn thumbnail(target: Target) -> Result<RgbaImage> {
    shoot(
        target,
        &ShotOptions {
            timeout: THUMBNAIL_TIMEOUT,
            // One discard: a stale thumbnail is worth less than a fast picker.
            warmup: Warmup::UntilFresh { max_frames: 1 },
            ..ShotOptions::default()
        },
    )
}

/// One frame of `region` within `target`, cropped during acquisition.
///
/// The crop happens in the backend (on the GPU where there is one), so the
/// pixels outside it are never read back.
pub fn grab_region(target: Target, region: Option<Rect>) -> Result<RgbaImage> {
    shoot(
        target,
        &ShotOptions {
            region,
            ..ShotOptions::default()
        },
    )
}

fn shoot(target: Target, opts: &ShotOptions) -> Result<RgbaImage> {
    let kind = target.kind_name();
    let image = capturekit::shot_with(target, opts).with_context(|| format!("{kind} capture"))?;
    to_rgba(&image)
}

/// Copy a captured frame into an `RgbaImage`, off its stride and out of BGRA.
fn to_rgba(image: &capturekit::Image) -> Result<RgbaImage> {
    let (width, height) = (image.width(), image.height());
    let mut out = RgbaImage::new(width, height);
    for y in 0..height {
        let row = image
            .row(y)
            .with_context(|| format!("frame is short of row {y}"))?;
        for (x, pixel) in row
            .as_chunks::<4>()
            .0
            .iter()
            .take(width as usize)
            .enumerate()
        {
            out.put_pixel(
                x as u32,
                y,
                image::Rgba([pixel[2], pixel[1], pixel[0], 255]),
            );
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capture::{CaptureTarget, RegionRect};

    /// The region path resolves a virtual-desktop rectangle to a display and
    /// crops during acquisition. Checked against an independent full-display
    /// capture of the same pixels, because a crop that is off by a display
    /// origin or a scale factor still returns an image of the right SIZE, and
    /// only the content says it came from the right PLACE.
    #[test]
    fn a_region_capture_matches_the_same_crop_of_its_display() {
        if !capturekit::capabilities().display_enumeration {
            return;
        }
        let Ok(displays) = capturekit::displays() else {
            return;
        };
        let Some(display) = displays.iter().find(|d| d.is_primary).or(displays.first()) else {
            return;
        };
        // Inset so the region is unambiguously inside one display.
        let (w, h) = (display.bounds.width / 4, display.bounds.height / 4);
        if w < 16 || h < 16 {
            return;
        }
        let region = RegionRect {
            x: display.bounds.x + w as i32,
            y: display.bounds.y + h as i32,
            width: w,
            height: h,
        };

        let Ok(full) = grab(Target::Display(display.id)) else {
            return;
        };
        let target = CaptureTarget::resolve_region(region).expect("the region resolves");
        let cropped = grab_region(
            Target::Display(capturekit::DisplayId(target.display_id)),
            target.crop_relative_to_source(),
        )
        .expect("the region captures");

        assert_eq!((cropped.width(), cropped.height()), (w, h));
        let mut mismatched = 0u64;
        for y in 0..h {
            for x in 0..w {
                let here = cropped.get_pixel(x, y);
                let there = full.get_pixel(x + w, y + h);
                if here != there {
                    mismatched += 1;
                }
            }
        }
        // Not zero: the screen changes between captures. A wrong origin misses nearly every pixel.
        let total = u64::from(w) * u64::from(h);
        assert!(
            mismatched * 20 < total,
            "{mismatched} of {total} pixels differ, so the crop is not where the region asked for"
        );
    }
}
