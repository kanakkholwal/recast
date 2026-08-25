use recast_cursor::{
    build_press_events_from_iter, click_anchor_at, click_highlight_at, idle_alpha_at,
    interpolate_at, press_state_at, smooth_cursor_path, CursorSample, CursorSettings, CursorTrack,
    IdlePeriod, PressEvent, SmoothingOptions,
};

/// The same file drives `packages/editor/src/lib/editor/cursor-parity.test.ts`.
/// If the two disagree, the preview and the export put the cursor in different
/// places, which is the failure this pair exists to make impossible.
const FIXTURE: &str =
    include_str!("../../../packages/editor/src/lib/editor/__fixtures__/cursor-parity.json");

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct Fixture {
    source: Source,
    samples: Vec<CursorSample>,
    idle_periods: Vec<IdlePeriod>,
    smoothing: Smoothing,
    settings: Settings,
}

#[derive(serde::Deserialize)]
struct Source {
    width: u32,
    height: u32,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct Smoothing {
    sigma_ms: f64,
    snap_to_clicks: bool,
    snap_window_ms: f64,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct Settings {
    hide_when_idle: bool,
    idle_timeout: f64,
    highlight_clicks: bool,
    highlight_opacity: f64,
}

fn fixture() -> Fixture {
    serde_json::from_str(FIXTURE).expect("cursor parity fixture")
}

fn smoothed(f: &Fixture) -> Vec<CursorSample> {
    smooth_cursor_path(
        &f.samples,
        SmoothingOptions {
            sigma_ms: f.smoothing.sigma_ms,
            snap_to_clicks: f.smoothing.snap_to_clicks,
            snap_window_ms: f.smoothing.snap_window_ms,
        },
    )
    .samples
}

fn presses(f: &Fixture) -> Vec<PressEvent> {
    build_press_events_from_iter(
        f.samples
            .iter()
            .map(|s| (s.timestamp_us, s.x, s.y, s.left_down, s.right_down)),
    )
}

fn at(samples: &[CursorSample], us: u64) -> CursorSample {
    *samples
        .iter()
        .find(|s| s.timestamp_us == us)
        .unwrap_or_else(|| panic!("no smoothed sample at {us}"))
}

fn close(got: f64, want: f64) {
    assert!((got - want).abs() < 1e-6, "got {got}, want {want}");
}

#[test]
fn smoothing_pulls_each_sample_toward_its_gaussian_neighbourhood() {
    let f = fixture();
    let s = smoothed(&f);
    close(at(&s, 0).x, 158.197716913);
    close(at(&s, 0).y, 116.231687949);
    close(at(&s, 208_000).x, 455.892554210);
    close(at(&s, 208_000).y, 268.880114936);
}

/// Without the snap, smoothing rounds the corner through a click and the
/// pointer lands somewhere the user never clicked.
#[test]
fn the_smoothed_path_is_anchored_exactly_onto_every_click_position() {
    let f = fixture();
    let s = smoothed(&f);
    assert_eq!((at(&s, 80_000).x, at(&s, 80_000).y), (300.0, 160.0));
    assert_eq!((at(&s, 176_000).x, at(&s, 176_000).y), (455.0, 268.0));
}

#[test]
fn a_run_of_identical_samples_survives_smoothing_untouched() {
    let f = fixture();
    let s = smoothed(&f);
    assert_eq!((at(&s, 400_000).x, at(&s, 400_000).y), (480.0, 288.0));
}

#[test]
fn each_rising_edge_pairs_with_its_release_and_the_button_is_classified() {
    let f = fixture();
    let events = presses(&f);
    assert_eq!(
        events,
        vec![
            PressEvent {
                down_us: 80_000,
                up_us: 112_000,
                down_x: 300.0,
                down_y: 160.0,
                right: false,
                dragged: false,
            },
            PressEvent {
                down_us: 176_000,
                up_us: 208_000,
                down_x: 455.0,
                down_y: 268.0,
                right: true,
                dragged: true,
            },
        ]
    );
}

#[test]
fn the_press_is_telegraphed_before_the_click_lands() {
    let f = fixture();
    let events = presses(&f);
    let state = press_state_at(0, &events);
    assert!(state.pressed_sprite);
    close(state.scale, 1.015743440);
    let (x, y, weight) = click_anchor_at(0, &events).expect("an anchor");
    assert_eq!((x, y), (300.0, 160.0));
    close(weight, 0.654508497);
}

#[test]
fn the_sprite_snaps_to_the_punch_scale_on_the_click_frame() {
    let f = fixture();
    let events = presses(&f);
    close(press_state_at(80_000, &events).scale, 0.84);
    close(press_state_at(200_000, &events).scale, 0.845872576);
}

/// The highlight starts at zero on the impact frame and ramps in over 40 ms, so
/// asserting "visible at the click" would pass on a broken envelope.
#[test]
fn the_click_highlight_fades_in_from_nothing_at_the_click() {
    let f = fixture();
    let events = presses(&f);
    close(
        click_highlight_at(80_000, &events).expect("highlight").2,
        0.0,
    );
    close(
        click_highlight_at(88_000, &events).expect("highlight").2,
        0.104,
    );
    close(
        click_highlight_at(130_000, &events).expect("highlight").2,
        1.0,
    );
    assert!(click_highlight_at(900_000, &events).is_none());
}

#[test]
fn a_press_keeps_the_cursor_visible_even_once_idle_hide_has_reached_zero() {
    let f = fixture();
    let events = presses(&f);
    close(
        idle_alpha_at(&f.idle_periods, 560_000, f.settings.idle_timeout),
        0.82,
    );
    let alpha = |ts: i64| {
        idle_alpha_at(&f.idle_periods, ts, f.settings.idle_timeout)
            .max(press_state_at(ts, &events).visible_alpha)
    };
    close(alpha(560_000), 1.0);
    close(alpha(900_000), 0.797849108);
}

/// Booleans flip at the midpoint of the LINEAR parameter, so an invisible sample
/// keeps the cursor hidden for the first half of its span.
#[test]
fn visibility_comes_from_the_nearer_sample_rather_than_being_interpolated() {
    let f = fixture();
    let s = smoothed(&f);
    assert!(!interpolate_at(&s, 165_000, |t| t).expect("sample").visible);
    assert!(interpolate_at(&s, 200_000, |t| t).expect("sample").visible);
}

#[test]
fn position_is_interpolated_between_captured_samples() {
    let f = fixture();
    let s = smoothed(&f);
    let got = interpolate_at(&s, 24_000, |t| t).expect("sample");
    close(got.x, 208.926806144);
    close(got.y, 132.126505975);
}

#[test]
fn the_anchor_is_dropped_once_the_snap_window_has_passed() {
    let f = fixture();
    assert!(click_anchor_at(560_000, &presses(&f)).is_none());
}

/// The whole point of the crate: one call the compositor can make per frame that
/// reproduces what the TypeScript preview assembled from five separate helpers.
#[test]
fn the_track_resolves_a_whole_frame_in_one_call() {
    let f = fixture();
    let track = CursorTrack::new(smoothed(&f), f.idle_periods.clone());
    let settings = CursorSettings {
        hide_when_idle: f.settings.hide_when_idle,
        idle_timeout: f.settings.idle_timeout,
        highlight_clicks: f.settings.highlight_clicks,
        highlight_opacity: f.settings.highlight_opacity,
    };
    let source = (f.source.width, f.source.height);

    let placed = track
        .resolve(88_000, source, settings, |t| t)
        .expect("a placement");
    // 8 ms after the click the anchor weight is 0.996, not 1, so the position
    // is very near the click target without being exactly on it.
    close(placed.x, 299.999786 / 1920.0);
    close(placed.y, 160.001497 / 1080.0);
    close(placed.alpha, 1.0);
    assert!(placed.pressed);
    assert!(!placed.right);
    let highlight = placed.highlight.expect("a highlight");
    close(highlight.x, 300.0 / 1920.0);
    close(highlight.alpha, 0.4 * 0.104);
}

/// `press_events` is derived rather than stored, so a track that came back from
/// JSON would have no clicks at all unless the rebuild runs.
#[test]
fn a_deserialised_track_rebuilds_its_press_events() {
    let f = fixture();
    let track = CursorTrack::new(f.samples.clone(), f.idle_periods.clone());
    let json = serde_json::to_string(&track).expect("serialize");
    let mut back: CursorTrack = serde_json::from_str(&json).expect("deserialize");
    assert!(back.press_events().is_empty());
    back.rebuild_press_events();
    assert_eq!(back.press_events(), track.press_events());
}
