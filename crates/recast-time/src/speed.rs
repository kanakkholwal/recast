#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::segments::Segment;
use crate::EPS;

pub const MIN_SEGMENT_SPEED: f64 = 0.25;
pub const MAX_SEGMENT_SPEED: f64 = 4.0;

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct SegmentSpeed {
    pub start: f64,
    pub speed: f64,
}

pub fn clamp_speed(speed: f64) -> f64 {
    if !speed.is_finite() || speed <= 0.0 {
        1.0
    } else {
        speed.clamp(MIN_SEGMENT_SPEED, MAX_SEGMENT_SPEED)
    }
}

pub fn segment_speed_at(overrides: &[SegmentSpeed], start: f64) -> f64 {
    overrides
        .iter()
        .find(|o| (o.start - start).abs() <= EPS)
        .map(|o| o.speed)
        .unwrap_or(1.0)
}

pub fn segment_speed_at_time(segments: &[Segment], overrides: &[SegmentSpeed], t: f64) -> f64 {
    if let Some(seg) = segments
        .iter()
        .find(|s| t >= s.start - EPS && t < s.end - EPS)
    {
        return segment_speed_at(overrides, seg.start);
    }
    segments
        .last()
        .map(|last| segment_speed_at(overrides, last.start))
        .unwrap_or(1.0)
}

pub fn set_segment_speed(overrides: &[SegmentSpeed], start: f64, speed: f64) -> Vec<SegmentSpeed> {
    let clamped = clamp_speed(speed);
    let mut rest: Vec<SegmentSpeed> = overrides
        .iter()
        .copied()
        .filter(|o| (o.start - start).abs() > EPS)
        .collect();
    if (clamped - 1.0).abs() > EPS {
        rest.push(SegmentSpeed {
            start,
            speed: clamped,
        });
    }
    rest.sort_by(|a, b| a.start.total_cmp(&b.start));
    rest
}

pub fn prune_segment_speeds(overrides: &[SegmentSpeed], segments: &[Segment]) -> Vec<SegmentSpeed> {
    overrides
        .iter()
        .copied()
        .filter(|o| segments.iter().any(|s| (s.start - o.start).abs() <= EPS))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seg(start: f64, end: f64, index: usize) -> Segment {
        Segment { start, end, index }
    }

    #[test]
    fn a_non_positive_or_non_finite_speed_resets_to_one() {
        assert_eq!(clamp_speed(0.0), 1.0);
        assert_eq!(clamp_speed(-2.0), 1.0);
        assert_eq!(clamp_speed(f64::NAN), 1.0);
        assert_eq!(clamp_speed(f64::INFINITY), 1.0);
    }

    #[test]
    fn speed_is_clamped_to_the_supported_range() {
        assert_eq!(clamp_speed(0.1), MIN_SEGMENT_SPEED);
        assert_eq!(clamp_speed(9.0), MAX_SEGMENT_SPEED);
    }

    #[test]
    fn an_anchor_matches_its_segment_within_eps() {
        let o = [SegmentSpeed {
            start: 4.0,
            speed: 2.0,
        }];
        assert_eq!(segment_speed_at(&o, 4.000_01), 2.0);
        assert_eq!(segment_speed_at(&o, 4.5), 1.0);
    }

    #[test]
    fn speed_at_time_is_forward_biased_and_holds_at_the_end() {
        let segs = [seg(0.0, 4.0, 0), seg(4.0, 10.0, 1)];
        let o = [SegmentSpeed {
            start: 4.0,
            speed: 2.0,
        }];
        assert_eq!(segment_speed_at_time(&segs, &o, 4.0), 2.0);
        assert_eq!(segment_speed_at_time(&segs, &o, 10.0), 2.0);
        assert_eq!(segment_speed_at_time(&segs, &o, 1.0), 1.0);
    }

    #[test]
    fn setting_a_speed_back_to_one_removes_the_entry() {
        let o = set_segment_speed(&[], 4.0, 2.0);
        assert_eq!(o.len(), 1);
        assert!(set_segment_speed(&o, 4.0, 1.0).is_empty());
    }

    #[test]
    fn setting_a_speed_stores_the_clamped_value_sorted() {
        let o = set_segment_speed(&set_segment_speed(&[], 8.0, 9.0), 2.0, 0.5);
        assert_eq!(o[0].start, 2.0);
        assert_eq!(o[1].speed, MAX_SEGMENT_SPEED);
    }

    #[test]
    fn orphaned_anchors_are_pruned() {
        let o = [
            SegmentSpeed {
                start: 0.0,
                speed: 2.0,
            },
            SegmentSpeed {
                start: 7.5,
                speed: 2.0,
            },
        ];
        let kept = prune_segment_speeds(&o, &[seg(0.0, 4.0, 0)]);
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].start, 0.0);
    }
}
