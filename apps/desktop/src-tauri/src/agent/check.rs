//! `recast check`, tier 1: structural findings over the document, no GPU and no decode.
//! Validator failures are errors; everything else here is what an agent cannot see from the state alone (a zoom inside a cut, captions with no words).

use recast_time::TimeMap;
use serde::Serialize;

use super::axis::{output_span, time_map};
use super::perception::transcript_words;
use crate::commands::validate_render_state;
use crate::render::graph::RenderState;

/// Shorter than this a zoom or annotation cannot be perceived.
const MIN_PERCEPTIBLE_SECS: f64 = 0.15;
/// Outside this range a speed override reads as a mistake.
const SPEED_RANGE: std::ops::RangeInclusive<f64> = 0.25..=4.0;
/// Below this the output is too short to be a video.
const MIN_OUTPUT_SECS: f64 = 1.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
    Info,
}

/// Facts about the project the state itself does not carry.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProjectFacts {
    pub source_duration: f64,
    pub has_camera: bool,
    pub has_cursor_track: bool,
}

impl ProjectFacts {
    pub fn of(doc: &crate::commands::types::EditorDocument) -> Self {
        Self {
            source_duration: doc.metadata.duration,
            has_camera: doc.camera_path.is_some(),
            has_cursor_track: doc.cursor_path.is_some(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Finding {
    /// Stable machine code; renaming one is a breaking change for any agent that branches on it.
    pub code: String,
    pub severity: Severity,
    /// Dotted path into the render state, the same shape `ValidationIssue::field` uses.
    pub field: String,
    pub message: String,
    /// Source-axis seconds the finding is about, where it has a time.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<(f64, f64)>,
    /// The same range on the output axis, where any of it survives cuts.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<(f64, f64)>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    /// No errors and no warnings; info findings do not count.
    pub clean: bool,
    pub errors: usize,
    pub warnings: usize,
    pub findings: Vec<Finding>,
}

impl Report {
    /// Findings in `self` that `other` did not have, keyed by code and field.
    pub fn introduced_since(&self, other: &Report) -> Vec<Finding> {
        self.findings
            .iter()
            .filter(|f| {
                !other
                    .findings
                    .iter()
                    .any(|o| o.code == f.code && o.field == f.field)
            })
            .cloned()
            .collect()
    }
}

pub fn check(state: &RenderState, facts: &ProjectFacts) -> Report {
    let map = time_map(state);
    let mut findings = Vec::new();
    push_validator(state, facts, &mut findings);
    push_visibility(state, &map, &mut findings);
    push_lanes(state, facts, &mut findings);
    push_timeline(state, &map, &mut findings);
    super::geometry::push_geometry(state, &map, &mut findings);
    let errors = findings
        .iter()
        .filter(|f| f.severity == Severity::Error)
        .count();
    let warnings = findings
        .iter()
        .filter(|f| f.severity == Severity::Warning)
        .count();
    Report {
        clean: errors == 0 && warnings == 0,
        errors,
        warnings,
        findings,
    }
}

fn push_validator(state: &RenderState, facts: &ProjectFacts, out: &mut Vec<Finding>) {
    if let Err(issues) = validate_render_state(state, facts.source_duration) {
        out.extend(issues.into_iter().map(|issue| Finding {
            message: format!("{} fails '{}'", issue.field, issue.reason),
            code: issue.reason,
            severity: Severity::Error,
            field: issue.field,
            source: None,
            output: None,
        }));
    }
}

fn push_visibility(state: &RenderState, map: &TimeMap, out: &mut Vec<Finding>) {
    for (i, z) in state.zoom_regions.iter().enumerate() {
        let field = format!("zoomRegions/{i}");
        timed(out, map, &field, "zoom", z.start, z.end, z.hidden);
    }
    for (i, a) in state.annotations.iter().enumerate() {
        let field = format!("annotations/{i}");
        timed(out, map, &field, "annotation", a.start, a.end, a.hidden);
    }
    let mut zooms: Vec<(usize, f64, f64)> = state
        .zoom_regions
        .iter()
        .enumerate()
        .filter(|(_, z)| !z.hidden)
        .map(|(i, z)| (i, z.start, z.end))
        .collect();
    zooms.sort_by(|a, b| a.1.total_cmp(&b.1));
    for pair in zooms.windows(2) {
        let (_, _, a_end) = pair[0];
        let (b_i, b_start, b_end) = pair[1];
        if b_start < a_end {
            out.push(finding(
                "zoom_overlap",
                Severity::Info,
                format!("zoomRegions/{b_i}"),
                "overlaps the previous zoom; the later start wins while both are active",
                map,
                b_start,
                b_end,
            ));
        }
    }
}

fn timed(
    out: &mut Vec<Finding>,
    map: &TimeMap,
    field: &str,
    kind: &str,
    start: f64,
    end: f64,
    hidden: bool,
) {
    if hidden {
        return;
    }
    if output_span(map, start, end).is_none() {
        out.push(finding(
            &format!("{kind}_never_visible"),
            Severity::Warning,
            field.to_string(),
            &format!("{kind} lies entirely inside a cut or outside the trim, so it never shows"),
            map,
            start,
            end,
        ));
    } else if end - start < MIN_PERCEPTIBLE_SECS {
        out.push(finding(
            &format!("{kind}_too_short"),
            Severity::Warning,
            field.to_string(),
            &format!("{kind} lasts under {MIN_PERCEPTIBLE_SECS}s, which reads as a flicker"),
            map,
            start,
            end,
        ));
    }
}

fn push_lanes(state: &RenderState, facts: &ProjectFacts, out: &mut Vec<Finding>) {
    let captions_on = state.caption_style.as_ref().is_some_and(|c| c.enabled);
    if captions_on && transcript_words(state).is_empty() {
        out.push(plain(
            "captions_without_words",
            Severity::Warning,
            "captionStyle.enabled",
            "captions are on but nothing has been transcribed, so none will draw",
        ));
    }
    if state.camera_overlay.enabled && !facts.has_camera {
        out.push(plain(
            "camera_without_recording",
            Severity::Warning,
            "cameraOverlay.enabled",
            "the camera bubble is on but the project has no camera recording",
        ));
    }
    if state.cursor_enabled && !facts.has_cursor_track {
        out.push(plain(
            "cursor_without_track",
            Severity::Info,
            "cursorEnabled",
            "the cursor lane is on but the project has no pointer track",
        ));
    }
    if !state.focus_enabled && !state.zoom_regions.is_empty() {
        out.push(plain(
            "zooms_lane_off",
            Severity::Info,
            "focusEnabled",
            "zoom regions are authored but the zoom lane is off",
        ));
    }
    if !state.annotations_enabled && !state.annotations.is_empty() {
        out.push(plain(
            "annotations_lane_off",
            Severity::Info,
            "annotationsEnabled",
            "annotations are authored but the annotation lane is off",
        ));
    }
}

fn push_timeline(state: &RenderState, map: &TimeMap, out: &mut Vec<Finding>) {
    for (i, s) in state.segment_speeds.iter().enumerate() {
        if !SPEED_RANGE.contains(&s.speed) {
            out.push(plain(
                "speed_extreme",
                Severity::Warning,
                &format!("segmentSpeeds/{i}/speed"),
                &format!(
                    "{}x is outside the {SPEED_RANGE:?} range viewers can follow",
                    s.speed
                ),
            ));
        }
    }
    if map.output_duration < MIN_OUTPUT_SECS {
        out.push(plain(
            "output_too_short",
            Severity::Warning,
            "trimEnd",
            &format!(
                "the output lasts {:.3}s after trim and cuts",
                map.output_duration
            ),
        ));
    }
}

pub(super) fn finding(
    code: &str,
    severity: Severity,
    field: String,
    message: &str,
    map: &TimeMap,
    start: f64,
    end: f64,
) -> Finding {
    Finding {
        code: code.to_string(),
        severity,
        field,
        message: message.to_string(),
        source: Some((start, end)),
        output: output_span(map, start, end),
    }
}

pub(super) fn plain(code: &str, severity: Severity, field: &str, message: &str) -> Finding {
    Finding {
        code: code.to_string(),
        severity,
        field: field.to_string(),
        message: message.to_string(),
        source: None,
        output: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::graph::{CutRange, SegmentSpeed};
    use crate::render::node_types::ZoomRegion;
    use recast_scene::v1::Easing;

    fn facts() -> ProjectFacts {
        ProjectFacts {
            source_duration: 60.0,
            has_camera: true,
            has_cursor_track: true,
        }
    }

    fn zoom(start: f64, end: f64) -> ZoomRegion {
        ZoomRegion {
            start,
            end,
            scale: 1.8,
            ease_in: Easing::default(),
            ease_out: Easing::default(),
            ramp_in: 0.3,
            ramp_out: 0.3,
            center_x: 0.5,
            center_y: 0.5,
            hidden: false,
            motion_blur: 0.0,
            extra: Default::default(),
        }
    }

    fn base() -> RenderState {
        RenderState {
            trim_start: 0.0,
            trim_end: 60.0,
            cursor_enabled: false,
            ..RenderState::default()
        }
    }

    fn codes(report: &Report) -> Vec<&str> {
        report.findings.iter().map(|f| f.code.as_str()).collect()
    }

    #[test]
    fn a_plain_project_is_clean() {
        let mut state = base();
        state.camera_overlay.enabled = false;
        let report = check(&state, &facts());
        assert!(report.clean, "{:?}", report.findings);
    }

    #[test]
    fn a_zoom_inside_a_cut_is_never_visible_and_carries_no_output_span() {
        let mut state = base();
        state.camera_overlay.enabled = false;
        state.cuts.push(CutRange {
            start: 10.0,
            end: 20.0,
            extra: Default::default(),
        });
        state.zoom_regions.push(zoom(12.0, 18.0));
        let report = check(&state, &facts());
        let f = report
            .findings
            .iter()
            .find(|f| f.code == "zoom_never_visible")
            .unwrap();
        assert_eq!(f.field, "zoomRegions/0");
        assert_eq!(f.source, Some((12.0, 18.0)));
        assert_eq!(f.output, None);
        assert_eq!(report.warnings, 1);
    }

    #[test]
    fn a_visible_zoom_after_a_cut_reports_its_output_position() {
        let mut state = base();
        state.camera_overlay.enabled = false;
        state.cuts.push(CutRange {
            start: 10.0,
            end: 20.0,
            extra: Default::default(),
        });
        state.zoom_regions.push(zoom(30.0, 30.05));
        let report = check(&state, &facts());
        let f = report
            .findings
            .iter()
            .find(|f| f.code == "zoom_too_short")
            .unwrap();
        let (s, _) = f.output.unwrap();
        assert!((s - 20.0).abs() < 1e-9);
    }

    #[test]
    fn overlapping_zooms_are_reported_on_the_later_one_as_info() {
        let mut state = base();
        state.camera_overlay.enabled = false;
        state.zoom_regions.push(zoom(5.0, 15.0));
        state.zoom_regions.push(zoom(10.0, 20.0));
        let report = check(&state, &facts());
        let f = report
            .findings
            .iter()
            .find(|f| f.code == "zoom_overlap")
            .unwrap();
        assert_eq!(f.field, "zoomRegions/1");
        assert_eq!(f.severity, Severity::Info);
        assert!(report.clean);
    }

    #[test]
    fn validator_failures_become_errors_with_the_reason_as_code() {
        let mut state = base();
        state.trim_end = 100.0;
        let report = check(&state, &facts());
        assert!(codes(&report).contains(&"trim_end_exceeds_source"));
        assert_eq!(report.errors, 1);
    }

    #[test]
    fn lanes_without_their_inputs_are_named() {
        let mut state = base();
        state.cursor_enabled = true;
        state.camera_overlay.enabled = true;
        state.caption_style = Some(recast_captions::CaptionStyle {
            enabled: true,
            ..Default::default()
        });
        let no_inputs = ProjectFacts {
            has_camera: false,
            has_cursor_track: false,
            ..facts()
        };
        let report = check(&state, &no_inputs);
        let found = codes(&report);
        assert!(found.contains(&"captions_without_words"));
        assert!(found.contains(&"camera_without_recording"));
        assert!(found.contains(&"cursor_without_track"));
    }

    #[test]
    fn an_extreme_speed_and_a_tiny_output_are_warnings() {
        let mut state = base();
        state.camera_overlay.enabled = false;
        state.trim_end = 0.5;
        state.segment_speeds.push(SegmentSpeed {
            start: 0.0,
            speed: 8.0,
        });
        let report = check(&state, &facts());
        let found = codes(&report);
        assert!(found.contains(&"speed_extreme"));
        assert!(found.contains(&"output_too_short"));
    }

    #[test]
    fn introduced_since_reports_only_new_findings() {
        let mut before = base();
        before.camera_overlay.enabled = false;
        let mut after = before.clone();
        after.zoom_regions.push(zoom(1.0, 1.01));
        let new = check(&after, &facts()).introduced_since(&check(&before, &facts()));
        assert_eq!(new.len(), 1);
        assert_eq!(new[0].code, "zoom_too_short");
    }
}
