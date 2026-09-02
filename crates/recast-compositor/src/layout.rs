use recast_scene::v1::nodes::{CameraClipLayout, CameraLayout, LayoutSide};

use crate::eval::DestRect;
use crate::geometry::CanvasGeometry;

/// Where the screen and the camera land for one frame, and how visible each is.
///
/// Every layout resolves to this, so a transition between two of them is a lerp
/// of four values rather than a second render target.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayoutRects {
    pub screen: DestRect,
    pub camera: DestRect,
    pub screen_opacity: f32,
    pub camera_opacity: f32,
}

/// The layout in force at `source_time`, or `Pip` when nothing is authored.
/// Latest start at or before the time wins, matching how a clip's other
/// per-segment data resolves.
#[must_use]
pub fn layout_at(clip_layouts: &[CameraClipLayout], source_time: f64) -> CameraLayout {
    clip_layouts
        .iter()
        .filter(|c| c.start <= source_time)
        .max_by(|a, b| a.start.total_cmp(&b.start))
        .map_or(CameraLayout::Pip, |c| c.layout)
}

/// Shrinks `into` to `aspect` and centres it. Used for the SCREEN in a split:
/// cropping to fill would hide the edges of what is being demonstrated, and a
/// tutorial cannot afford to lose the side of a terminal.
#[must_use]
fn fit(into: DestRect, aspect: f32) -> DestRect {
    if into.w <= 0.0 || into.h <= 0.0 || aspect <= 0.0 {
        return into;
    }
    let (w, h) = if into.w / into.h > aspect {
        (into.h * aspect, into.h)
    } else {
        (into.w, into.w / aspect)
    };
    DestRect {
        x: into.x + (into.w - w) * 0.5,
        y: into.y + (into.h - h) * 0.5,
        w,
        h,
    }
}

/// The two halves of `canvas`, camera first, for a split of `fraction` on `side`.
fn halves(
    canvas: DestRect,
    fraction: f32,
    side: LayoutSide,
    vertical: bool,
) -> (DestRect, DestRect) {
    let (span, camera_span) = match vertical {
        true => (canvas.h, canvas.h * fraction),
        false => (canvas.w, canvas.w * fraction),
    };
    let screen_span = span - camera_span;
    let camera_first = side == LayoutSide::Start;
    let (first, second) = match camera_first {
        true => (camera_span, screen_span),
        false => (screen_span, camera_span),
    };
    let (a, b) = match vertical {
        true => (
            DestRect {
                x: canvas.x,
                y: canvas.y,
                w: canvas.w,
                h: first,
            },
            DestRect {
                x: canvas.x,
                y: canvas.y + first,
                w: canvas.w,
                h: second,
            },
        ),
        false => (
            DestRect {
                x: canvas.x,
                y: canvas.y,
                w: first,
                h: canvas.h,
            },
            DestRect {
                x: canvas.x + first,
                y: canvas.y,
                w: second,
                h: canvas.h,
            },
        ),
    };
    match camera_first {
        true => (a, b),
        false => (b, a),
    }
}

/// Resolve one layout against the canvas.
///
/// `bubble` is where the PiP bubble would sit, already resolved by
/// `camera::bubble_params`, so this function never re-derives placement.
/// `source_aspect` is the screen recording's; the camera covers its half rather
/// than fitting it, which is what `LayerParams::cover_fit` already does for the
/// bubble.
#[must_use]
pub fn resolve(
    layout: CameraLayout,
    geometry: CanvasGeometry,
    bubble: Option<DestRect>,
    source_aspect: f32,
) -> LayoutRects {
    let screen = DestRect {
        x: geometry.video_x as f32,
        y: geometry.video_y as f32,
        w: geometry.video_w as f32,
        h: geometry.video_h as f32,
    };
    let canvas = DestRect {
        x: 0.0,
        y: 0.0,
        w: geometry.canvas_w as f32,
        h: geometry.canvas_h as f32,
    };
    let hidden = DestRect {
        x: 0.0,
        y: 0.0,
        w: 0.0,
        h: 0.0,
    };

    match layout {
        CameraLayout::Pip => LayoutRects {
            screen,
            camera: bubble.unwrap_or(hidden),
            screen_opacity: 1.0,
            camera_opacity: f32::from(u8::from(bubble.is_some())),
        },
        CameraLayout::ScreenOnly => LayoutRects {
            screen,
            camera: hidden,
            screen_opacity: 1.0,
            camera_opacity: 0.0,
        },
        CameraLayout::SplitH { .. } | CameraLayout::SplitV { .. } => {
            let vertical = matches!(layout, CameraLayout::SplitV { .. });
            let side = match layout {
                CameraLayout::SplitH { side, .. } | CameraLayout::SplitV { side, .. } => side,
                _ => LayoutSide::Start,
            };
            let fraction = layout.split_fraction().unwrap_or(0.5) as f32;
            let (camera_half, screen_half) = halves(canvas, fraction, side, vertical);
            LayoutRects {
                screen: fit(screen_half, source_aspect),
                camera: camera_half,
                screen_opacity: 1.0,
                camera_opacity: 1.0,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn geometry() -> CanvasGeometry {
        crate::geometry::canvas_geometry(1920, 1080, 0.0, None)
    }

    fn bubble() -> DestRect {
        DestRect {
            x: 1500.0,
            y: 800.0,
            w: 300.0,
            h: 300.0,
        }
    }

    fn split(fraction: f64, side: LayoutSide, vertical: bool) -> CameraLayout {
        match vertical {
            true => CameraLayout::SplitV { fraction, side },
            false => CameraLayout::SplitH { fraction, side },
        }
    }

    #[test]
    fn no_authored_layout_is_the_bubble_every_project_already_had() {
        assert_eq!(layout_at(&[], 5.0), CameraLayout::Pip);
    }

    #[test]
    fn the_latest_clip_at_or_before_the_time_wins() {
        let clips = [
            CameraClipLayout {
                start: 0.0,
                layout: CameraLayout::Pip,
            },
            CameraClipLayout {
                start: 4.0,
                layout: CameraLayout::ScreenOnly,
            },
        ];
        assert_eq!(layout_at(&clips, 3.9), CameraLayout::Pip);
        assert_eq!(layout_at(&clips, 4.0), CameraLayout::ScreenOnly);
        assert_eq!(layout_at(&clips, 99.0), CameraLayout::ScreenOnly);
    }

    /// A clip authored after a trim can start later than the playhead ever goes
    /// backwards to; before the first key the recording is still the bubble.
    #[test]
    fn a_time_before_every_clip_falls_back_to_the_bubble() {
        let clips = [CameraClipLayout {
            start: 4.0,
            layout: CameraLayout::ScreenOnly,
        }];
        assert_eq!(layout_at(&clips, 1.0), CameraLayout::Pip);
    }

    #[test]
    fn pip_draws_the_screen_full_frame_and_the_bubble_where_it_was_placed() {
        let r = resolve(CameraLayout::Pip, geometry(), Some(bubble()), 16.0 / 9.0);
        assert_eq!(
            r.screen,
            DestRect {
                x: 0.0,
                y: 0.0,
                w: 1920.0,
                h: 1080.0
            }
        );
        assert_eq!(r.camera, bubble());
        assert_eq!(r.camera_opacity, 1.0);
    }

    /// The camera layer is disabled or has no recording: the bubble must not
    /// draw at whatever rect happens to be left over.
    #[test]
    fn pip_without_a_bubble_hides_the_camera() {
        let r = resolve(CameraLayout::Pip, geometry(), None, 16.0 / 9.0);
        assert_eq!(r.camera_opacity, 0.0);
        assert_eq!(r.screen_opacity, 1.0);
    }

    #[test]
    fn screen_only_hides_the_camera_and_leaves_the_screen_alone() {
        let r = resolve(
            CameraLayout::ScreenOnly,
            geometry(),
            Some(bubble()),
            16.0 / 9.0,
        );
        assert_eq!(r.camera_opacity, 0.0);
        assert_eq!(
            r.screen,
            DestRect {
                x: 0.0,
                y: 0.0,
                w: 1920.0,
                h: 1080.0
            }
        );
    }

    #[test]
    fn a_horizontal_split_gives_the_camera_its_fraction_of_the_width() {
        let r = resolve(
            split(0.3, LayoutSide::Start, false),
            geometry(),
            None,
            16.0 / 9.0,
        );
        assert!((r.camera.w - 1920.0 * 0.3).abs() < 0.01, "{:?}", r.camera);
        assert_eq!(r.camera.x, 0.0);
        assert_eq!(r.camera.h, 1080.0);
        assert!(
            r.screen.x >= r.camera.w - 0.01,
            "the screen overlaps the camera"
        );
    }

    #[test]
    fn the_side_decides_which_half_the_camera_takes() {
        let start = resolve(split(0.3, LayoutSide::Start, false), geometry(), None, 1.0);
        let end = resolve(split(0.3, LayoutSide::End, false), geometry(), None, 1.0);
        assert_eq!(start.camera.x, 0.0);
        assert!(
            (end.camera.x - 1920.0 * 0.7).abs() < 0.01,
            "{:?}",
            end.camera
        );
        // Centred in its half, not pinned to the edge, so the screen sits at the middle of the 70% the camera left it.
        let centre = end.screen.x + end.screen.w * 0.5;
        assert!((centre - 1920.0 * 0.35).abs() < 0.01, "{:?}", end.screen);
    }

    #[test]
    fn a_vertical_split_divides_the_height_instead() {
        let r = resolve(
            split(0.4, LayoutSide::Start, true),
            geometry(),
            None,
            16.0 / 9.0,
        );
        assert_eq!(r.camera.w, 1920.0);
        assert!((r.camera.h - 1080.0 * 0.4).abs() < 0.01, "{:?}", r.camera);
        assert!(r.screen.y >= r.camera.h - 0.01);
    }

    /// The whole reason the screen fits rather than covers: a cropped terminal
    /// loses the side of what is being demonstrated.
    #[test]
    fn the_screen_keeps_its_aspect_inside_its_half() {
        let r = resolve(
            split(0.5, LayoutSide::Start, false),
            geometry(),
            None,
            16.0 / 9.0,
        );
        assert!(
            (r.screen.w / r.screen.h - 16.0 / 9.0).abs() < 1e-4,
            "screen {:?} is not 16:9",
            r.screen
        );
        assert!(r.screen.w <= 1920.0 * 0.5 + 0.01);
        assert!(r.screen.h <= 1080.0 + 0.01);
    }

    /// The camera fills its half instead, the way the bubble already covers.
    #[test]
    fn the_camera_fills_its_half_rather_than_fitting_it() {
        let r = resolve(
            split(0.5, LayoutSide::End, true),
            geometry(),
            None,
            16.0 / 9.0,
        );
        assert_eq!(r.camera.w, 1920.0);
        assert!((r.camera.h - 540.0).abs() < 0.01);
    }

    /// A fraction that collapses one side is a worse `ScreenOnly`, so it clamps.
    #[test]
    fn a_degenerate_fraction_still_leaves_both_halves_on_screen() {
        for fraction in [-1.0, 0.0, 0.01, 0.99, 1.0, 5.0] {
            let r = resolve(
                split(fraction, LayoutSide::Start, false),
                geometry(),
                None,
                1.0,
            );
            assert!(
                r.camera.w > 1.0,
                "camera vanished at {fraction}: {:?}",
                r.camera
            );
            assert!(
                r.screen.w > 1.0,
                "screen vanished at {fraction}: {:?}",
                r.screen
            );
        }
    }

    /// Padding moves the video rect inside a larger canvas; a split must divide
    /// the CANVAS, or the halves would sit inside the padded area.
    #[test]
    fn a_split_divides_the_whole_canvas_not_the_padded_video_rect() {
        let padded = crate::geometry::canvas_geometry(1920, 1080, 10.0, None);
        let r = resolve(
            split(0.5, LayoutSide::Start, false),
            padded,
            None,
            16.0 / 9.0,
        );
        assert_eq!(r.camera.x, 0.0);
        assert!((r.camera.w - padded.canvas_w as f32 * 0.5).abs() < 0.01);
    }
}
