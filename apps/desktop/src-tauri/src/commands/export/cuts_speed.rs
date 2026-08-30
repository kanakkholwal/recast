//! Cut + per-segment-speed math for the video export pipeline: resolve cuts
//! to post-trim ranges, derive kept segments, and build the FFmpeg
//! select/setpts/atempo expressions. Split out of commands/editor.rs; the
//! parity tests (against the shared speed-parity fixture) moved with it.
//!
//! The derivation here is now the FALLBACK. Normal exports carry the editor's
//! resolved time map (`TimeSpanWire`) and this module formats it, so the two
//! cannot drift; deriving is kept for payloads that arrive without one.

use serde::{Deserialize, Serialize};

/// Resolve the render state's silence/manual cuts into post-trim stream
/// seconds (the input is seeked by `-ss trim_start`, so the filtergraph's `t`
/// starts at 0 = `trim_start`). Cuts are clamped to the kept `[trim_start,
/// trim_end]` window, sorted, and overlaps merged.
///
/// Note: split/cut editing and silence detection are EXPERIMENTAL, opt-in
/// features on the client. The frontend only includes a cut in `render_state`
/// when its feature is enabled (see `effectiveCuts` in the editor store and
/// `buildExportRenderState`), so when a feature is opted off `render_state.cuts`
/// is empty here and the export matches an un-edited clip. This pipeline applies
/// whatever cuts it is handed; it does not (and cannot) re-check the flags.
/// Two cut edges within this many seconds are treated as the same boundary and
/// merged. Kept in lockstep with `EPS` in the frontend's cut/segment model
/// (apps/desktop/src/lib/timeline/{cuts,segments}.ts) so the previewed edit and
/// the export never disagree on where a segment begins or ends.
const CUT_MERGE_EPS: f64 = 1e-4;

pub(crate) fn collect_export_cuts(
    render_state: &crate::render::graph::RenderState,
    trim_start: f64,
    trim_end: f64,
) -> Vec<(f64, f64)> {
    let mut cuts: Vec<(f64, f64)> = render_state
        .cuts
        .iter()
        .filter_map(|c| {
            let lo = c.start.max(trim_start) - trim_start;
            let hi = c.end.min(trim_end) - trim_start;
            (hi - lo > 0.01).then_some((lo.max(0.0), hi))
        })
        .collect();
    cuts.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    let mut merged: Vec<(f64, f64)> = Vec::with_capacity(cuts.len());
    for cut in cuts {
        match merged.last_mut() {
            // Tolerance MUST match the frontend's `normalizeCuts` EPS so both agree on segment boundaries. See the cut-parity tests.
            Some(last) if cut.0 <= last.1 + CUT_MERGE_EPS => last.1 = last.1.max(cut.1),
            _ => merged.push(cut),
        }
    }
    merged
}

/// Build a `select`/`aselect` expression that *keeps* every frame outside the
/// cut ranges: `not(between(t,a,b)+between(t,c,d)+…)`. Single-quoted at the
/// call site so the inner commas survive the filtergraph parser.
pub(crate) fn build_cut_select_expr(cuts: &[(f64, f64)]) -> String {
    let terms: Vec<String> = cuts
        .iter()
        .map(|(a, b)| format!("between(t,{a:.3},{b:.3})"))
        .collect();
    format!("not({})", terms.join("+"))
}

// --- Per-segment speed: a tail-point timing warp, so no overlay evaluator changes; parity-tested against speed-parity.json.

/// Clamp mirroring MIN/MAX_SEGMENT_SPEED in segment-speed.ts; a bad value → 1×.
fn clamp_segment_speed(speed: f64) -> f64 {
    if !speed.is_finite() || speed <= 0.0 {
        1.0
    } else {
        speed.clamp(0.25, 4.0)
    }
}

/// A kept segment on the post-trim timeline (t=0 at trim_start) with its speed.
#[derive(Debug, Clone, Copy)]
pub(crate) struct SpeedSegment {
    pub(crate) start: f64,
    pub(crate) end: f64,
    pub(crate) speed: f64,
}

fn make_speed_segment(
    start: f64,
    end: f64,
    segment_speeds: &[crate::render::graph::SegmentSpeed],
    trim_start: f64,
) -> SpeedSegment {
    // Anchors are ORIGINAL-recording seconds, so a post-trim segment's original start is `start + trim_start`.
    let anchor = start + trim_start;
    let speed = segment_speeds
        .iter()
        .find(|sp| (sp.start - anchor).abs() <= CUT_MERGE_EPS)
        .map(|sp| clamp_segment_speed(sp.speed))
        .unwrap_or(1.0);
    SpeedSegment { start, end, speed }
}

/// Derive the kept segments on the post-trim timeline with their speeds. `cuts`
/// are the already-collected post-trim cut ranges; `split_points` and the speed
/// anchors are ORIGINAL seconds (shifted by `trim_start`). Mirrors
/// deriveSegments + segment-speed anchoring on the frontend.
pub(crate) fn build_speed_segments(
    duration: f64,
    cuts: &[(f64, f64)],
    split_points: &[f64],
    segment_speeds: &[crate::render::graph::SegmentSpeed],
    trim_start: f64,
) -> Vec<SpeedSegment> {
    // Kept intervals = [0, duration] minus the cuts.
    let mut kept: Vec<(f64, f64)> = Vec::new();
    let mut cursor = 0.0;
    for (cs, ce) in cuts {
        if cs - cursor > CUT_MERGE_EPS {
            kept.push((cursor, *cs));
        }
        cursor = cursor.max(*ce);
    }
    if duration - cursor > CUT_MERGE_EPS {
        kept.push((cursor, duration));
    }
    // Slice each kept interval at the split points strictly inside it.
    let mut segs: Vec<SpeedSegment> = Vec::new();
    for (s, e) in kept {
        let mut inside: Vec<f64> = split_points
            .iter()
            .map(|p| p - trim_start)
            .filter(|p| *p > s + CUT_MERGE_EPS && *p < e - CUT_MERGE_EPS)
            .collect();
        inside.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let mut from = s;
        for p in inside {
            segs.push(make_speed_segment(from, p, segment_speeds, trim_start));
            from = p;
        }
        segs.push(make_speed_segment(from, e, segment_speeds, trim_start));
    }
    segs
}

/// One kept span of the timeline exactly as the editor resolved it, in ORIGINAL
/// recording seconds. Sent with the export so the encoder replays the editor's
/// own axis rather than recomputing it from cuts + splits + speed anchors — the
/// recomputation that had to be kept in step with the frontend by hand.
#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TimeSpanWire {
    pub orig_start: f64,
    pub orig_end: f64,
    #[serde(default = "one")]
    pub speed: f64,
}

fn one() -> f64 {
    1.0
}

/// Convert the editor's spans onto the post-trim timeline the FFmpeg graph
/// works in. Spans outside `[trim_start, trim_start + duration]` are clipped
/// away, so a stale map can shorten the export but never address time the
/// trimmed stream does not have.
fn segments_from_wire(spans: &[TimeSpanWire], duration: f64, trim_start: f64) -> Vec<SpeedSegment> {
    spans
        .iter()
        .filter_map(|span| {
            let start = (span.orig_start - trim_start).max(0.0);
            let end = (span.orig_end - trim_start).min(duration);
            (end - start > CUT_MERGE_EPS).then(|| SpeedSegment {
                start,
                end,
                speed: clamp_segment_speed(span.speed),
            })
        })
        .collect()
}

/// The kept segments to export. Prefers the editor's resolved time map; falls
/// back to deriving it for payloads that carry none (agent/MCP requests, and
/// projects queued by an older build).
pub(crate) fn resolve_speed_segments(
    time_map: Option<&Vec<TimeSpanWire>>,
    duration: f64,
    cuts: &[(f64, f64)],
    split_points: &[f64],
    segment_speeds: &[crate::render::graph::SegmentSpeed],
    trim_start: f64,
) -> Vec<SpeedSegment> {
    if let Some(spans) = time_map {
        let segs = segments_from_wire(spans, duration, trim_start);
        if !segs.is_empty() {
            return segs;
        }
        log::warn!("export time map resolved to nothing; deriving segments instead");
    }
    build_speed_segments(duration, cuts, split_points, segment_speeds, trim_start)
}

/// Any segment off 1× — the guard that keeps the no-speed export path unchanged.
pub(crate) fn has_speed_change(segs: &[SpeedSegment]) -> bool {
    segs.iter().any(|s| (s.speed - 1.0).abs() > CUT_MERGE_EPS)
}

/// Warped output duration — the value the export and the frontend time-map must
/// agree on. The FFmpeg pipeline expresses the same warp via `setpts`/`atempo`;
/// this also drives the output-side `-t` cap so the encode stops at the real
/// post-edit content length (cuts dropped + speed warped), not the raw trimmed
/// span — otherwise the infinite background generators freeze the last frame
/// past content-end. Parity-tested against the frontend time-map.
pub(crate) fn warped_output_duration(segs: &[SpeedSegment]) -> f64 {
    segs.iter().map(|s| (s.end - s.start) / s.speed).sum()
}

/// Output-side `-t` cap (seconds): the real post-edit content length. Non-GIF
/// exports run cuts + per-segment speed through select/setpts, so the stream is
/// the warped duration — capping at the raw trimmed span would freeze the last
/// frame over the infinite background for the cut/sped-away time (and truncate
/// slow-motion, where warped > raw). GIF keeps the raw trimmed span for a plain
/// cuts-only export (it loops and has no infinite audio/background tail to
/// freeze), but once per-segment speed warps the stream it must follow the
/// warped length — otherwise slow-motion (warped > raw) is truncated and a
/// speed-up leaves a dangling tail.
pub(crate) fn output_duration_cap(
    format: &str,
    duration: f64,
    speed_segments: &[SpeedSegment],
) -> f64 {
    if format == "gif" && !has_speed_change(speed_segments) {
        duration
    } else {
        warped_output_duration(speed_segments)
    }
}

/// `setpts` seconds-expression mapping a survivor frame's post-trim source time
/// `T` onto the warped output axis: within segment i it is
/// `offset_i + (T - start_i)/speed_i`, selected by nested `if(lt(T,end_i),…)`
/// with the last segment as the else branch. Caller wraps as `setpts=(EXPR)/TB`.
pub(crate) fn build_speed_setpts_expr(segs: &[SpeedSegment]) -> String {
    fn rec(segs: &[SpeedSegment], i: usize, offset: f64) -> String {
        let s = &segs[i];
        let here = format!("{:.6}+(T-{:.6})/{:.6}", offset, s.start, s.speed);
        if i + 1 >= segs.len() {
            here
        } else {
            let next_offset = offset + (s.end - s.start) / s.speed;
            format!(
                "if(lt(T,{:.6}),{},{})",
                s.end,
                here,
                rec(segs, i + 1, next_offset)
            )
        }
    }
    if segs.is_empty() {
        "T".to_string()
    } else {
        rec(segs, 0, 0.0)
    }
}

/// FFmpeg `atempo` accepts 0.5..=2.0 per instance; chain stages to cover the
/// 0.25..=4.0 speed range (e.g. 4× → "atempo=2.0,atempo=2.0").
fn atempo_chain(speed: f64) -> String {
    let mut remaining = speed;
    let mut stages: Vec<f64> = Vec::new();
    while remaining > 2.0 + 1e-9 {
        stages.push(2.0);
        remaining /= 2.0;
    }
    while remaining < 0.5 - 1e-9 {
        stages.push(0.5);
        remaining /= 0.5;
    }
    stages.push(remaining);
    stages
        .iter()
        .map(|s| format!("atempo={s:.6}"))
        .collect::<Vec<_>>()
        .join(",")
}

/// Per-segment audio retime that matches the video warp: split the kept audio
/// into one branch per segment, `atrim` each to its post-trim range, `atempo` to
/// its speed, then `concat` (audio mode). Replaces the cut-only `aselect`/`asetpts` path when
/// any segment is sped. `amap` is the audio label to consume (e.g. "[0:a:0]").
pub(crate) fn build_speed_audio_filter(amap: &str, segs: &[SpeedSegment]) -> String {
    let n = segs.len();
    let mut parts: Vec<String> = Vec::new();
    let split_labels: Vec<String> = (0..n).map(|i| format!("[aspd{i}]")).collect();
    parts.push(format!("{amap}asplit={n}{}", split_labels.join("")));
    let mut seg_labels: Vec<String> = Vec::new();
    for (i, s) in segs.iter().enumerate() {
        let out = format!("[aseg{i}]");
        parts.push(format!(
            "{}atrim=start={:.6}:end={:.6},asetpts=PTS-STARTPTS,{}{}",
            split_labels[i],
            s.start,
            s.end,
            atempo_chain(s.speed),
            out
        ));
        seg_labels.push(out);
    }
    // FFmpeg has no `aconcat`; audio uses the `concat` filter configured audio-only (v=0:a=1).
    parts.push(format!("{}concat=n={n}:v=0:a=1[acut]", seg_labels.join("")));
    parts.join(";")
}

/// Append the tail-of-chain cut+speed stage to the export filter graph. Drops the
/// cut ranges (`select`/`aselect`) and re-times survivors (`setpts`/`atempo`) —
/// everything upstream (zoom, cursor, blur) was computed on the continuous
/// post-trim timeline and stays correct, since select only removes frames. GIF is
/// skipped (it has its own paletteuse tail). No-op when there are no cuts and no
/// speed change. Extracted verbatim from `export_video`; mutates the filter
/// accumulator, video/audio maps, and the pending output-filter list in place.
#[allow(clippy::too_many_arguments)]
pub(crate) fn append_cut_speed_stage(
    filter_complex: &mut Option<String>,
    video_map: &mut String,
    audio_map: &mut Option<String>,
    output_filters: &mut Vec<String>,
    format: &str,
    export_cuts: &[(f64, f64)],
    speed_segments: &[SpeedSegment],
    speed_active: bool,
) {
    if !((!export_cuts.is_empty() || speed_active) && format != "gif") {
        return;
    }
    let has_cuts = !export_cuts.is_empty();
    let select_expr = build_cut_select_expr(export_cuts);
    let (mut complex, video_label) = match filter_complex.take() {
        Some(existing) => (existing, video_map.clone()),
        None => {
            // Seed a filtergraph and fold in pending output-side filters, which are lost now that `-vf` no longer applies.
            let mut seed = String::new();
            let prefix = if output_filters.is_empty() {
                String::new()
            } else {
                format!("{},", output_filters.join(","))
            };
            output_filters.clear();
            seed.push_str(&format!("[0:v:0]{prefix}"));
            (seed, String::new())
        }
    };
    if !complex.is_empty() && !complex.ends_with(';') && !video_label.is_empty() {
        complex.push(';');
    }
    complex.push_str(&video_label);
    // Drop cut frames, then re-time the survivors; the output `-r` resamples the warped PTS back to CFR.
    let select_prefix = if has_cuts {
        format!("select='{select_expr}',")
    } else {
        String::new()
    };
    let setpts = if speed_active {
        // Single-quote the value: the warp expression's commas would otherwise read as filter separators.
        format!("setpts='({})/TB'", build_speed_setpts_expr(speed_segments))
    } else {
        "setpts=N/FRAME_RATE/TB".to_string()
    };
    complex.push_str(&format!("{select_prefix}{setpts}[vcut]"));
    *video_map = "[vcut]".to_string();
    if let Some(amap) = audio_map.take() {
        if speed_active {
            // Per-segment atrim, atempo and concat keep audio length matched to the warped video, pitch preserved.
            complex.push_str(&format!(
                ";{}",
                build_speed_audio_filter(&amap, speed_segments)
            ));
        } else {
            complex.push_str(&format!(
                ";{amap}aselect='{select_expr}',asetpts=N/SR/TB[acut]"
            ));
        }
        *audio_map = Some("[acut]".to_string());
    }
    *filter_complex = Some(complex);
}

/// Audio-only cut+speed warp, for the browser-render mux path where the video is
/// already warped to the output timeline (`-c:v copy`) and only the source audio
/// needs the same `aselect`/`atempo` treatment. Shares the cut-select and speed
/// helpers with `append_cut_speed_stage` so the two stay in lock-step.
pub(crate) fn append_audio_cut_speed(
    filter_complex: &mut Option<String>,
    audio_map: &mut Option<String>,
    export_cuts: &[(f64, f64)],
    speed_segments: &[SpeedSegment],
    speed_active: bool,
) {
    if export_cuts.is_empty() && !speed_active {
        return;
    }
    let Some(amap) = audio_map.take() else {
        return;
    };
    let mut complex = filter_complex.take().unwrap_or_default();
    if !complex.is_empty() && !complex.ends_with(';') {
        complex.push(';');
    }
    if speed_active {
        complex.push_str(&build_speed_audio_filter(&amap, speed_segments));
    } else {
        let select_expr = build_cut_select_expr(export_cuts);
        complex.push_str(&format!(
            "{amap}aselect='{select_expr}',asetpts=N/SR/TB[acut]"
        ));
    }
    *audio_map = Some("[acut]".to_string());
    *filter_complex = Some(complex);
}

#[cfg(test)]
mod cut_export_tests {
    use super::{
        atempo_chain, build_cut_select_expr, build_speed_segments, build_speed_setpts_expr,
        clamp_segment_speed, collect_export_cuts, output_duration_cap, resolve_speed_segments,
        warped_output_duration, SpeedSegment, TimeSpanWire,
    };
    use crate::render::graph::{CutRange, RenderState, SegmentSpeed};

    fn seg(start: f64, end: f64, speed: f64) -> SpeedSegment {
        SpeedSegment { start, end, speed }
    }

    fn span(orig_start: f64, orig_end: f64, speed: f64) -> TimeSpanWire {
        TimeSpanWire {
            orig_start,
            orig_end,
            speed,
        }
    }

    fn shape(segs: &[SpeedSegment]) -> Vec<(f64, f64, f64)> {
        segs.iter().map(|s| (s.start, s.end, s.speed)).collect()
    }

    #[test]
    fn the_editors_time_map_reproduces_what_derivation_would_have_built() {
        // Trim 2..14 with a cut at 6..8 and a split at 10, middle segment 2x.
        let (trim_start, duration) = (2.0, 12.0);
        let cuts = [(4.0, 6.0)];
        let splits = [10.0];
        let speeds = [SegmentSpeed {
            start: 8.0,
            speed: 2.0,
        }];
        let derived = build_speed_segments(duration, &cuts, &splits, &speeds, trim_start);
        // The same kept spans the editor's time map holds, in ORIGINAL seconds.
        let wire = vec![
            span(2.0, 6.0, 1.0),
            span(8.0, 10.0, 2.0),
            span(10.0, 14.0, 1.0),
        ];
        let consumed =
            resolve_speed_segments(Some(&wire), duration, &cuts, &splits, &speeds, trim_start);
        assert_eq!(shape(&consumed), shape(&derived));
    }

    #[test]
    fn a_payload_without_a_time_map_still_derives() {
        let cuts = [(4.0, 6.0)];
        let derived = build_speed_segments(12.0, &cuts, &[], &[], 2.0);
        let resolved = resolve_speed_segments(None, 12.0, &cuts, &[], &[], 2.0);
        assert_eq!(shape(&resolved), shape(&derived));
    }

    #[test]
    fn wire_spans_are_clipped_to_the_trimmed_stream() {
        // A stale map reaching past both edges must not address time the trimmed input does not contain.
        let wire = vec![span(0.0, 20.0, 1.0)];
        let segs = resolve_speed_segments(Some(&wire), 12.0, &[], &[], &[], 2.0);
        assert_eq!(shape(&segs), vec![(0.0, 12.0, 1.0)]);
    }

    #[test]
    fn an_empty_time_map_falls_back_rather_than_exporting_nothing() {
        let wire: Vec<TimeSpanWire> = vec![];
        let segs = resolve_speed_segments(Some(&wire), 12.0, &[], &[], &[], 0.0);
        assert_eq!(shape(&segs), vec![(0.0, 12.0, 1.0)]);
    }

    #[test]
    fn wire_speeds_are_clamped_like_derived_ones() {
        let wire = vec![span(0.0, 4.0, 99.0), span(4.0, 8.0, -1.0)];
        let segs = resolve_speed_segments(Some(&wire), 8.0, &[], &[], &[], 0.0);
        assert_eq!(shape(&segs), vec![(0.0, 4.0, 4.0), (4.0, 8.0, 1.0)]);
    }

    #[test]
    fn a_wire_map_and_its_derivation_warp_to_the_same_output_length() {
        let wire = vec![span(0.0, 4.0, 1.0), span(6.0, 10.0, 2.0)];
        let segs = resolve_speed_segments(Some(&wire), 10.0, &[], &[], &[], 0.0);
        assert!((warped_output_duration(&segs) - 6.0).abs() < 1e-9);
    }

    #[test]
    fn output_cap_uses_warped_length_when_speed_is_active_incl_gif() {
        // Two kept segments, the second sped 2× → raw span 8s, warped 4 + 2 = 6s.
        let segs = vec![seg(0.0, 4.0, 1.0), seg(4.0, 8.0, 2.0)];
        // Non-GIF caps at the real content length — this is the frozen-tail fix.
        assert!((output_duration_cap("mp4", 8.0, &segs) - 6.0).abs() < 1e-9);
        assert!((output_duration_cap("webm", 8.0, &segs) - 6.0).abs() < 1e-9);
        // GIF warps too, so the cap follows the warped length; the raw span would leave a frozen tail.
        assert!((output_duration_cap("gif", 8.0, &segs) - 6.0).abs() < 1e-9);
    }

    #[test]
    fn output_cap_keeps_raw_span_for_cuts_only_gif() {
        // All 1x: GIF keeps the raw trimmed span, since it loops and has no infinite tail to freeze.
        let segs = vec![seg(0.0, 3.0, 1.0), seg(5.0, 8.0, 1.0)];
        assert!((output_duration_cap("gif", 8.0, &segs) - 8.0).abs() < 1e-9);
        // Non-GIF still collapses to the kept length (6s here).
        assert!((output_duration_cap("mp4", 8.0, &segs) - 6.0).abs() < 1e-9);
    }

    #[test]
    fn output_cap_unchanged_without_edits() {
        let segs = vec![seg(0.0, 10.0, 1.0)];
        assert!((output_duration_cap("mp4", 10.0, &segs) - 10.0).abs() < 1e-9);
    }

    #[test]
    fn output_cap_extends_for_slow_motion_so_it_is_not_truncated() {
        // 0.5× → warped 20s > raw 10s; the cap must grow, not clip the slow-mo.
        let segs = vec![seg(0.0, 10.0, 0.5)];
        assert!((output_duration_cap("mp4", 10.0, &segs) - 20.0).abs() < 1e-9);
    }

    #[test]
    fn setpts_expr_edge_cases() {
        // No segments → identity remap.
        assert_eq!(build_speed_setpts_expr(&[]), "T");
        // Single segment → flat affine map, no nested if().
        let one = build_speed_setpts_expr(&[seg(0.0, 4.0, 2.0)]);
        assert_eq!(one, "0.000000+(T-0.000000)/2.000000");
        assert!(!one.contains("if("));
        // Two segments → exactly one branch boundary at the first segment's end.
        let two = build_speed_setpts_expr(&[seg(0.0, 4.0, 1.0), seg(4.0, 8.0, 2.0)]);
        assert_eq!(two.matches("if(lt(T,").count(), 1);
        assert!(two.contains("if(lt(T,4.000000)"));
    }

    #[test]
    fn clamp_segment_speed_guards_bad_values_and_clamps_range() {
        // Non-positive or non-finite collapse to 1x, never 0: no atempo hang or setpts divide-by-zero.
        assert_eq!(clamp_segment_speed(0.0), 1.0);
        assert_eq!(clamp_segment_speed(-2.0), 1.0);
        assert_eq!(clamp_segment_speed(f64::NAN), 1.0);
        assert_eq!(clamp_segment_speed(f64::INFINITY), 1.0);
        // In-range passes through; out-of-range clamps to [0.25, 4.0].
        assert_eq!(clamp_segment_speed(1.5), 1.5);
        assert_eq!(clamp_segment_speed(0.1), 0.25);
        assert_eq!(clamp_segment_speed(99.0), 4.0);
    }

    fn cut(start: f64, end: f64) -> CutRange {
        CutRange {
            start,
            end,
            extra: Default::default(),
        }
    }

    fn state_with_cuts(cuts: Vec<CutRange>) -> RenderState {
        RenderState {
            cuts,
            ..Default::default()
        }
    }

    #[test]
    fn select_expr_keeps_everything_outside_the_cuts() {
        // The export drops frames where this is false, so two cuts means keep = not(in A or in B).
        let expr = build_cut_select_expr(&[(1.5, 2.0), (4.0, 5.5)]);
        assert_eq!(expr, "not(between(t,1.500,2.000)+between(t,4.000,5.500))");
    }

    #[test]
    fn select_expr_single_cut() {
        assert_eq!(
            build_cut_select_expr(&[(2.0, 3.0)]),
            "not(between(t,2.000,3.000))"
        );
    }

    #[test]
    fn ripple_delete_in_middle_offsets_into_post_trim_time() {
        // Trimmed to [10,20]: a ripple-deleted clip at original [12,14] must reach ffmpeg as post-trim [2,4].
        let cuts = collect_export_cuts(&state_with_cuts(vec![cut(12.0, 14.0)]), 10.0, 20.0);
        assert_eq!(cuts, vec![(2.0, 4.0)]);
    }

    #[test]
    fn cut_outside_trim_is_dropped_and_straddling_is_clamped() {
        // [0,5] is entirely pre-trim and dropped; [8,12] straddles the in point and clamps to post-trim [0,2].
        let cuts = collect_export_cuts(
            &state_with_cuts(vec![cut(0.0, 5.0), cut(8.0, 12.0)]),
            10.0,
            20.0,
        );
        assert_eq!(cuts, vec![(0.0, 2.0)]);
    }

    #[test]
    fn overlapping_cuts_merge() {
        // post-trim [1,3] and [2,5] overlap → single [1,5].
        let cuts = collect_export_cuts(
            &state_with_cuts(vec![cut(11.0, 13.0), cut(12.0, 15.0)]),
            10.0,
            20.0,
        );
        assert_eq!(cuts, vec![(1.0, 5.0)]);
    }

    #[test]
    fn no_cuts_yields_empty() {
        assert!(collect_export_cuts(&state_with_cuts(vec![]), 0.0, 10.0).is_empty());
    }

    #[test]
    fn kept_duration_matches_shared_parity_fixtures() {
        // Anti-drift guard on the SAME json cuts.test.ts asserts against; a divergence fails one of the two tests.
        let raw = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../packages/editor/src/lib/timeline/__fixtures__/cut-parity.json"
        ));
        let doc: serde_json::Value = serde_json::from_str(raw).expect("valid fixture json");
        let cases = doc["cases"].as_array().expect("cases array");
        for case in cases {
            let name = case["name"].as_str().unwrap_or("?");
            let trim_start = case["trimStart"].as_f64().unwrap();
            let trim_end = case["trimEnd"].as_f64().unwrap();
            let expected = case["expectedKeptDuration"].as_f64().unwrap();
            let cuts: Vec<CutRange> = case["cuts"]
                .as_array()
                .unwrap()
                .iter()
                .map(|pair| {
                    let p = pair.as_array().unwrap();
                    cut(p[0].as_f64().unwrap(), p[1].as_f64().unwrap())
                })
                .collect();

            let merged = collect_export_cuts(&state_with_cuts(cuts), trim_start, trim_end);
            let removed: f64 = merged.iter().map(|(a, b)| b - a).sum();
            let kept = (trim_end - trim_start) - removed;
            assert!(
                (kept - expected).abs() < 1e-6,
                "parity case '{name}': export kept duration {kept} != expected {expected}"
            );
        }
    }

    #[test]
    fn warped_duration_matches_shared_parity_fixtures() {
        // Anti-drift guard for per-segment speed against the same json segment-speed.test.ts uses; all cases trimStart=0.
        let raw = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../packages/editor/src/lib/timeline/__fixtures__/speed-parity.json"
        ));
        let doc: serde_json::Value = serde_json::from_str(raw).expect("valid fixture json");
        for case in doc["cases"].as_array().expect("cases array") {
            let name = case["name"].as_str().unwrap_or("?");
            let trim_end = case["trimEnd"].as_f64().unwrap();
            let expected = case["expectedOutputDuration"].as_f64().unwrap();
            let cuts: Vec<CutRange> = case["cuts"]
                .as_array()
                .unwrap()
                .iter()
                .map(|p| {
                    let p = p.as_array().unwrap();
                    cut(p[0].as_f64().unwrap(), p[1].as_f64().unwrap())
                })
                .collect();
            let split_points: Vec<f64> = case["splitPoints"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_f64().unwrap())
                .collect();
            let speeds: Vec<SegmentSpeed> = case["segmentSpeeds"]
                .as_array()
                .unwrap()
                .iter()
                .map(|p| {
                    let p = p.as_array().unwrap();
                    SegmentSpeed {
                        start: p[0].as_f64().unwrap(),
                        speed: p[1].as_f64().unwrap(),
                    }
                })
                .collect();

            let merged = collect_export_cuts(&state_with_cuts(cuts), 0.0, trim_end);
            let segs = build_speed_segments(trim_end, &merged, &split_points, &speeds, 0.0);
            let got = warped_output_duration(&segs);
            assert!(
                (got - expected).abs() < 1e-6,
                "speed parity '{name}': warped duration {got} != expected {expected}"
            );
        }
    }

    #[test]
    fn atempo_chain_covers_the_speed_range() {
        assert_eq!(atempo_chain(1.5), "atempo=1.500000");
        assert_eq!(atempo_chain(4.0), "atempo=2.000000,atempo=2.000000");
        assert_eq!(atempo_chain(0.25), "atempo=0.500000,atempo=0.500000");
    }

    #[test]
    fn setpts_expr_warps_a_two_segment_clip() {
        // [0,4]@1x then [4,10]@2x → second segment maps T into half-rate output.
        let segs = build_speed_segments(
            10.0,
            &[],
            &[4.0],
            &[SegmentSpeed {
                start: 4.0,
                speed: 2.0,
            }],
            0.0,
        );
        let expr = build_speed_setpts_expr(&segs);
        assert_eq!(
            expr,
            "if(lt(T,4.000000),0.000000+(T-0.000000)/1.000000,4.000000+(T-4.000000)/2.000000)"
        );
    }
}
