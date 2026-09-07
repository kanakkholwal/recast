#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::EPS;

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Cut {
    pub start: f64,
    pub end: f64,
}

impl Cut {
    pub fn new(start: f64, end: f64) -> Self {
        Self { start, end }
    }

    pub fn duration(&self) -> f64 {
        self.end - self.start
    }
}

pub fn normalize_cuts(cuts: &[Cut]) -> Vec<Cut> {
    let mut valid: Vec<Cut> = cuts
        .iter()
        .copied()
        .filter(|c| c.end > c.start && c.start.is_finite() && c.end.is_finite())
        .collect();
    valid.sort_by(|a, b| a.start.total_cmp(&b.start));

    let mut out: Vec<Cut> = Vec::with_capacity(valid.len());
    for cut in valid {
        match out.last_mut() {
            Some(last) if cut.start <= last.end + EPS => last.end = last.end.max(cut.end),
            _ => out.push(cut),
        }
    }
    out
}

pub fn total_cut_duration(cuts: &[Cut]) -> f64 {
    normalize_cuts(cuts).iter().map(Cut::duration).sum()
}

pub fn cut_containing(cuts: &[Cut], t: f64) -> Option<Cut> {
    cuts.iter().copied().find(|c| t >= c.start && t < c.end)
}

pub fn original_to_output_cuts(cuts: &[Cut], t: f64) -> f64 {
    let mut removed = 0.0;
    for c in normalize_cuts(cuts) {
        if c.end <= t {
            removed += c.duration();
        } else if c.start < t {
            removed += t - c.start;
            break;
        } else {
            break;
        }
    }
    t - removed
}

pub fn output_to_original_cuts(cuts: &[Cut], t: f64) -> f64 {
    let mut orig = t;
    for c in normalize_cuts(cuts) {
        if c.start <= orig {
            orig += c.duration();
        } else {
            break;
        }
    }
    orig
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cuts(pairs: &[(f64, f64)]) -> Vec<Cut> {
        pairs.iter().map(|(s, e)| Cut::new(*s, *e)).collect()
    }

    #[test]
    fn normalize_sorts_and_merges_overlaps() {
        let out = normalize_cuts(&cuts(&[(5.0, 7.0), (1.0, 3.0), (2.0, 4.0)]));
        assert_eq!(out, cuts(&[(1.0, 4.0), (5.0, 7.0)]));
    }

    #[test]
    fn normalize_merges_cuts_that_only_touch() {
        let out = normalize_cuts(&cuts(&[(1.0, 3.0), (3.0, 5.0)]));
        assert_eq!(out, cuts(&[(1.0, 5.0)]));
    }

    #[test]
    fn normalize_drops_empty_and_inverted_ranges() {
        assert!(normalize_cuts(&cuts(&[(2.0, 2.0), (5.0, 1.0)])).is_empty());
    }

    #[test]
    fn a_time_inside_a_cut_collapses_onto_its_start() {
        let c = cuts(&[(3.0, 5.0)]);
        assert!((original_to_output_cuts(&c, 4.0) - 3.0).abs() < 1e-9);
    }

    #[test]
    fn output_to_original_reverses_a_kept_time() {
        let c = cuts(&[(3.0, 5.0)]);
        let out = original_to_output_cuts(&c, 8.0);
        assert!((output_to_original_cuts(&c, out) - 8.0).abs() < 1e-9);
    }

    #[test]
    fn total_duration_counts_the_merged_union_once() {
        assert!((total_cut_duration(&cuts(&[(1.0, 4.0), (2.0, 3.0)])) - 3.0).abs() < 1e-9);
    }

    #[test]
    fn cut_containing_is_half_open() {
        let c = cuts(&[(3.0, 5.0)]);
        assert!(cut_containing(&c, 3.0).is_some());
        assert!(cut_containing(&c, 5.0).is_none());
    }
}
