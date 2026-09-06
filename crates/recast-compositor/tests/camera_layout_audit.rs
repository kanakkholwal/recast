//! Regressions from the 2026-09-04 camera/cursor audit. Each one failed before
//! the fix and names the defect it pins.

use recast_compositor::{Evaluator, FrameParams, LayerParams, SourceGeometry};
use recast_cursor::{CursorSample, CursorTrack};
use recast_scene::migrate::to_scene;
use recast_scene::v1::nodes::{CameraClipLayout, CameraLayout, LayoutSide};
use recast_scene::v1::RenderState;
use recast_scene::{LayerSource, Scene};
use recast_time::Segment;

fn source() -> SourceGeometry {
    SourceGeometry {
        width: 1920,
        height: 1080,
    }
}

fn sample(us: u64, x: f64, y: f64) -> CursorSample {
    CursorSample {
        timestamp_us: us,
        x,
        y,
        visible: true,
        left_down: false,
        right_down: false,
    }
}

/// A layout is authored on a clip, so every anchor past the start gets one.
fn state(camera_enabled: bool, layouts: Vec<CameraClipLayout>) -> RenderState {
    let mut state = RenderState {
        trim_end: 10.0,
        split_points: layouts
            .iter()
            .map(|c| c.start)
            .filter(|start| *start > 0.0)
            .collect(),
        ..Default::default()
    };
    state.camera_overlay.enabled = camera_enabled;
    state.camera_overlay.clip_layouts = layouts;
    state
}

fn scene(camera_enabled: bool, layouts: Vec<CameraClipLayout>, cursor: bool) -> Scene {
    let mut state = state(camera_enabled, layouts);
    state.cursor_enabled = cursor;
    let mut scene = to_scene(&state);
    if cursor {
        scene.cursor_track = Some(CursorTrack::new(
            vec![
                sample(0, 960.0, 540.0),
                sample(1_000_000, 960.0, 540.0),
                sample(2_000_000, 960.0, 540.0),
            ],
            Vec::new(),
        ));
    }
    scene
}

fn layer_of(params: &FrameParams, scene: &Scene, camera: bool) -> LayerParams {
    let id = scene
        .layers
        .iter()
        .find(|l| match &l.source {
            LayerSource::Camera(_) => camera,
            LayerSource::Screen => !camera,
            _ => false,
        })
        .expect("layer")
        .id;
    params
        .layers
        .iter()
        .find(|p| p.id == id)
        .expect("evaluated")
        .clone()
}

fn at(scene: &Scene, t: f64) -> FrameParams {
    Evaluator::new(scene, source()).evaluate(scene, t)
}

fn clip(start: f64, layout: CameraLayout) -> Vec<CameraClipLayout> {
    vec![CameraClipLayout { start, layout }]
}

fn split(fraction: f64) -> CameraLayout {
    CameraLayout::SplitH {
        fraction,
        side: LayoutSide::Start,
    }
}

/// CAM-1. `to_scene` hides the camera layer rather than removing it, so the
/// layout kept driving the frame after the camera was switched off. Camera-only
/// then rendered as background alone.
#[test]
fn a_camera_that_is_switched_off_gives_the_screen_the_whole_frame_back() {
    let s = scene(false, clip(0.0, CameraLayout::CameraOnly), false);
    let p = at(&s, 1.0);
    assert!(!layer_of(&p, &s, true).visible, "camera should be off");
    let screen = layer_of(&p, &s, false);
    assert!(
        screen.visible && screen.opacity > 0.0,
        "the screen vanished with the camera off: opacity {}",
        screen.opacity
    );
}

/// CAM-1, the other half: a split left the screen letterboxed with a dead gap
/// where the camera would have been.
#[test]
fn a_camera_that_is_switched_off_does_not_reserve_half_the_frame() {
    let s = scene(false, clip(0.0, split(0.4)), false);
    let plain = scene(false, Vec::new(), false);
    assert_eq!(
        layer_of(&at(&s, 1.0), &s, false).dest,
        layer_of(&at(&plain, 1.0), &plain, false).dest,
        "a disabled camera still reserved half the frame"
    );
}

/// CAM-2. The pointer is anchored to the screen card, which `evaluate` captured
/// without checking whether the screen was drawn: camera-only put a full-alpha
/// pointer over the face.
#[test]
fn the_pointer_goes_with_the_screen_a_layout_hid() {
    let s = scene(true, clip(0.0, CameraLayout::CameraOnly), true);
    let p = at(&s, 1.0);
    assert!(!layer_of(&p, &s, false).visible, "screen should be hidden");
    assert!(
        p.cursor_draw.is_none(),
        "the pointer was drawn over a hidden screen: {:?}",
        p.cursor_draw
    );
}

/// The same rule mid-move: the pointer fades with the screen rather than
/// staying opaque until the frame it disappears on.
#[test]
fn the_pointer_fades_with_a_screen_that_is_on_its_way_out() {
    let mut st = state(
        true,
        vec![
            CameraClipLayout {
                start: 0.0,
                layout: CameraLayout::Pip,
            },
            CameraClipLayout {
                start: 4.0,
                layout: CameraLayout::CameraOnly,
            },
        ],
    );
    st.cursor_enabled = true;
    st.camera_overlay.layout_transition = 1.0;
    let mut s = to_scene(&st);
    s.cursor_track = Some(CursorTrack::new(
        vec![sample(0, 960.0, 540.0), sample(9_000_000, 960.0, 540.0)],
        Vec::new(),
    ));
    let full = at(&s, 1.0).cursor_draw.expect("a pointer before the move");
    let mid = at(&s, 4.5).cursor_draw.expect("a pointer during the move");
    assert!(
        mid.alpha > 0.0 && mid.alpha < full.alpha,
        "the pointer did not fade with the screen: {} then {}",
        full.alpha,
        mid.alpha
    );
}

/// CAM-3. Rounding was zeroed the moment a layout applied, so a one-second move
/// out of the bubble squared its corners in a single frame.
#[test]
fn the_bubble_keeps_its_rounding_while_it_eases_into_a_split() {
    let mut st = state(
        true,
        vec![
            CameraClipLayout {
                start: 0.0,
                layout: CameraLayout::Pip,
            },
            CameraClipLayout {
                start: 4.0,
                layout: split(0.35),
            },
        ],
    );
    st.camera_overlay.layout_transition = 1.0;
    let s = to_scene(&st);
    let before = layer_of(&at(&s, 3.9), &s, true);
    let just_after = layer_of(&at(&s, 4.02), &s, true);
    let arrived = layer_of(&at(&s, 5.5), &s, true);
    assert!(before.corner_radius > 0.0, "the bubble was never rounded");
    assert!(
        just_after.corner_radius > before.corner_radius * 0.5,
        "rounding snapped from {} to {} in 20ms of a 1s move",
        before.corner_radius,
        just_after.corner_radius
    );
    assert_eq!(
        arrived.corner_radius, 0.0,
        "a half of the frame is still rounded once the move has finished"
    );
    assert!(
        arrived.shadow.is_none(),
        "a half of the frame is still casting a shadow"
    );
}

/// CAM-4. `neighbours` keyed on raw time while the editor keyed on segment
/// starts, so a cut that orphaned an anchor left the timeline saying "Bubble"
/// over a frame the engine was still splitting.
#[test]
fn an_anchor_no_clip_starts_at_stops_applying() {
    let segments = [
        Segment {
            start: 0.0,
            end: 3.0,
            index: 0,
        },
        Segment {
            start: 4.0,
            end: 9.0,
            index: 1,
        },
    ];
    let (to, _, _) = recast_compositor::layout::neighbours(
        &[CameraClipLayout {
            start: 5.0,
            layout: CameraLayout::CameraOnly,
        }],
        &segments,
        7.0,
    );
    assert_eq!(
        to,
        CameraLayout::Pip,
        "an anchor on no segment start still applied"
    );
}

/// CAM-5. The previous clip was picked by scan order, so a document that held
/// its anchors unsorted eased out of the wrong arrangement.
#[test]
fn the_previous_layout_does_not_depend_on_the_order_clips_were_written() {
    let segments = [
        Segment {
            start: 0.0,
            end: 4.0,
            index: 0,
        },
        Segment {
            start: 4.0,
            end: 9.0,
            index: 1,
        },
    ];
    let sorted = [
        CameraClipLayout {
            start: 0.0,
            layout: CameraLayout::CameraOnly,
        },
        CameraClipLayout {
            start: 4.0,
            layout: CameraLayout::ScreenOnly,
        },
    ];
    let unsorted = [sorted[1], sorted[0]];
    assert_eq!(
        recast_compositor::layout::neighbours(&sorted, &segments, 5.0).1,
        recast_compositor::layout::neighbours(&unsorted, &segments, 5.0).1,
        "clip order changed which layout the transition eases out of"
    );
}

/// CAM-9. `CursorTrack::resolve` reports a pointer the frame does not draw at
/// (0, 0) so its click ring can still fade out there. The dodge read that
/// corner as a position and shoved a corner-flush bubble aside for nothing.
#[test]
fn a_pointer_the_frame_does_not_draw_does_not_push_the_bubble() {
    let track = || {
        let mut clicked = sample(0, 0.0, 0.0);
        clicked.left_down = true;
        CursorTrack::new(
            vec![
                clicked,
                sample(150_000, 0.0, 0.0),
                CursorSample {
                    visible: false,
                    ..sample(300_000, 0.0, 0.0)
                },
                CursorSample {
                    visible: false,
                    ..sample(9_000_000, 0.0, 0.0)
                },
            ],
            Vec::new(),
        )
    };
    let bubble_at = |dodge: bool| {
        let mut st = state(true, Vec::new());
        st.cursor_enabled = true;
        st.cursor_highlight_clicks = true;
        st.camera_overlay.cursor_dodge = dodge;
        st.camera_overlay.default_placement.x = 0.0;
        st.camera_overlay.default_placement.y = 0.0;
        let mut s = to_scene(&st);
        s.cursor_track = Some(track());
        let params = at(&s, 0.35);
        assert!(
            params.cursor_draw.is_none_or(|c| c.alpha <= 0.0),
            "the fixture still draws a pointer, so there is nothing to prove"
        );
        layer_of(&params, &s, true).dest
    };
    assert_eq!(
        bubble_at(true),
        bubble_at(false),
        "a pointer the frame does not draw still moved the bubble"
    );
}
