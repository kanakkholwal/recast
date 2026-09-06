//! One-shot captures over the same backends the recorder streams from, so a thumbnail and a recording cannot disagree.
//! capturekit discards stale frames first: the frame after opening a duplication can predate the open.

use std::time::Duration;

use anyhow::{Context, Result};
use capturekit::{Rect, ShotOptions, Target, Warmup};
use image::RgbaImage;

/// How long a picker thumbnail waits for a frame.
/// A picker opens one capture per row serially, so the screenshot budget would make a ten-window list take twenty seconds; a row that misses this simply shows no thumbnail.
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
/// The crop happens in the backend (on the GPU where there is one), so the pixels outside it are never read back.
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

    /// How many distinct colours the compared window needs before a match means
    /// the crop is in the right PLACE. Over a flat desktop every offset matches,
    /// so a pass there would say nothing; measured rather than assumed, because
    /// that vacuous pass is what an earlier version of this test shipped.
    const DISTINCT_COLOURS: usize = 24;

    /// How a cropped capture lines up against the same window of a full one.
    struct CropMatch {
        /// Pixels that were identical in `before` and `after`, so a difference
        /// against the crop is the crop's and not the desktop's.
        stable: u64,
        /// Distinct colours among those, which is what makes a match locating.
        colours: usize,
        mismatched: u64,
    }

    /// Compare `cropped` against the window of `before` at `at`, counting only
    /// pixels `after` agrees with. Pure, so the offset sensitivity this test
    /// depends on is provable without a real screen.
    fn compare_crop(
        cropped: &RgbaImage,
        before: &RgbaImage,
        after: &RgbaImage,
        at: (u32, u32),
    ) -> CropMatch {
        let mut colours = std::collections::HashSet::new();
        let mut stable = 0u64;
        let mut mismatched = 0u64;
        for y in 0..cropped.height() {
            for x in 0..cropped.width() {
                let (sx, sy) = (x + at.0, y + at.1);
                let there = before.get_pixel(sx, sy);
                if there != after.get_pixel(sx, sy) {
                    continue;
                }
                stable += 1;
                colours.insert(there.0);
                if cropped.get_pixel(x, y) != there {
                    mismatched += 1;
                }
            }
        }
        CropMatch {
            stable,
            colours: colours.len(),
            mismatched,
        }
    }

    /// A deterministic non-flat image, standing in for a busy desktop.
    fn patterned(w: u32, h: u32) -> RgbaImage {
        RgbaImage::from_fn(w, h, |x, y| {
            image::Rgba([(x * 7) as u8, (y * 11) as u8, (x * y) as u8, 255])
        })
    }

    fn window_of(img: &RgbaImage, at: (u32, u32), w: u32, h: u32) -> RgbaImage {
        RgbaImage::from_fn(w, h, |x, y| *img.get_pixel(x + at.0, y + at.1))
    }

    #[test]
    fn a_crop_taken_from_the_right_place_matches_every_stable_pixel() {
        let full = patterned(64, 64);
        let crop = window_of(&full, (16, 16), 32, 32);
        let found = compare_crop(&crop, &full, &full, (16, 16));
        assert_eq!(found.mismatched, 0);
    }

    /// The assertion the live test rests on: shift the origin and it must notice.
    #[test]
    fn a_crop_taken_from_the_wrong_place_is_caught() {
        let full = patterned(64, 64);
        let crop = window_of(&full, (16, 16), 32, 32);
        let found = compare_crop(&crop, &full, &full, (25, 23));
        assert!(found.mismatched > 0);
    }

    /// The flat-area guard is only meaningful if a busy area clears it.
    #[test]
    fn a_busy_area_clears_the_flatness_guard() {
        let full = patterned(64, 64);
        let crop = window_of(&full, (16, 16), 32, 32);
        let found = compare_crop(&crop, &full, &full, (16, 16));
        assert!(found.colours >= DISTINCT_COLOURS);
    }

    /// Why the live test refuses a flat area rather than passing on one.
    #[test]
    fn a_flat_area_matches_at_any_offset_and_reports_one_colour() {
        let full = RgbaImage::from_pixel(64, 64, image::Rgba([9, 9, 9, 255]));
        let crop = window_of(&full, (16, 16), 32, 32);
        let found = compare_crop(&crop, &full, &full, (25, 23));
        assert_eq!((found.mismatched, found.colours), (0, 1));
    }

    #[test]
    fn a_pixel_that_moved_between_the_two_captures_is_not_counted() {
        let full = patterned(64, 64);
        let mut moved = full.clone();
        moved.put_pixel(20, 20, image::Rgba([1, 2, 3, 255]));
        let crop = window_of(&full, (16, 16), 32, 32);
        let found = compare_crop(&crop, &full, &moved, (16, 16));
        assert_eq!(found.stable, 32 * 32 - 1);
    }

    /// Checks the region crop against an independent full-display capture: a crop off by an origin or a scale still returns the right SIZE, so only content proves the PLACE.
    /// Live and `#[ignore]`d: it needs a desktop that is showing something and holding still, which a CI runner is not.
    #[test]
    #[ignore = "live: needs a real display showing static content"]
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

        let Ok(before) = grab(Target::Display(display.id)) else {
            return;
        };
        let target = CaptureTarget::resolve_region(region).expect("the region resolves");
        let cropped = grab_region(
            Target::Display(capturekit::DisplayId(target.display_id)),
            target.crop_relative_to_source(),
        )
        .expect("the region captures");
        let Ok(after) = grab(Target::Display(display.id)) else {
            return;
        };

        assert_eq!((cropped.width(), cropped.height()), (w, h));
        // The two full grabs are what separate a moved origin from a moving desktop.
        let found = compare_crop(&cropped, &before, &after, (w, h));
        let total = u64::from(w) * u64::from(h);
        assert!(
            found.stable * 4 >= total,
            "only {} of {total} pixels held still; run this on a static desktop",
            found.stable
        );
        assert!(
            found.colours >= DISTINCT_COLOURS,
            "the compared area is near-flat ({} colours), so a match would not locate the crop",
            found.colours
        );
        assert_eq!(
            found.mismatched, 0,
            "{} of {} unchanged pixels differ, so the crop is not where the region asked for",
            found.mismatched, found.stable
        );
    }
}
