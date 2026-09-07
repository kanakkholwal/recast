#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::cuts::{normalize_cuts, Cut};
use crate::segments::Segment;
use crate::EPS;

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(
    feature = "serde",
    derive(Serialize, Deserialize),
    serde(rename_all = "camelCase")
)]
pub struct TimeSpan {
    pub orig_start: f64,
    pub orig_end: f64,
    #[cfg_attr(feature = "serde", serde(default = "unit_speed"))]
    pub speed: f64,
}

#[cfg(feature = "serde")]
fn unit_speed() -> f64 {
    1.0
}

impl TimeSpan {
    pub fn new(orig_start: f64, orig_end: f64, speed: f64) -> Self {
        Self {
            orig_start,
            orig_end,
            speed,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(
    feature = "serde",
    derive(Serialize, Deserialize),
    serde(rename_all = "camelCase")
)]
pub struct MappedSpan {
    pub orig_start: f64,
    pub orig_end: f64,
    pub speed: f64,
    pub out_start: f64,
    pub out_end: f64,
}

impl MappedSpan {
    pub fn orig_duration(&self) -> f64 {
        self.orig_end - self.orig_start
    }

    pub fn out_duration(&self) -> f64 {
        self.out_end - self.out_start
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(
    feature = "serde",
    derive(Serialize, Deserialize),
    serde(rename_all = "camelCase")
)]
pub struct TimeMap {
    pub spans: Vec<MappedSpan>,
    pub output_duration: f64,
}

impl TimeMap {
    pub fn is_empty(&self) -> bool {
        self.spans.is_empty()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Region {
    pub start: f64,
    pub end: f64,
    pub speed: f64,
}

fn effective_speed(speed: f64) -> f64 {
    if speed > 0.0 && speed.is_finite() {
        speed
    } else {
        1.0
    }
}

pub fn build_time_map(spans: &[TimeSpan]) -> TimeMap {
    let mut ordered: Vec<TimeSpan> = spans
        .iter()
        .copied()
        .filter(|s| s.orig_end - s.orig_start > EPS)
        .collect();
    ordered.sort_by(|a, b| a.orig_start.total_cmp(&b.orig_start));

    let mut mapped = Vec::with_capacity(ordered.len());
    let mut out = 0.0;
    for s in ordered {
        let speed = effective_speed(s.speed);
        let out_start = out;
        out += (s.orig_end - s.orig_start) / speed;
        mapped.push(MappedSpan {
            orig_start: s.orig_start,
            orig_end: s.orig_end,
            speed,
            out_start,
            out_end: out,
        });
    }
    TimeMap {
        spans: mapped,
        output_duration: out,
    }
}

pub fn time_map_from_segments<F>(segments: &[Segment], speed_of: F) -> TimeMap
where
    F: Fn(usize) -> f64,
{
    let spans: Vec<TimeSpan> = segments
        .iter()
        .map(|seg| TimeSpan::new(seg.start, seg.end, speed_of(seg.index)))
        .collect();
    build_time_map(&spans)
}

#[derive(Debug, Clone, Default)]
pub struct DisplayAxis {
    pub trim_start: f64,
    pub trim_end: f64,
    pub duration_secs: f64,
    pub segments: Vec<Segment>,
    pub cuts: Vec<Cut>,
}

fn kept_intervals(lo: f64, hi: f64, normalized: &[Cut]) -> Vec<(f64, f64)> {
    if hi - lo <= EPS {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut cursor = lo;
    for c in normalized {
        if c.end <= lo || c.start >= hi {
            continue;
        }
        let cut_start = c.start.max(lo);
        if cut_start - cursor > EPS {
            out.push((cursor, cut_start));
        }
        cursor = cursor.max(c.end.min(hi));
    }
    if hi - cursor > EPS {
        out.push((cursor, hi));
    }
    out
}

/// The transient axis used WHILE trimming: the whole recording, with the trimmed
/// head and tail un-collapsed at 1x so a handle can drag across the full source.
pub fn display_time_map<F>(axis: &DisplayAxis, speed_of: F) -> TimeMap
where
    F: Fn(usize) -> f64,
{
    let normalized = normalize_cuts(&axis.cuts);
    let mut spans: Vec<TimeSpan> = Vec::new();
    for (start, end) in kept_intervals(0.0, axis.trim_start, &normalized) {
        spans.push(TimeSpan::new(start, end, 1.0));
    }
    for seg in &axis.segments {
        spans.push(TimeSpan::new(seg.start, seg.end, speed_of(seg.index)));
    }
    for (start, end) in kept_intervals(axis.trim_end, axis.duration_secs, &normalized) {
        spans.push(TimeSpan::new(start, end, 1.0));
    }
    build_time_map(&spans)
}

/// Re-spaces a collapsed map so removed time shows as real width. Rendering only;
/// playback and export must never read it.
pub fn build_gap_map(map: &TimeMap) -> TimeMap {
    let mut spans = Vec::with_capacity(map.spans.len());
    let mut out = 0.0;
    let mut prev_orig_end: Option<f64> = None;
    for s in &map.spans {
        if let Some(prev) = prev_orig_end {
            if s.orig_start - prev > EPS {
                out += s.orig_start - prev;
            }
        }
        let width = s.out_duration();
        spans.push(MappedSpan {
            out_start: out,
            out_end: out + width,
            ..*s
        });
        out += width;
        prev_orig_end = Some(s.orig_end);
    }
    TimeMap {
        spans,
        output_duration: out,
    }
}

pub fn original_to_output(map: &TimeMap, t: f64) -> f64 {
    let spans = &map.spans;
    let mut lo = 0usize;
    let mut hi = spans.len();
    while lo < hi {
        let mid = (lo + hi) / 2;
        if spans[mid].orig_end >= t {
            hi = mid;
        } else {
            lo = mid + 1;
        }
    }
    let Some(s) = spans.get(lo) else {
        return map.output_duration;
    };
    if t < s.orig_start {
        s.out_start
    } else {
        s.out_start + (t - s.orig_start) / s.speed
    }
}

/// Clamped to the kept range; an exact interior seam resolves to the right-hand
/// span, matching `segment_at`.
pub fn output_to_original(map: &TimeMap, t: f64) -> f64 {
    let Some(last) = map.spans.last() else {
        return 0.0;
    };
    for s in &map.spans {
        if t <= s.out_start + EPS {
            return s.orig_start;
        }
        if t < s.out_end - EPS {
            return s.orig_start + (t - s.out_start) * s.speed;
        }
    }
    last.orig_end
}

pub fn span_at_original(map: &TimeMap, t: f64) -> Option<MappedSpan> {
    if let Some(s) = map
        .spans
        .iter()
        .copied()
        .find(|s| t >= s.orig_start - EPS && t < s.orig_end - EPS)
    {
        return Some(s);
    }
    map.spans
        .last()
        .copied()
        .filter(|last| (t - last.orig_end).abs() <= EPS)
}

pub fn to_regions(map: &TimeMap) -> Vec<Region> {
    map.spans
        .iter()
        .map(|s| Region {
            start: s.orig_start,
            end: s.orig_end,
            speed: s.speed,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::segments::{derive_segments, ClipShape};

    fn span(orig_start: f64, orig_end: f64, speed: f64) -> TimeSpan {
        TimeSpan::new(orig_start, orig_end, speed)
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-6
    }

    #[test]
    fn kept_spans_lie_end_to_end_on_the_output_axis() {
        let map = build_time_map(&[span(0.0, 2.0, 1.0), span(5.0, 6.0, 1.0)]);
        assert_eq!(
            map.spans
                .iter()
                .map(|s| (s.out_start, s.out_end))
                .collect::<Vec<_>>(),
            vec![(0.0, 2.0), (2.0, 3.0)]
        );
        assert!(close(map.output_duration, 3.0));
    }

    #[test]
    fn spans_are_sorted_by_original_start() {
        let map = build_time_map(&[span(5.0, 6.0, 1.0), span(0.0, 2.0, 1.0)]);
        assert_eq!(map.spans[0].orig_start, 0.0);
    }

    #[test]
    fn zero_length_spans_are_dropped() {
        assert!(build_time_map(&[span(2.0, 2.0, 1.0)]).spans.is_empty());
    }

    #[test]
    fn a_two_times_span_occupies_half_the_output_width() {
        assert!(close(
            build_time_map(&[span(0.0, 4.0, 2.0)]).output_duration,
            2.0
        ));
    }

    #[test]
    fn a_non_positive_or_non_finite_speed_falls_back_to_one() {
        assert!(close(
            build_time_map(&[span(0.0, 4.0, 0.0)]).output_duration,
            4.0
        ));
        assert!(close(
            build_time_map(&[span(0.0, 4.0, f64::INFINITY)]).output_duration,
            4.0
        ));
    }

    fn two_span_map() -> TimeMap {
        build_time_map(&[span(0.0, 4.0, 2.0), span(6.0, 10.0, 1.0)])
    }

    #[test]
    fn each_span_applies_its_own_slope() {
        let map = two_span_map();
        assert!(close(original_to_output(&map, 2.0), 1.0));
        assert!(close(original_to_output(&map, 8.0), 4.0));
    }

    #[test]
    fn a_removed_gap_collapses_onto_the_next_seam() {
        assert!(close(original_to_output(&two_span_map(), 5.0), 2.0));
    }

    #[test]
    fn kept_times_round_trip() {
        let map = two_span_map();
        for t in [0.0, 1.0, 3.0, 6.0, 7.0, 9.0, 10.0] {
            assert!(close(
                output_to_original(&map, original_to_output(&map, t)),
                t
            ));
        }
    }

    #[test]
    fn an_exact_interior_seam_resolves_right() {
        assert!(close(output_to_original(&two_span_map(), 2.0), 6.0));
    }

    #[test]
    fn output_outside_the_kept_range_is_clamped() {
        let map = two_span_map();
        assert!(close(output_to_original(&map, -1.0), 0.0));
        assert!(close(output_to_original(&map, 99.0), 10.0));
    }

    #[test]
    fn a_fully_cut_timeline_degrades_to_zero_in_both_directions() {
        let empty = build_time_map(&[]);
        assert!(empty.is_empty());
        assert_eq!(empty.output_duration, 0.0);
        assert_eq!(original_to_output(&empty, 5.0), 0.0);
        assert_eq!(output_to_original(&empty, 5.0), 0.0);
    }

    #[test]
    fn the_projection_is_monotonic_non_decreasing() {
        let map = two_span_map();
        let mut prev = f64::NEG_INFINITY;
        for step in 0..=100 {
            let o = original_to_output(&map, step as f64 * 0.1);
            assert!(o >= prev - 1e-9);
            prev = o;
        }
    }

    #[test]
    fn the_binary_search_agrees_with_a_linear_scan_over_randomised_maps() {
        fn linear(map: &TimeMap, t: f64) -> f64 {
            for s in &map.spans {
                if t < s.orig_start {
                    return s.out_start;
                }
                if t <= s.orig_end {
                    return s.out_start + (t - s.orig_start) / s.speed;
                }
            }
            map.output_duration
        }

        let mut seed: i64 = 0x2545_f491;
        let mut rand = move || {
            seed = (seed.wrapping_mul(1_103_515_245).wrapping_add(12_345)) & 0x7fff_ffff;
            seed as f64 / 0x7fff_ffff as f64
        };

        for _ in 0..2000 {
            let n = 1 + (rand() * 12.0) as usize;
            let mut spans = Vec::new();
            let mut cursor = rand() * 3.0;
            for _ in 0..n {
                let start = cursor + rand() * 2.0;
                let len = 0.2 + rand() * 3.0;
                spans.push(span(start, start + len, 0.5 + rand() * 2.0));
                cursor = start + len;
            }
            let map = build_time_map(&spans);
            let mut probes = vec![-1.0, cursor + 1.0];
            for s in &map.spans {
                probes.extend([
                    s.orig_start,
                    s.orig_end,
                    (s.orig_start + s.orig_end) / 2.0,
                    s.orig_start - 0.01,
                ]);
            }
            for t in probes {
                assert!(
                    (original_to_output(&map, t) - linear(&map, t)).abs() < 1e-9,
                    "binary search diverged at t={t}"
                );
            }
        }
    }

    #[test]
    fn span_at_original_finds_the_covering_span_and_rejects_a_gap() {
        let map = build_time_map(&[span(0.0, 4.0, 2.0), span(6.0, 10.0, 1.0)]);
        assert_eq!(span_at_original(&map, 1.0).map(|s| s.orig_start), Some(0.0));
        assert_eq!(span_at_original(&map, 7.0).map(|s| s.orig_start), Some(6.0));
        assert!(span_at_original(&map, 5.0).is_none());
    }

    #[test]
    fn a_gap_map_gives_removed_time_real_width() {
        let collapsed = build_time_map(&[span(0.0, 2.0, 1.0), span(5.0, 7.0, 1.0)]);
        assert!(close(collapsed.output_duration, 4.0));
        let gap = build_gap_map(&collapsed);
        assert!(close(gap.output_duration, 7.0));
        assert!(close(original_to_output(&gap, 2.0), 2.0));
        assert!(close(original_to_output(&gap, 5.0), 5.0));
        assert!(close(gap.spans[0].out_duration(), 2.0));
        assert!(close(gap.spans[1].out_duration(), 2.0));
    }

    #[test]
    fn a_gap_map_is_a_no_op_when_nothing_was_removed() {
        let contiguous = build_time_map(&[span(0.0, 4.0, 1.0), span(4.0, 10.0, 1.0)]);
        let gap = build_gap_map(&contiguous);
        assert!(close(gap.output_duration, contiguous.output_duration));
        assert_eq!(
            gap.spans.iter().map(|s| s.out_start).collect::<Vec<_>>(),
            contiguous
                .spans
                .iter()
                .map(|s| s.out_start)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn a_gap_map_keeps_a_sped_span_warped() {
        let collapsed = build_time_map(&[span(0.0, 2.0, 1.0), span(4.0, 8.0, 2.0)]);
        let gap = build_gap_map(&collapsed);
        assert!(close(gap.spans[1].out_start, 4.0));
        assert!(close(gap.spans[1].out_duration(), 2.0));
        assert!(close(gap.output_duration, 6.0));
    }

    #[test]
    fn a_sped_segment_narrows_the_kept_axis() {
        let segments = derive_segments(&ClipShape {
            trim_start: 0.0,
            trim_end: 10.0,
            cuts: Vec::new(),
            split_points: vec![4.0],
        });
        let map = time_map_from_segments(&segments, |i| if i == 1 { 2.0 } else { 1.0 });
        assert!(close(map.output_duration, 7.0));
        assert!(close(original_to_output(&map, 4.0), 4.0));
        assert!(close(original_to_output(&map, 10.0), 7.0));
    }

    #[test]
    fn regions_carry_the_effective_speed_and_drop_empty_segments() {
        let segments = [
            Segment {
                start: 0.0,
                end: 4.0,
                index: 0,
            },
            Segment {
                start: 4.0,
                end: 4.0,
                index: 1,
            },
        ];
        let map = time_map_from_segments(&segments, |_| 0.0);
        assert_eq!(
            to_regions(&map),
            vec![Region {
                start: 0.0,
                end: 4.0,
                speed: 1.0
            }]
        );
    }

    #[test]
    fn the_kept_axis_stays_inside_the_trim_while_the_display_axis_spans_the_source() {
        let shape = ClipShape {
            trim_start: 5.0,
            trim_end: 15.0,
            cuts: Vec::new(),
            split_points: Vec::new(),
        };
        let segments = derive_segments(&shape);
        let kept = time_map_from_segments(&segments, |_| 1.0);
        assert_eq!(
            to_regions(&kept),
            vec![Region {
                start: 5.0,
                end: 15.0,
                speed: 1.0
            }]
        );
        assert!(close(kept.output_duration, 10.0));

        let display = display_time_map(
            &DisplayAxis {
                trim_start: 5.0,
                trim_end: 15.0,
                duration_secs: 30.0,
                segments,
                cuts: Vec::new(),
            },
            |_| 1.0,
        );
        let regions = to_regions(&display);
        assert_eq!(regions[0].start, 0.0);
        assert_eq!(regions[regions.len() - 1].end, 30.0);
    }
}
