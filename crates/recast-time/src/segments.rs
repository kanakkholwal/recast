#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::cuts::{normalize_cuts, Cut};
use crate::EPS;

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Segment {
    pub start: f64,
    pub end: f64,
    pub index: usize,
}

impl Segment {
    pub fn duration(&self) -> f64 {
        self.end - self.start
    }
}

#[derive(Debug, Clone, Default)]
pub struct ClipShape {
    pub trim_start: f64,
    pub trim_end: f64,
    pub cuts: Vec<Cut>,
    pub split_points: Vec<f64>,
}

pub fn derive_segments(shape: &ClipShape) -> Vec<Segment> {
    if shape.trim_end - shape.trim_start <= EPS {
        return Vec::new();
    }

    let cuts: Vec<Cut> = normalize_cuts(&shape.cuts)
        .into_iter()
        .filter(|c| c.end > shape.trim_start && c.start < shape.trim_end)
        .collect();

    let mut kept: Vec<(f64, f64)> = Vec::new();
    let mut cursor = shape.trim_start;
    for c in cuts {
        let cut_start = c.start.max(shape.trim_start);
        let cut_end = c.end.min(shape.trim_end);
        if cut_start - cursor > EPS {
            kept.push((cursor, cut_start));
        }
        cursor = cursor.max(cut_end);
    }
    if shape.trim_end - cursor > EPS {
        kept.push((cursor, shape.trim_end));
    }

    let mut segments: Vec<Segment> = Vec::new();
    for (start, end) in kept {
        let mut inside: Vec<f64> = shape
            .split_points
            .iter()
            .copied()
            .filter(|p| *p > start + EPS && *p < end - EPS)
            .collect();
        inside.sort_by(f64::total_cmp);
        let mut from = start;
        for p in inside {
            segments.push(Segment {
                start: from,
                end: p,
                index: segments.len(),
            });
            from = p;
        }
        segments.push(Segment {
            start: from,
            end,
            index: segments.len(),
        });
    }
    segments
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Seam {
    pub gap_start: f64,
    pub gap_end: f64,
    pub removed: f64,
}

pub fn derive_seams(segments: &[Segment]) -> Vec<Seam> {
    segments
        .windows(2)
        .filter_map(|pair| {
            let gap = pair[1].start - pair[0].end;
            (gap > EPS).then_some(Seam {
                gap_start: pair[0].end,
                gap_end: pair[1].start,
                removed: gap,
            })
        })
        .collect()
}

/// Right-biased at an interior boundary (NLE convention); the clip's final
/// instant belongs to the last segment.
pub fn segment_at(segments: &[Segment], t: f64) -> Option<Segment> {
    if let Some(seg) = segments
        .iter()
        .copied()
        .find(|s| t >= s.start - EPS && t < s.end - EPS)
    {
        return Some(seg);
    }
    segments
        .last()
        .copied()
        .filter(|last| (t - last.end).abs() <= EPS)
}

pub fn plan_split(t: f64, shape: &ClipShape) -> Option<Vec<f64>> {
    if t <= shape.trim_start + EPS || t >= shape.trim_end - EPS {
        return None;
    }
    if normalize_cuts(&shape.cuts)
        .iter()
        .any(|c| t > c.start - EPS && t < c.end + EPS)
    {
        return None;
    }
    if shape.split_points.iter().any(|p| (p - t).abs() <= EPS) {
        return None;
    }
    let mut out = shape.split_points.clone();
    out.push(t);
    out.sort_by(f64::total_cmp);
    Some(out)
}

#[derive(Debug, Clone, PartialEq)]
pub struct DeletePlan {
    pub cut: Cut,
    pub split_points: Vec<f64>,
}

pub fn plan_delete_segment(seg: &Segment, split_points: &[f64]) -> DeletePlan {
    DeletePlan {
        cut: Cut::new(seg.start, seg.end),
        split_points: split_points
            .iter()
            .copied()
            .filter(|p| *p < seg.start - EPS || *p > seg.end + EPS)
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shape(trim_start: f64, trim_end: f64, cuts: &[(f64, f64)], splits: &[f64]) -> ClipShape {
        ClipShape {
            trim_start,
            trim_end,
            cuts: cuts.iter().map(|(s, e)| Cut::new(*s, *e)).collect(),
            split_points: splits.to_vec(),
        }
    }

    #[test]
    fn an_uncut_clip_is_one_segment() {
        let segs = derive_segments(&shape(2.0, 8.0, &[], &[]));
        assert_eq!(segs.len(), 1);
        assert_eq!((segs[0].start, segs[0].end), (2.0, 8.0));
    }

    #[test]
    fn a_cut_splits_the_clip_and_indices_stay_contiguous() {
        let segs = derive_segments(&shape(0.0, 10.0, &[(4.0, 6.0)], &[]));
        assert_eq!(segs.len(), 2);
        assert_eq!(segs[1].index, 1);
        assert_eq!((segs[0].end, segs[1].start), (4.0, 6.0));
    }

    #[test]
    fn a_split_subdivides_without_removing_time() {
        let segs = derive_segments(&shape(0.0, 10.0, &[], &[4.0]));
        assert_eq!(segs.len(), 2);
        assert_eq!(segs[0].end, segs[1].start);
    }

    #[test]
    fn split_points_inside_a_cut_or_outside_the_clip_are_ignored() {
        let segs = derive_segments(&shape(0.0, 10.0, &[(4.0, 6.0)], &[5.0, 20.0, -3.0]));
        assert_eq!(segs.len(), 2);
    }

    #[test]
    fn a_cut_straddling_the_in_point_is_clamped() {
        let segs = derive_segments(&shape(10.0, 20.0, &[(8.0, 12.0)], &[]));
        assert_eq!(segs.len(), 1);
        assert_eq!(segs[0].start, 12.0);
    }

    #[test]
    fn a_degenerate_trim_yields_nothing() {
        assert!(derive_segments(&shape(5.0, 5.0, &[], &[])).is_empty());
    }

    #[test]
    fn seams_appear_only_where_time_was_removed() {
        let segs = derive_segments(&shape(0.0, 10.0, &[(4.0, 6.0)], &[8.0]));
        let seams = derive_seams(&segs);
        assert_eq!(seams.len(), 1);
        assert_eq!(seams[0].removed, 2.0);
    }

    #[test]
    fn segment_at_is_right_biased_on_a_boundary() {
        let segs = derive_segments(&shape(0.0, 10.0, &[], &[4.0]));
        assert_eq!(segment_at(&segs, 4.0).map(|s| s.index), Some(1));
        assert_eq!(segment_at(&segs, 10.0).map(|s| s.index), Some(1));
        assert!(segment_at(&segs, 11.0).is_none());
    }

    #[test]
    fn split_is_refused_at_the_edges_and_inside_a_cut() {
        let s = shape(0.0, 10.0, &[(4.0, 6.0)], &[]);
        assert!(plan_split(0.0, &s).is_none());
        assert!(plan_split(10.0, &s).is_none());
        assert!(plan_split(5.0, &s).is_none());
        assert!(plan_split(4.0, &s).is_none());
    }

    #[test]
    fn split_is_refused_where_one_already_exists() {
        let s = shape(0.0, 10.0, &[], &[4.0]);
        assert!(plan_split(4.0, &s).is_none());
        assert_eq!(plan_split(2.0, &s), Some(vec![2.0, 4.0]));
    }

    #[test]
    fn deleting_a_segment_prunes_the_splits_inside_it() {
        let seg = Segment {
            start: 4.0,
            end: 8.0,
            index: 1,
        };
        let plan = plan_delete_segment(&seg, &[2.0, 4.0, 6.0, 8.0, 9.0]);
        assert_eq!(plan.cut, Cut::new(4.0, 8.0));
        assert_eq!(plan.split_points, vec![2.0, 9.0]);
    }
}
