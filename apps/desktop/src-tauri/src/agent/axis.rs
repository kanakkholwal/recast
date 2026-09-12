//! The one place the harness converts between the recording's clock and the output clock.
//! Every agent-facing number is output seconds; every stored number is source seconds. Tools convert, the model never does.

use recast_time::{original_to_output, output_to_original, TimeMap};

use crate::render::graph::RenderState;

/// Below this an output span is treated as not visible at all.
const VISIBLE_EPS: f64 = 1e-3;

/// The time map the preview and export both derive from this state.
pub fn time_map(state: &RenderState) -> TimeMap {
    recast_scene::migrate::to_scene(state).timeline.time_map()
}

/// Where a source span lands on the output axis, or `None` when cuts or the trim remove all of it.
pub fn output_span(map: &TimeMap, start: f64, end: f64) -> Option<(f64, f64)> {
    let out_start = original_to_output(map, start);
    let out_end = original_to_output(map, end);
    (out_end - out_start > VISIBLE_EPS).then_some((out_start, out_end))
}

/// The source second that plays at output second `t`.
pub fn to_source(map: &TimeMap, t: f64) -> f64 {
    output_to_original(map, t)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::graph::CutRange;

    fn state(cuts: &[(f64, f64)]) -> RenderState {
        RenderState {
            trim_start: 0.0,
            trim_end: 60.0,
            cuts: cuts
                .iter()
                .map(|&(start, end)| CutRange {
                    start,
                    end,
                    extra: Default::default(),
                })
                .collect(),
            ..RenderState::default()
        }
    }

    #[test]
    fn a_span_inside_a_cut_is_not_visible() {
        let map = time_map(&state(&[(10.0, 20.0)]));
        assert_eq!(output_span(&map, 12.0, 18.0), None);
    }

    #[test]
    fn a_span_after_a_cut_shifts_left_by_the_cut() {
        let map = time_map(&state(&[(10.0, 20.0)]));
        let (s, e) = output_span(&map, 30.0, 35.0).unwrap();
        assert!((s - 20.0).abs() < 1e-9);
        assert!((e - 25.0).abs() < 1e-9);
    }

    #[test]
    fn a_span_straddling_a_cut_keeps_only_its_kept_part() {
        let map = time_map(&state(&[(10.0, 20.0)]));
        let (s, e) = output_span(&map, 5.0, 25.0).unwrap();
        assert!((s - 5.0).abs() < 1e-9);
        assert!((e - 15.0).abs() < 1e-9);
    }

    #[test]
    fn output_and_source_round_trip_on_kept_material() {
        let map = time_map(&state(&[(10.0, 20.0)]));
        let source = to_source(&map, 25.0);
        assert!((source - 35.0).abs() < 1e-9);
        assert!((original_to_output(&map, source) - 25.0).abs() < 1e-9);
    }
}
