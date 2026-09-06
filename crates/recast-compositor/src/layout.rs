use recast_scene::v1::nodes::{CameraClipLayout, CameraLayout, LayoutSide};
use recast_time::Segment;

use crate::eval::DestRect;
use crate::geometry::CanvasGeometry;

/// Tolerance for matching an anchor to a segment start. Mirrors `segments.ts`.
pub const ANCHOR_EPS: f64 = 1e-4;

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
    /// How much of the bubble's rounding and shadow this arrangement keeps: 1
    /// for the bubble, 0 for anything that owns a whole region of the frame.
    /// Blended like the rects so a move eases the corners instead of popping.
    pub camera_rounding: f32,
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
            camera_rounding: 1.0,
        },
        // Still a bubble, just an invisible one, so easing back out of it rounds rather than snaps.
        CameraLayout::ScreenOnly => LayoutRects {
            screen,
            camera: bubble.unwrap_or(hidden),
            screen_opacity: 1.0,
            camera_opacity: 0.0,
            camera_rounding: 1.0,
        },
        // Fills the canvas and covers, like a bubble: a talking head letterboxed inside its own frame would be odd.
        CameraLayout::CameraOnly => LayoutRects {
            screen,
            camera: canvas,
            screen_opacity: 0.0,
            camera_opacity: 1.0,
            camera_rounding: 0.0,
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
                camera_rounding: 0.0,
            }
        }
    }
}

/// Linear blend of two resolved layouts. A transition MOVES the screen and the
/// camera between arrangements rather than dissolving one composite into
/// another: a dissolve needs a second render target and reads like a slideshow.
#[must_use]
pub fn lerp(from: LayoutRects, to: LayoutRects, t: f32) -> LayoutRects {
    let t = t.clamp(0.0, 1.0);
    let mix = |a: f32, b: f32| a + (b - a) * t;
    let rect = |a: DestRect, b: DestRect| DestRect {
        x: mix(a.x, b.x),
        y: mix(a.y, b.y),
        w: mix(a.w, b.w),
        h: mix(a.h, b.h),
    };
    LayoutRects {
        screen: rect(from.screen, to.screen),
        camera: rect(from.camera, to.camera),
        screen_opacity: mix(from.screen_opacity, to.screen_opacity),
        camera_opacity: mix(from.camera_opacity, to.camera_opacity),
        camera_rounding: mix(from.camera_rounding, to.camera_rounding),
    }
}

/// The layout anchored exactly at `start`, or the bubble when that clip was
/// never given one. Exact match, so the order the document holds clips in
/// cannot change the answer.
#[must_use]
pub fn layout_at_start(clips: &[CameraClipLayout], start: f64) -> CameraLayout {
    clips
        .iter()
        .find(|c| (c.start - start).abs() <= ANCHOR_EPS)
        .map_or(CameraLayout::Pip, |c| c.layout)
}

/// The layout in force at `source_time`, the one the clip before it had, and
/// this clip's ORIGINAL start.
///
/// Anchors resolve against `segments` exactly the way `SegmentAnim` does, so a
/// key that a cut leaves on no clip stops applying rather than taking effect
/// from its raw time onward. The editor labels its timeline by the same rule,
/// which is what keeps the label and the rendered frame agreeing.
///
/// `from` is `None` when the previous clip was arranged the same way: there is
/// nothing to ease, and reporting a move would strip the bubble's rounding for
/// a transition that does not exist.
#[must_use]
pub fn neighbours(
    clips: &[CameraClipLayout],
    segments: &[Segment],
    source_time: f64,
) -> (CameraLayout, Option<CameraLayout>, Option<f64>) {
    // Held at the last clip once the playhead runs past it, as `layoutAtTime` does.
    let Some(index) = segments
        .iter()
        .position(|s| source_time >= s.start - ANCHOR_EPS && source_time < s.end)
        .or_else(|| segments.len().checked_sub(1))
    else {
        return (CameraLayout::Pip, None, None);
    };
    let to = layout_at_start(clips, segments[index].start);
    let from = index
        .checked_sub(1)
        .map(|i| layout_at_start(clips, segments[i].start))
        .filter(|from| *from != to);
    (to, from, from.map(|_| segments[index].start))
}

/// How far through the transition into the current clip `output_time` is, on a
/// 0..1 scale where 1 is fully arrived.
///
/// OUTPUT axis on purpose: a cross-fade is what the viewer sees, so a cut that
/// removes time between two clips must not stretch or skip it. The layouts
/// themselves stay keyed to the original axis, which is what follows the clip.
#[must_use]
pub fn transition_progress(output_time: f64, boundary: f64, duration: f64) -> f64 {
    if duration <= 0.0 {
        return 1.0;
    }
    ((output_time - boundary) / duration).clamp(0.0, 1.0)
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
    fn a_transition_of_no_length_lands_immediately() {
        assert_eq!(transition_progress(5.0, 5.0, 0.0), 1.0);
        assert_eq!(transition_progress(5.0, 5.0, -1.0), 1.0);
    }

    #[test]
    fn progress_runs_from_the_boundary_to_the_duration() {
        assert_eq!(transition_progress(4.0, 4.0, 0.5), 0.0);
        assert!((transition_progress(4.25, 4.0, 0.5) - 0.5).abs() < 1e-9);
        assert_eq!(transition_progress(4.5, 4.0, 0.5), 1.0);
    }

    /// The playhead can sit before the boundary on the frame a seek lands on,
    /// and past it for the whole rest of the clip.
    #[test]
    fn progress_is_clamped_on_both_sides_of_the_window() {
        assert_eq!(transition_progress(1.0, 4.0, 0.5), 0.0);
        assert_eq!(transition_progress(99.0, 4.0, 0.5), 1.0);
    }

    /// Layouts are keyed to a clip, so the clips have to exist for one to apply.
    fn segments(bounds: &[(f64, f64)]) -> Vec<Segment> {
        bounds
            .iter()
            .enumerate()
            .map(|(index, (start, end))| Segment {
                start: *start,
                end: *end,
                index,
            })
            .collect()
    }

    fn clip(start: f64, layout: CameraLayout) -> CameraClipLayout {
        CameraClipLayout { start, layout }
    }

    #[test]
    fn the_first_clip_of_a_project_has_nothing_to_ease_from() {
        let clips = [clip(0.0, CameraLayout::ScreenOnly)];
        let (to, from, start) = neighbours(&clips, &segments(&[(0.0, 4.0)]), 1.0);
        assert_eq!(to, CameraLayout::ScreenOnly);
        assert_eq!(from, None);
        assert_eq!(start, None);
    }

    /// Before the first key the recording was showing the bubble, so that is
    /// what a clip authored later eases out of.
    #[test]
    fn a_clip_authored_after_the_start_eases_out_of_the_bubble() {
        let clips = [clip(4.0, CameraLayout::ScreenOnly)];
        let segs = segments(&[(0.0, 4.0), (4.0, 9.0)]);
        let (to, from, start) = neighbours(&clips, &segs, 5.0);
        assert_eq!(to, CameraLayout::ScreenOnly);
        assert_eq!(from, Some(CameraLayout::Pip));
        assert_eq!(start, Some(4.0));
    }

    #[test]
    fn a_later_clip_eases_out_of_the_one_before_it() {
        let clips = [
            clip(0.0, CameraLayout::Pip),
            clip(4.0, CameraLayout::ScreenOnly),
            clip(
                9.0,
                CameraLayout::SplitH {
                    fraction: 0.3,
                    side: LayoutSide::Start,
                },
            ),
        ];
        let segs = segments(&[(0.0, 4.0), (4.0, 9.0), (9.0, 14.0)]);
        let (to, from, start) = neighbours(&clips, &segs, 10.0);
        assert!(matches!(to, CameraLayout::SplitH { .. }));
        assert_eq!(from, Some(CameraLayout::ScreenOnly));
        assert_eq!(start, Some(9.0));
    }

    /// Anchors arrive in whatever order the document holds them, and both
    /// neighbours can sit behind the playhead, which a scan-order walk got wrong.
    #[test]
    fn the_neighbours_are_found_however_the_clips_are_ordered() {
        let ordered = [
            clip(0.0, CameraLayout::CameraOnly),
            clip(4.0, CameraLayout::ScreenOnly),
        ];
        let reversed = [ordered[1], ordered[0]];
        let segs = segments(&[(0.0, 4.0), (4.0, 9.0)]);
        assert_eq!(
            neighbours(&ordered, &segs, 5.0),
            neighbours(&reversed, &segs, 5.0)
        );
        assert_eq!(
            neighbours(&reversed, &segs, 5.0).1,
            Some(CameraLayout::CameraOnly)
        );
    }

    /// A cut that removes the clip a layout was keyed to leaves the anchor on
    /// nothing. The editor's timeline already reads it as the bubble; keying on
    /// raw time here rendered the layout anyway, from the wrong moment.
    #[test]
    fn an_anchor_no_clip_starts_at_stops_applying() {
        let clips = [clip(5.0, CameraLayout::CameraOnly)];
        let segs = segments(&[(0.0, 3.0), (4.0, 9.0)]);
        assert_eq!(neighbours(&clips, &segs, 7.0).0, CameraLayout::Pip);
    }

    /// Two clips arranged the same way are not a transition. Reporting one
    /// would ease between identical rects while stripping the bubble's rounding.
    #[test]
    fn neighbouring_clips_with_the_same_layout_are_not_a_move() {
        let clips = [
            clip(0.0, CameraLayout::CameraOnly),
            clip(4.0, CameraLayout::CameraOnly),
        ];
        let segs = segments(&[(0.0, 4.0), (4.0, 9.0)]);
        let (to, from, start) = neighbours(&clips, &segs, 5.0);
        assert_eq!(to, CameraLayout::CameraOnly);
        assert_eq!(from, None);
        assert_eq!(start, None);
    }

    /// Held at the last clip, the rule `layoutAtTime` uses, so the final frame
    /// of an export does not fall back to the bubble.
    #[test]
    fn a_playhead_past_the_last_clip_keeps_that_clips_layout() {
        let clips = [clip(4.0, CameraLayout::CameraOnly)];
        let segs = segments(&[(0.0, 4.0), (4.0, 9.0)]);
        assert_eq!(neighbours(&clips, &segs, 99.0).0, CameraLayout::CameraOnly);
    }

    #[test]
    fn a_project_with_no_clips_is_the_bubble_and_eases_from_nothing() {
        let (to, from, start) = neighbours(&[], &segments(&[(0.0, 9.0)]), 5.0);
        assert_eq!(to, CameraLayout::Pip);
        assert_eq!(from, None);
        assert_eq!(start, None);
    }

    #[test]
    fn the_ends_of_a_lerp_are_the_layouts_themselves() {
        let a = resolve(CameraLayout::Pip, geometry(), Some(bubble()), 16.0 / 9.0);
        let b = resolve(
            split(0.4, LayoutSide::End, true),
            geometry(),
            None,
            16.0 / 9.0,
        );
        assert_eq!(lerp(a, b, 0.0), a);
        assert_eq!(lerp(a, b, 1.0), b);
    }

    /// A layer that appears moves toward the rect it will occupy while it fades
    /// in, rather than popping into place at full opacity.
    #[test]
    fn a_lerp_moves_the_rects_and_the_opacities_together() {
        let hidden = resolve(CameraLayout::ScreenOnly, geometry(), Some(bubble()), 1.0);
        let shown = resolve(split(0.5, LayoutSide::Start, false), geometry(), None, 1.0);
        let mid = lerp(hidden, shown, 0.5);
        assert!((mid.camera_opacity - 0.5).abs() < 1e-6);
        assert!(mid.camera.w > hidden.camera.w && mid.camera.w < shown.camera.w);
    }

    #[test]
    fn a_lerp_is_clamped_rather_than_extrapolating_past_either_end() {
        let a = resolve(CameraLayout::Pip, geometry(), Some(bubble()), 1.0);
        let b = resolve(split(0.5, LayoutSide::Start, false), geometry(), None, 1.0);
        assert_eq!(lerp(a, b, -3.0), a);
        assert_eq!(lerp(a, b, 7.0), b);
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
    fn camera_only_fills_the_canvas_and_hides_the_screen() {
        let r = resolve(
            CameraLayout::CameraOnly,
            geometry(),
            Some(bubble()),
            16.0 / 9.0,
        );
        assert_eq!(
            r.camera,
            DestRect {
                x: 0.0,
                y: 0.0,
                w: 1920.0,
                h: 1080.0
            }
        );
        assert_eq!(r.camera_opacity, 1.0);
        assert_eq!(r.screen_opacity, 0.0);
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
