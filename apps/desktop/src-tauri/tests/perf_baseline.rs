//! Baseline numbers for the engine rewrite. Reports rather than gates: CI runners
//! vary too much to assert a tight bound, but a catastrophic regression still fails.
//! Run with `--nocapture` to read the table.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Instant;

use recast_lib::render::graph::{
    compute_canvas_geometry, RenderGraph, RenderState, SourceVideoMetadata,
};
use recast_testkit::{media, SourceSpec};

/// Below this the pipeline is not usable for the product, whatever the machine.
const MIN_REALTIME_FACTOR: f64 = 0.25;

struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("recast-perf-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch dir");
        Self(dir)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn representative_state(duration: f64) -> RenderState {
    let mut state = RenderState {
        trim_start: 0.0,
        trim_end: duration,
        padding: 6.0,
        background_type: "color".into(),
        background_value: "#0f172a".into(),
        cursor_enabled: false,
        ..Default::default()
    };
    state.zoom_regions = vec![serde_json::from_value(serde_json::json!({
        "start": 0.5,
        "end": duration - 0.5,
        "scale": 1.8,
        "rampIn": 0.4,
        "rampOut": 0.4,
        "centerX": 0.4,
        "centerY": 0.6
    }))
    .expect("zoom region")];
    state
}

fn export_seconds(ffmpeg: &Path, source: &Path, spec: SourceSpec, state: &RenderState) -> f64 {
    let geom = compute_canvas_geometry(
        spec.width,
        spec.height,
        state.padding,
        state.output_aspect.as_deref(),
    );
    let plan = RenderGraph::from_state(state)
        .build_export_plan_with(
            SourceVideoMetadata {
                width: spec.width,
                height: spec.height,
                fps: spec.fps as f64,
            },
            Path::new("."),
            1,
            None,
            None,
            None,
            None,
            geom,
            None,
        )
        .expect("export plan");

    let mut args: Vec<String> = ["-hide_banner", "-loglevel", "error", "-y", "-i"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    args.push(source.to_string_lossy().into_owned());
    if let Some(fc) = &plan.filter_complex {
        args.push("-filter_complex".into());
        args.push(fc.clone());
        args.push("-map".into());
        args.push(format!("[{}]", plan.video_map.trim_matches(['[', ']'])));
    }
    // The generated background is an infinite source and the overlay base, so without a cap the graph never ends, as `append_output_tail` enforces.
    args.push("-t".into());
    args.push(format!("{:.3}", state.trim_end - state.trim_start));
    for arg in [
        "-c:v", "libx264", "-preset", "veryfast", "-crf", "23", "-pix_fmt", "yuv420p", "-an", "-f",
        "null", "-",
    ] {
        args.push(arg.to_string());
    }

    let start = Instant::now();
    let output = Command::new(ffmpeg)
        .args(&args)
        .stdin(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .expect("run ffmpeg");
    assert!(
        output.status.success(),
        "export failed (exit {:?}, stderr {} bytes): {}
filter: {}",
        output.status.code(),
        output.stderr.len(),
        String::from_utf8_lossy(&output.stderr).trim(),
        plan.filter_complex.as_deref().unwrap_or("<none>")
    );
    start.elapsed().as_secs_f64()
}

#[test]
#[ignore = "baseline measurement, not a gate: run with --ignored --nocapture"]
fn export_throughput_baseline() {
    let Some(ffmpeg) = media::ffmpeg_path() else {
        eprintln!("skipping: no ffmpeg");
        return;
    };
    let scratch = Scratch::new("throughput");

    println!();
    println!("| resolution | source | export | realtime factor |");
    println!("|---|---|---|---|");

    for (label, width, height) in [("720p", 1280u32, 720u32), ("1080p", 1920, 1080)] {
        let spec = SourceSpec {
            width,
            height,
            fps: 60,
            duration_secs: 2.0,
            ..Default::default()
        };
        let source = scratch.0.join(format!("{label}.mp4"));
        let encode_start = Instant::now();
        media::write_source(&ffmpeg, spec, &source).expect("synthetic source");
        eprintln!(
            "  {label}: source encoded in {:.1}s",
            encode_start.elapsed().as_secs_f64()
        );

        let state = representative_state(spec.duration_secs);
        let elapsed = export_seconds(&ffmpeg, &source, spec, &state);
        let factor = spec.duration_secs / elapsed;

        println!(
            "| {label} | {:.1}s | {elapsed:.2}s | {factor:.2}x |",
            spec.duration_secs
        );
        assert!(
            factor >= MIN_REALTIME_FACTOR,
            "{label} export ran at {factor:.2}x realtime, below the {MIN_REALTIME_FACTOR}x floor"
        );
    }
    println!();
}

fn zoom_terms_for(region_count: usize) -> (usize, usize) {
    let mut state = representative_state(60.0);
    state.zoom_regions = (0..region_count)
        .map(|index| {
            let start = index as f64 * 0.7;
            serde_json::from_value(serde_json::json!({
                "start": start,
                "end": start + 0.5,
                "scale": 1.5 + (index % 4) as f64 * 0.2,
                "rampIn": 0.15,
                "rampOut": 0.15,
                "centerX": 0.2 + (index % 5) as f64 * 0.1,
                "centerY": 0.3
            }))
            .expect("zoom region")
        })
        .collect();

    let geom = compute_canvas_geometry(1920, 1080, state.padding, None);
    let plan = RenderGraph::from_state(&state)
        .build_export_plan_with(
            SourceVideoMetadata {
                width: 1920,
                height: 1080,
                fps: 60.0,
            },
            Path::new("."),
            1,
            None,
            None,
            None,
            None,
            geom,
            None,
        )
        .expect("export plan");
    let fc = plan.filter_complex.unwrap_or_default();
    (fc.matches("if(gte(t,").count(), fc.len())
}

/// The export zoom is a piecewise-linear LUT whose tolerance is doubled until it
/// fits av_expr_parse's term budget. Past a certain region count the collapse is
/// total: every ramp merges to a flat 1.0 and the zoom disappears from the export
/// with no error, while the preview still shows it.
#[test]
#[ignore = "reporting only: run with --ignored --nocapture"]
fn zoom_term_count_by_region_count() {
    println!();
    println!("| regions | window terms | filter chars |");
    println!("|---|---|---|");
    let mut first_loss = None;
    for count in [1usize, 2, 4, 8, 12, 16, 24, 32, 40] {
        let (terms, chars) = zoom_terms_for(count);
        println!("| {count} | {terms} | {chars} |");
        if terms == 0 && first_loss.is_none() {
            first_loss = Some(count);
        }
    }
    println!();
    if let Some(count) = first_loss {
        println!("zoom vanishes entirely at {count} regions");
    }
    println!();

    let _ = first_loss;
}

#[test]
fn a_single_zoom_region_reaches_the_export() {
    assert!(zoom_terms_for(1).0 > 0);
}

/// KNOWN BUG, pinned so a fix is noticed. `build_zoom_exprs` doubles its merge
/// tolerance until the expression fits av_expr_parse's 48-term budget; past ~40
/// regions every ramp merges to a flat 1.0, `fmt_term` drops it as a no-op, and
/// the export ships with no zoom while the preview still shows it. No error, no log.
#[test]
fn zoom_currently_vanishes_at_forty_regions() {
    assert_eq!(
        zoom_terms_for(40).0,
        0,
        "zoom at 40 regions now produces terms; the collapse is fixed, update this test"
    );
    assert!(
        zoom_terms_for(32).0 > 0,
        "32 regions still reached the export"
    );
}
