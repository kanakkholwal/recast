//! Intent-level verbs built from ops: the agent says what it wants on the output clock and gets the raw ops it would otherwise have to derive.
//! Pure over state and time map; the socket arm supplies ids and appends the result, so a journal replays without this module.

use recast_time::TimeMap;
use serde_json::{json, Map};

use super::axis::to_source;
use crate::render::graph::RenderState;
use crate::render::node_types::ZoomRegion;
use crate::render::ops::Op;
use crate::silence::SilenceSegment;
use recast_scene::v1::Easing;

/// Same bounds the validator enforces, restated here so a refused intent names them.
const ZOOM_SCALE: std::ops::RangeInclusive<f64> = 1.0..=3.0;
/// Two cuts closer than this are merged rather than left as a sliver of speech.
const MERGE_GAP_SECS: f64 = 0.1;

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum IntentError {
    #[error("{what} must be finite seconds")]
    NotFinite { what: &'static str },
    #[error("duration must be positive, got {0}")]
    NonPositiveDuration(f64),
    #[error("scale {0} is outside {ZOOM_SCALE:?}")]
    ScaleOutOfRange(f64),
    #[error("center ({0}, {1}) is outside the 0..1 video rect")]
    CenterOutOfRange(f64, f64),
    #[error("output second {0} is past the end of the output ({1:.3}s)")]
    PastEnd(f64, f64),
}

/// How to turn silences into cuts.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SilencePolicy {
    /// Silences shorter than this after padding are left alone.
    pub min_duration: f64,
    /// Seconds of the silence kept at each end so speech is never clipped.
    pub pad: f64,
    /// Candidates below this confidence are ignored.
    pub min_confidence: f32,
}

impl Default for SilencePolicy {
    fn default() -> Self {
        Self {
            min_duration: 0.8,
            pad: 0.15,
            min_confidence: 0.5,
        }
    }
}

/// The cuts that remove `silences` under `policy`, skipping any range already cut and clamping to the trim. Source axis, as ops are.
pub fn cuts_for_silences(
    state: &RenderState,
    silences: &[SilenceSegment],
    policy: SilencePolicy,
) -> Vec<Op> {
    let mut ranges: Vec<(f64, f64)> = silences
        .iter()
        .filter(|s| s.confidence >= policy.min_confidence)
        .map(|s| (s.start + policy.pad, s.end - policy.pad))
        .map(|(s, e)| (s.max(state.trim_start), e.min(state.trim_end)))
        .filter(|(s, e)| e - s >= policy.min_duration)
        .filter(|&(s, e)| !state.cuts.iter().any(|c| s < c.end && e > c.start))
        .collect();
    ranges.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut merged: Vec<(f64, f64)> = Vec::new();
    for (s, e) in ranges {
        match merged.last_mut() {
            Some(last) if s - last.1 <= MERGE_GAP_SECS => last.1 = last.1.max(e),
            _ => merged.push((s, e)),
        }
    }
    merged
        .into_iter()
        .map(|(start, end)| Op::CutAdd { start, end })
        .collect()
}

/// A zoom the agent describes on the output clock.
#[derive(Debug, Clone, PartialEq)]
pub struct ZoomIntent {
    pub id: String,
    /// Output seconds the zoom starts.
    pub at: f64,
    /// Output seconds it lasts.
    pub duration: f64,
    pub center_x: f64,
    pub center_y: f64,
    pub scale: f64,
    /// Seconds to ramp in and out; clamped to half the duration.
    pub ramp: f64,
}

/// The op that adds the zoom, with times converted to the source clock and every default filled.
/// # Errors On a non-finite, out-of-range or past-the-end intent.
pub fn zoom_op(map: &TimeMap, intent: &ZoomIntent) -> Result<Op, IntentError> {
    for (what, v) in [
        ("at", intent.at),
        ("duration", intent.duration),
        ("ramp", intent.ramp),
    ] {
        if !v.is_finite() {
            return Err(IntentError::NotFinite { what });
        }
    }
    if intent.duration <= 0.0 {
        return Err(IntentError::NonPositiveDuration(intent.duration));
    }
    if !ZOOM_SCALE.contains(&intent.scale) {
        return Err(IntentError::ScaleOutOfRange(intent.scale));
    }
    if !(0.0..=1.0).contains(&intent.center_x) || !(0.0..=1.0).contains(&intent.center_y) {
        return Err(IntentError::CenterOutOfRange(
            intent.center_x,
            intent.center_y,
        ));
    }
    let out_end = intent.at + intent.duration;
    if intent.at >= map.output_duration {
        return Err(IntentError::PastEnd(intent.at, map.output_duration));
    }
    let start = to_source(map, intent.at);
    let end = to_source(map, out_end.min(map.output_duration));
    let ramp = intent.ramp.max(0.0).min(intent.duration / 2.0);
    let mut extra = Map::new();
    extra.insert("id".into(), json!(intent.id));
    extra.insert("source".into(), json!("manual"));
    Ok(Op::ZoomAdd {
        region: Box::new(ZoomRegion {
            start,
            end,
            scale: intent.scale,
            ease_in: Easing::default(),
            ease_out: Easing::default(),
            ramp_in: ramp,
            ramp_out: ramp,
            center_x: intent.center_x,
            center_y: intent.center_y,
            hidden: false,
            motion_blur: 0.0,
            extra,
        }),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::axis::time_map;
    use crate::render::graph::CutRange;

    fn state(cuts: &[(f64, f64)]) -> RenderState {
        RenderState {
            trim_start: 1.0,
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

    fn silence(start: f64, end: f64, confidence: f32) -> SilenceSegment {
        SilenceSegment {
            start,
            end,
            confidence,
            mic_silent: true,
            system_silent: false,
            cursor_idle: false,
        }
    }

    fn ranges(ops: &[Op]) -> Vec<(f64, f64)> {
        ops.iter()
            .map(|op| match op {
                Op::CutAdd { start, end } => (*start, *end),
                other => panic!("unexpected {other:?}"),
            })
            .collect()
    }

    #[test]
    fn silences_are_padded_inward_and_short_ones_dropped() {
        let ops = cuts_for_silences(
            &state(&[]),
            &[silence(10.0, 12.0, 0.9), silence(20.0, 20.9, 0.9)],
            SilencePolicy::default(),
        );
        let got = ranges(&ops);
        assert_eq!(got.len(), 1);
        assert!((got[0].0 - 10.15).abs() < 1e-9);
        assert!((got[0].1 - 11.85).abs() < 1e-9);
    }

    #[test]
    fn a_silence_overlapping_an_existing_cut_is_skipped() {
        let ops = cuts_for_silences(
            &state(&[(10.0, 11.0)]),
            &[silence(10.5, 14.0, 0.9)],
            SilencePolicy::default(),
        );
        assert!(ops.is_empty());
    }

    #[test]
    fn low_confidence_is_ignored_and_the_trim_clamps() {
        let policy = SilencePolicy {
            min_confidence: 0.8,
            ..SilencePolicy::default()
        };
        let ops = cuts_for_silences(
            &state(&[]),
            &[silence(0.0, 3.0, 0.9), silence(30.0, 33.0, 0.3)],
            policy,
        );
        let got = ranges(&ops);
        assert_eq!(got.len(), 1);
        assert!((got[0].0 - 1.0).abs() < 1e-9);
    }

    /// Padding protects the speech between two detections, so only ranges that still touch after padding merge.
    #[test]
    fn overlapping_silences_merge_and_separated_ones_stay_apart() {
        let merged = cuts_for_silences(
            &state(&[]),
            &[silence(10.0, 12.5, 0.9), silence(12.0, 14.0, 0.9)],
            SilencePolicy::default(),
        );
        let got = ranges(&merged);
        assert_eq!(got.len(), 1);
        assert!((got[0].0 - 10.15).abs() < 1e-9);
        assert!((got[0].1 - 13.85).abs() < 1e-9);

        let apart = cuts_for_silences(
            &state(&[]),
            &[silence(10.0, 12.0, 0.9), silence(12.25, 14.0, 0.9)],
            SilencePolicy::default(),
        );
        assert_eq!(ranges(&apart).len(), 2);
    }

    fn intent() -> ZoomIntent {
        ZoomIntent {
            id: "z9".into(),
            at: 15.0,
            duration: 4.0,
            center_x: 0.3,
            center_y: 0.4,
            scale: 2.0,
            ramp: 5.0,
        }
    }

    #[test]
    fn a_zoom_on_the_output_clock_lands_on_the_source_clock_past_a_cut() {
        let s = state(&[(10.0, 20.0)]);
        let Op::ZoomAdd { region } = zoom_op(&time_map(&s), &intent()).unwrap() else {
            panic!("not a zoom");
        };
        assert!((region.start - 26.0).abs() < 1e-9);
        assert!((region.end - 30.0).abs() < 1e-9);
        assert!(
            (region.ramp_in - 2.0).abs() < 1e-9,
            "ramp clamps to half the duration"
        );
        assert_eq!(region.extra["id"], json!("z9"));
    }

    #[test]
    fn a_zoom_past_the_end_or_out_of_range_is_refused_by_name() {
        let s = state(&[]);
        let map = time_map(&s);
        let past = ZoomIntent {
            at: 500.0,
            ..intent()
        };
        assert!(matches!(
            zoom_op(&map, &past),
            Err(IntentError::PastEnd(..))
        ));
        let big = ZoomIntent {
            scale: 9.0,
            ..intent()
        };
        assert_eq!(
            zoom_op(&map, &big).unwrap_err(),
            IntentError::ScaleOutOfRange(9.0)
        );
        let off = ZoomIntent {
            center_x: 1.5,
            ..intent()
        };
        assert!(matches!(
            zoom_op(&map, &off),
            Err(IntentError::CenterOutOfRange(..))
        ));
        let nan = ZoomIntent {
            at: f64::NAN,
            ..intent()
        };
        assert_eq!(
            zoom_op(&map, &nan).unwrap_err(),
            IntentError::NotFinite { what: "at" }
        );
    }
}
