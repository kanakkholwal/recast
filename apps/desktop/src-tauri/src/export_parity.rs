//! Diffing the FFmpeg graph against the engine, file against file: the gate on
//! deleting `render/graph.rs`.

use std::path::Path;

/// One fixture's disagreement, in decoded luma.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Delta {
    /// Frames the two paths both produced and compared.
    pub compared: u64,
    /// Mean absolute luma difference over those frames, 0..255.
    pub mean_abs: f64,
    /// The worst single frame's mean absolute difference.
    pub worst_frame: f64,
}

impl Delta {
    /// Whether the two paths agree closely enough to call them one renderer.
    /// Never zero: sRGB against linear light is a deliberate correction.
    #[must_use]
    pub fn agrees_within(&self, mean: f64) -> bool {
        self.compared > 0 && self.mean_abs <= mean
    }
}

/// Mean absolute difference between two luma planes, ignoring any tail.
#[must_use]
pub fn luma_delta(a: &[u8], b: &[u8]) -> f64 {
    let len = a.len().min(b.len());
    if len == 0 {
        return f64::INFINITY;
    }
    let total: u64 = a[..len]
        .iter()
        .zip(&b[..len])
        .map(|(x, y)| u64::from(x.abs_diff(*y)))
        .sum();
    total as f64 / len as f64
}

/// Mean absolute difference over RGB, ignoring alpha. For frames compared
/// BEFORE encoding, where the only difference left is the compositing itself.
#[must_use]
pub fn rgba_delta(a: &[u8], b: &[u8]) -> f64 {
    let len = a.len().min(b.len());
    if len < 4 {
        return f64::INFINITY;
    }
    let mut total = 0u64;
    let mut counted = 0u64;
    for (x, y) in a[..len]
        .as_chunks::<4>()
        .0
        .iter()
        .zip(b[..len].as_chunks::<4>().0)
    {
        for channel in 0..3 {
            total += u64::from(x[channel].abs_diff(y[channel]));
            counted += 1;
        }
    }
    if counted == 0 {
        return f64::INFINITY;
    }
    total as f64 / counted as f64
}

/// Compares two encoded files frame by frame on luma. Luma only: chroma
/// subsampling and encoder quantisation say nothing about the compositing.
#[cfg(windows)]
pub fn compare_files(left: &Path, right: &Path) -> Result<Delta, String> {
    use recast_codec_mf::VideoReader;

    let mut a = VideoReader::open(left).map_err(|e| format!("{}: {e}", left.display()))?;
    let mut b = VideoReader::open(right).map_err(|e| format!("{}: {e}", right.display()))?;
    let luma = (a.info().width * a.info().height) as usize;

    let mut compared = 0u64;
    let mut total = 0.0;
    let mut worst = 0.0f64;
    while let (Some(fa), Some(fb)) = (
        a.next_frame().map_err(|e| e.to_string())?,
        b.next_frame().map_err(|e| e.to_string())?,
    ) {
        let delta = luma_delta(
            &fa.data[..luma.min(fa.data.len())],
            &fb.data[..luma.min(fb.data.len())],
        );
        worst = worst.max(delta);
        total += delta;
        compared += 1;
    }

    Ok(Delta {
        compared,
        mean_abs: if compared == 0 {
            f64::INFINITY
        } else {
            total / compared as f64
        },
        worst_frame: worst,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_planes_have_no_delta() {
        assert!((luma_delta(&[10, 20, 30], &[10, 20, 30])).abs() < f64::EPSILON);
    }

    #[test]
    fn the_delta_is_the_mean_absolute_difference() {
        // 0 + 10 + 20 over three samples.
        assert!((luma_delta(&[10, 20, 30], &[10, 30, 10]) - 10.0).abs() < 1e-9);
    }

    /// Two files that decoded nothing must not read as perfect agreement, which
    /// is what a zero-length mean would say.
    #[test]
    fn comparing_nothing_is_not_agreement() {
        assert_eq!(luma_delta(&[], &[]), f64::INFINITY);
        let empty = Delta {
            compared: 0,
            mean_abs: 0.0,
            worst_frame: 0.0,
        };
        assert!(!empty.agrees_within(1.0), "an empty comparison passed");
    }

    #[test]
    fn identical_rgba_frames_have_no_delta() {
        let frame = [10u8, 20, 30, 255, 40, 50, 60, 255];
        assert!(rgba_delta(&frame, &frame).abs() < f64::EPSILON);
    }

    /// Alpha is excluded: the graph writes opaque frames and the engine
    /// un-premultiplies, so an alpha difference says nothing about compositing.
    #[test]
    fn the_rgba_delta_ignores_alpha() {
        let a = [10u8, 20, 30, 255];
        let b = [10u8, 20, 30, 0];
        assert!(rgba_delta(&a, &b).abs() < f64::EPSILON);
    }

    #[test]
    fn the_rgba_delta_averages_over_the_colour_channels() {
        // 3 + 0 + 0 over three channels.
        let a = [10u8, 20, 30, 255];
        let b = [13u8, 20, 30, 255];
        assert!((rgba_delta(&a, &b) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn comparing_no_pixels_is_not_agreement() {
        assert_eq!(rgba_delta(&[], &[]), f64::INFINITY);
    }

    #[test]
    fn a_delta_inside_the_bound_agrees_and_one_outside_does_not() {
        let close = Delta {
            compared: 30,
            mean_abs: 2.0,
            worst_frame: 5.0,
        };
        assert!(close.agrees_within(3.0));
        assert!(!close.agrees_within(1.0));
    }
}

#[cfg(all(test, windows))]
mod live {
    use std::path::PathBuf;
    use std::process::Command;

    use recast_compositor::{
        PlaneData, PlaneLayout, RenderSource, Session, SourceColor, SourceGeometry, SourcePlanes,
    };
    use recast_export::{FrameLoop, FrameWalk, Mp4Sink, PictureSource};
    use recast_gpu::{GpuContext, GpuOptions};
    use recast_scene::migrate::to_scene;
    use recast_scene::v1::RenderState;

    use super::*;

    /// The engine side of a parity run: same rate and bitrate as the graph, no
    /// quality cap, so only the renderer differs.
    fn engine_spec<'a>(
        input: &'a std::path::Path,
        output: &'a std::path::Path,
    ) -> crate::export_engine::ExportSpec<'a> {
        crate::export_engine::ExportSpec {
            input,
            output,
            fps: (FPS, 1),
            bitrate: Some(8_000_000),
            max_size: None,
            captions: None,
            audio: true,
            source: recast_export::SourceInfo {
                width: W,
                height: H,
                fps: f64::from(FPS),
            },
            ffmpeg: None,
            force_ffmpeg: false,
            audio_sources: crate::export_audio::RecordingAudio::default(),
        }
    }

    /// The parity harness measures pixels, not progress; it never cancels.
    fn never_cancels(_done: u64, _total: u64) -> crate::export_engine::Flow {
        crate::export_engine::Flow::Continue
    }
    use crate::render::graph::{compute_canvas_geometry, RenderGraph, SourceVideoMetadata};

    const W: u32 = 640;
    const H: u32 = 360;
    const FPS: u32 = 30;
    const SECONDS: f64 = 0.5;

    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let dir =
                std::env::temp_dir().join(format!("recast-parity-{name}-{}", std::process::id()));
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

    struct Grey(Vec<u8>);

    impl PictureSource for Grey {
        type Error = std::convert::Infallible;

        fn picture_at(&mut self, _t: f64) -> Result<Option<SourcePlanes<'_>>, Self::Error> {
            Ok(Some(SourcePlanes {
                width: W,
                height: H,
                layout: PlaneLayout::Nv12,
                color: SourceColor::default(),
                data: PlaneData::Packed(&self.0),
            }))
        }
    }

    fn fixture() -> RenderState {
        RenderState {
            trim_start: 0.0,
            trim_end: SECONDS,
            padding: 0.0,
            cursor_enabled: false,
            background_type: "color".into(),
            background_value: "#2200ff".into(),
            ..Default::default()
        }
    }

    /// The cases a solid background under an opaque overlay cannot show:
    /// padding geometry, a zoom transform, a gradient, and a reframe.
    fn fixtures() -> Vec<(&'static str, RenderState)> {
        let plain = fixture();

        let mut padded = plain.clone();
        padded.padding = 8.0;
        padded.background_value = "#1e293b".into();

        let mut zoomed = padded.clone();
        zoomed.zoom_regions = vec![serde_json::from_value(serde_json::json!({
            "start": 0.0, "end": SECONDS, "scale": 2.0,
            "rampIn": 0.0, "rampOut": 0.0, "centerX": 0.5, "centerY": 0.5
        }))
        .expect("zoom fixture")];

        let mut off_centre = zoomed.clone();
        off_centre.zoom_regions = vec![serde_json::from_value(serde_json::json!({
            "start": 0.0, "end": SECONDS, "scale": 1.8,
            "rampIn": 0.0, "rampOut": 0.0, "centerX": 0.28, "centerY": 0.62
        }))
        .expect("zoom fixture")];

        let mut gradient = plain.clone();
        gradient.padding = 8.0;
        gradient.background_type = "gradient".into();
        gradient.background_value = "linear-gradient(90deg, #ff0000 0%, #0000ff 100%)".into();

        // Same gradient, more of it: a colour-space delta scales with area.
        let mut gradient_wide = gradient.clone();
        gradient_wide.padding = 30.0;

        let mut portrait = padded.clone();
        portrait.output_aspect = Some("9:16".into());

        // A blurred backdrop is its own filter and its own engine pass, and nothing else here exercises either.
        let mut blurred = padded.clone();
        blurred.background_blur = 40.0;

        // Trim alone moves the source axis; cut and speed are appended outside this harness's plan, so they stay unmeasured.
        let mut trimmed = padded.clone();
        trimmed.trim_start = 0.1;
        trimmed.trim_end = 0.4;

        vec![
            ("plain", plain),
            ("padded", padded),
            ("zoom-centred", zoomed),
            ("zoom-off-centre", off_centre),
            ("gradient", gradient),
            ("gradient-wide", gradient_wide),
            ("portrait-9x16", portrait),
            ("background-blur", blurred),
            ("trimmed", trimmed),
        ]
    }

    /// A recording both paths read, written by the engine so the input is
    /// identical for each and only the render differs.
    fn source(ctx: &GpuContext, path: &Path) {
        let state = fixture();
        let mut session = Session::new(
            ctx,
            to_scene(&state),
            SourceGeometry {
                width: W,
                height: H,
            },
        )
        .expect("session");
        let size = RenderSource::output_size(&session);
        let walk = FrameWalk::new(SECONDS, (FPS, 1));
        let mut sink = Mp4Sink::new(
            size.width,
            size.height,
            walk,
            8_000_000,
            SourceColor::default(),
        )
        .expect("encoder");
        let mut bytes = vec![170u8; (W * H) as usize];
        bytes.resize(PlaneLayout::Nv12.packed_len(W, H), 128);
        FrameLoop::new()
            .run(
                &mut session,
                &mut Grey(bytes),
                walk,
                ctx.device(),
                ctx.queue(),
                |index, rgba| sink.push(index, rgba),
            )
            .expect("rendered");
        std::fs::write(path, sink.finish().expect("finished")).expect("write");
    }

    /// The sidecar, found from the manifest rather than the running exe: a test
    /// binary lives in `target/debug/deps`, where the bundle search never looks.
    fn ffmpeg_binary() -> Option<PathBuf> {
        let target = format!("ffmpeg-{}.exe", std::env::consts::ARCH);
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("binaries");
        let candidates = [
            dir.join("ffmpeg-x86_64-pc-windows-msvc.exe"),
            dir.join(target),
            crate::ffmpeg::ffmpeg_path().clone(),
        ];
        candidates.into_iter().find(|p| p.exists())
    }

    /// The graph path: the same filter_complex the shipping export builds.
    fn render_graph(
        ffmpeg: &Path,
        input: &Path,
        output: &Path,
        state: &RenderState,
    ) -> Result<(), String> {
        let geom = compute_canvas_geometry(W, H, state.padding, state.output_aspect.as_deref());
        let plan = RenderGraph::from_state(state)
            .build_export_plan_with(
                SourceVideoMetadata {
                    width: W,
                    height: H,
                    fps: f64::from(FPS),
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
            .map_err(|e| e.to_string())?;

        let mut args: Vec<String> = ["-hide_banner", "-loglevel", "error", "-y", "-i"]
            .iter()
            .map(ToString::to_string)
            .collect();
        args.push(input.to_string_lossy().into_owned());
        if let Some(fc) = &plan.filter_complex {
            args.push("-filter_complex".into());
            args.push(fc.clone());
            args.push("-map".into());
            args.push(format!("[{}]", plan.video_map.trim_matches(['[', ']'])));
        }
        // `color=` is INFINITE: unbounded, overlay never ends and ffmpeg fills the disk.
        args.push("-frames:v".into());
        args.push(FrameWalk::new(SECONDS, (FPS, 1)).len().to_string());
        for arg in ["-an", "-c:v", "libx264", "-pix_fmt", "yuv420p"] {
            args.push(arg.to_string());
        }
        args.push(output.to_string_lossy().into_owned());

        let mut command = Command::new(ffmpeg);
        command.args(&args);
        crate::ffmpeg::configure_silent_command(&mut command);
        let out = command.output().map_err(|e| e.to_string())?;
        if !out.status.success() {
            return Err(String::from_utf8_lossy(&out.stderr).into_owned());
        }
        Ok(())
    }

    /// MEASUREMENT, not a gate. The number this prints is what decides whether
    /// `render/graph.rs` can be deleted. Run with `--ignored --nocapture`.
    #[test]
    #[ignore = "measurement: run with --ignored --nocapture"]
    fn the_graph_and_the_engine_are_diffed_file_against_file() {
        let Ok(ctx) = GpuContext::new_blocking(GpuOptions {
            require_hardware: false,
            ..Default::default()
        }) else {
            eprintln!("skipping: no adapter");
            return;
        };
        let scratch = Scratch::new("diff");
        let input = scratch.0.join("in.mp4");
        let graph_out = scratch.0.join("graph.mp4");
        let engine_out = scratch.0.join("engine.mp4");
        source(&ctx, &input);

        // Absent ffmpeg skips; ffmpeg failing is a FAILURE, not a quiet skip.
        let Some(ffmpeg) = ffmpeg_binary() else {
            eprintln!("skipping: no ffmpeg sidecar");
            return;
        };
        let state = fixture();
        render_graph(&ffmpeg, &input, &graph_out, &state).expect("the graph path renders");
        crate::export_engine::export_video(
            &state,
            &engine_spec(&input, &engine_out),
            &mut never_cancels,
        )
        .expect("the engine export runs");

        let delta = compare_files(&graph_out, &engine_out).expect("both files decode");
        println!(
            "graph vs engine: {} frames, mean |dY| {:.2}, worst frame {:.2}",
            delta.compared, delta.mean_abs, delta.worst_frame
        );
        assert!(delta.compared > 0, "nothing was compared");
    }

    /// The graph path as raw RGBA: no encoder between compositing and compare.
    fn graph_frames(
        ffmpeg: &Path,
        input: &Path,
        state: &RenderState,
        frames: u64,
    ) -> Result<Vec<Vec<u8>>, String> {
        let geom = compute_canvas_geometry(W, H, state.padding, state.output_aspect.as_deref());
        let plan = RenderGraph::from_state(state)
            .build_export_plan_with(
                SourceVideoMetadata {
                    width: W,
                    height: H,
                    fps: f64::from(FPS),
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
            .map_err(|e| e.to_string())?;

        let mut args: Vec<String> = ["-hide_banner", "-loglevel", "error", "-i"]
            .iter()
            .map(ToString::to_string)
            .collect();
        args.push(input.to_string_lossy().into_owned());
        if let Some(fc) = &plan.filter_complex {
            args.push("-filter_complex".into());
            args.push(fc.clone());
            args.push("-map".into());
            args.push(format!("[{}]", plan.video_map.trim_matches(['[', ']'])));
        }
        args.push("-frames:v".into());
        args.push(frames.to_string());
        for arg in ["-an", "-f", "rawvideo", "-pix_fmt", "rgba", "-"] {
            args.push(arg.to_string());
        }

        let mut command = Command::new(ffmpeg);
        command.args(&args);
        crate::ffmpeg::configure_silent_command(&mut command);
        let out = command.output().map_err(|e| e.to_string())?;
        if !out.status.success() {
            return Err(String::from_utf8_lossy(&out.stderr).into_owned());
        }
        let stride = (geom.canvas_w * geom.canvas_h * 4) as usize;
        if stride == 0 || out.stdout.len() < stride {
            return Err(format!(
                "ffmpeg produced {} bytes for a {stride}-byte frame",
                out.stdout.len()
            ));
        }
        Ok(out.stdout.chunks(stride).map(<[u8]>::to_vec).collect())
    }

    /// The engine path rendered to raw RGBA, the same frames.
    fn engine_frames(ctx: &GpuContext, input: &Path, state: &RenderState) -> Vec<Vec<u8>> {
        let mut pictures = recast_export::VideoPictures::open(input, SourceColor::default())
            .expect("the recording opens");
        let source = SourceGeometry {
            width: pictures.width(),
            height: pictures.height(),
        };
        let mut session = Session::new(ctx, to_scene(state), source).expect("session");
        let walk = FrameWalk::new(RenderSource::output_duration(&session), (FPS, 1));
        let mut frames = Vec::new();
        FrameLoop::new()
            .run(
                &mut session,
                &mut pictures,
                walk,
                ctx.device(),
                ctx.queue(),
                |_, rgba| {
                    frames.push(rgba.to_vec());
                    Ok::<_, std::convert::Infallible>(())
                },
            )
            .expect("rendered");
        frames
    }

    /// The real gate: both renderers compared BEFORE either encoder, so the
    /// number is the compositing difference and nothing else.
    #[test]
    #[ignore = "measurement: run with --ignored --nocapture"]
    fn every_fixture_is_diffed_before_encoding() {
        let Ok(ctx) = GpuContext::new_blocking(GpuOptions {
            require_hardware: false,
            ..Default::default()
        }) else {
            eprintln!("skipping: no adapter");
            return;
        };
        let Some(ffmpeg) = ffmpeg_binary() else {
            eprintln!("skipping: no ffmpeg sidecar");
            return;
        };
        let scratch = Scratch::new("preencode");
        let input = scratch.0.join("in.mp4");
        source(&ctx, &input);

        println!(
            "{:<18} {:>7} {:>10} {:>10}",
            "fixture", "frames", "mean |dRGB|", "worst"
        );
        for (name, state) in fixtures() {
            let engine = engine_frames(&ctx, &input, &state);
            let graph = match graph_frames(&ffmpeg, &input, &state, engine.len() as u64) {
                Ok(frames) => frames,
                Err(e) => {
                    println!("{name:<18} GRAPH FAILED: {e}");
                    continue;
                }
            };
            let pairs = engine.len().min(graph.len());
            let mut total = 0.0;
            let mut worst = 0.0f64;
            for (a, b) in graph.iter().zip(&engine).take(pairs) {
                let d = rgba_delta(a, b);
                worst = worst.max(d);
                total += d;
            }
            let mean = if pairs == 0 {
                f64::INFINITY
            } else {
                total / pairs as f64
            };
            println!("{name:<18} {pairs:>7} {mean:>10.2} {worst:>10.2}");
        }
    }

    /// Measured at 1.28-3.40 and bit-stable, so 6.0 is adapter headroom rather
    /// than slack for a regression.
    const TRANSFORM_CEILING: f64 = 6.0;

    /// A GATE: geometry, padding, zoom and reframing must agree, or the engine
    /// cannot replace the graph. `gradient` differs by design, so it is separate.
    #[test]
    fn the_transforms_agree_between_both_renderers() {
        let Ok(ctx) = GpuContext::new_blocking(GpuOptions {
            require_hardware: false,
            ..Default::default()
        }) else {
            eprintln!("skipping: no adapter");
            return;
        };
        let Some(ffmpeg) = ffmpeg_binary() else {
            eprintln!("skipping: no ffmpeg sidecar");
            return;
        };
        let scratch = Scratch::new("gate");
        let input = scratch.0.join("in.mp4");
        source(&ctx, &input);

        for (name, state) in fixtures() {
            if name.starts_with("gradient") {
                continue;
            }
            let engine = engine_frames(&ctx, &input, &state);
            let graph = graph_frames(&ffmpeg, &input, &state, engine.len() as u64)
                .unwrap_or_else(|e| panic!("{name}: the graph path failed: {e}"));
            assert!(
                !engine.is_empty() && !graph.is_empty(),
                "{name}: nothing rendered"
            );
            let worst = graph
                .iter()
                .zip(&engine)
                .map(|(a, b)| rgba_delta(a, b))
                .fold(0.0f64, f64::max);
            assert!(
                worst <= TRANSFORM_CEILING,
                "{name}: renderers diverged by {worst:.2}, ceiling {TRANSFORM_CEILING}"
            );
        }
    }

    /// Pinned because it scales with visible area, which is what says the
    /// difference is the background's colour space and not a geometry fault.
    #[test]
    fn the_gradient_difference_scales_with_how_much_gradient_shows() {
        let Ok(ctx) = GpuContext::new_blocking(GpuOptions {
            require_hardware: false,
            ..Default::default()
        }) else {
            eprintln!("skipping: no adapter");
            return;
        };
        let Some(ffmpeg) = ffmpeg_binary() else {
            eprintln!("skipping: no ffmpeg sidecar");
            return;
        };
        let scratch = Scratch::new("gradient");
        let input = scratch.0.join("in.mp4");
        source(&ctx, &input);

        let mut deltas = Vec::new();
        for (name, state) in fixtures() {
            if !name.starts_with("gradient") {
                continue;
            }
            let engine = engine_frames(&ctx, &input, &state);
            let graph = graph_frames(&ffmpeg, &input, &state, engine.len() as u64)
                .unwrap_or_else(|e| panic!("{name}: the graph path failed: {e}"));
            let mean = graph
                .iter()
                .zip(&engine)
                .map(|(a, b)| rgba_delta(a, b))
                .sum::<f64>()
                / graph.len().max(1) as f64;
            deltas.push((name, mean));
        }

        let narrow = deltas
            .iter()
            .find(|(n, _)| *n == "gradient")
            .expect("narrow");
        let wide = deltas
            .iter()
            .find(|(n, _)| *n == "gradient-wide")
            .expect("wide");
        assert!(
            wide.1 > narrow.1 * 1.3,
            "more gradient did not mean more difference: {:.2} then {:.2}",
            narrow.1,
            wide.1
        );
        assert!(
            narrow.1 > TRANSFORM_CEILING,
            "the gradient stopped differing; if the graph now composites in              linear light too, delete this test"
        );
    }

    /// The third renderer: `cursor_export` rebuilds FFmpeg's SAMPLED table
    /// rather than evaluating the ease, so it only approximates the picture.
    fn cursor_zoom_at(state: &RenderState, t: f64) -> f64 {
        let regions = state.zoom_regions.clone();
        crate::render::cursor_export::active_zoom_at(&regions, t, 0.0).map_or(1.0, |(s, _, _)| s)
    }

    /// The engine's zoom at `t`, read back out of the layer transform.
    fn engine_zoom_at(state: &RenderState, t: f64) -> f64 {
        let scene = to_scene(state);
        let evaluator = recast_compositor::Evaluator::new(
            &scene,
            SourceGeometry {
                width: W,
                height: H,
            },
        );
        let params = evaluator.evaluate(&scene, t);
        params
            .layers
            .first()
            .map_or(1.0, |l| 1.0 / f64::from(l.transform.sx).max(1e-6))
    }

    /// KNOWN DEFECT, pinned: the overlay and the picture agree AT the table's
    /// samples and drift between them, sliding the cursor during a ramp.
    #[test]
    fn the_cursor_overlay_zoom_drifts_from_the_picture_during_a_ramp() {
        let mut ramped = fixture();
        ramped.zoom_regions = vec![serde_json::from_value(serde_json::json!({
            "start": 0.0, "end": SECONDS, "scale": 2.5,
            "rampIn": 0.2, "rampOut": 0.2, "centerX": 0.5, "centerY": 0.5
        }))
        .expect("zoom fixture")];

        // `clamp(ceil(duration * 20), 8, 200)` steps: every 0.05s and nowhere else.
        let samples = (SECONDS * 20.0).ceil().clamp(8.0, 200.0);
        let sample_step = SECONDS / samples;

        let mut worst = 0.0f64;
        for step in 0..=40 {
            let t = SECONDS * f64::from(step) / 40.0;
            worst = worst.max((engine_zoom_at(&ramped, t) - cursor_zoom_at(&ramped, t)).abs());
        }
        let mut at_a_sample = 0.0f64;
        for k in 0..=samples as u32 {
            let t = f64::from(k) * sample_step;
            at_a_sample =
                at_a_sample.max((engine_zoom_at(&ramped, t) - cursor_zoom_at(&ramped, t)).abs());
        }

        assert!(
            at_a_sample < 1e-6,
            "the two disagree even at a table sample ({at_a_sample:.4}); that is a              different bug from the interpolation drift this pins"
        );
        assert!(
            worst > 0.05,
            "the drift is gone ({worst:.4}). If the cursor now evaluates the ease,              delete this test"
        );
        // 0.1 of scale displaces a quarter-frame point ~48px at 1920. Visible.
        assert!(worst < 0.2, "the drift grew to {worst:.4}");
    }

    /// The two annotation fades are line-for-line duplicates in two files, with
    /// nothing keeping them in step. This is what would notice a divergence.
    #[test]
    fn both_renderers_fade_an_annotation_identically() {
        use recast_scene::v1::nodes::{Annotation, AnnotationKind};

        let make = |start: f64, end: f64, ramp_in: f64, ramp_out: f64| -> Annotation {
            serde_json::from_value(serde_json::json!({
                "id": "a1",
                "kind": { "kind": "rect", "x": 0.1, "y": 0.1, "w": 0.3, "h": 0.2 },
                "start": start, "end": end,
                "rampIn": ramp_in, "rampOut": ramp_out,
                "opacity": 0.8
            }))
            .expect("annotation fixture")
        };

        // The last two bind the `duration * 0.5` clamp; the first never reaches it.
        let cases = [
            make(0.5, 2.5, 0.4, 0.6),
            make(0.5, 1.5, 0.8, 0.9),
            make(0.0, 0.2, 1.0, 1.0),
        ];
        assert!(
            matches!(cases[0].kind, AnnotationKind::Rect { .. }),
            "the fixture stopped being a rect"
        );

        let mut worst = 0.0f64;
        for annotation in &cases {
            for step in 0..=240 {
                let t = f64::from(step) * 3.0 / 240.0;
                let graph = crate::render::cursor_export::annotation_opacity(annotation, t);
                let engine = recast_compositor::annotation_alpha(annotation, t);
                worst = worst.max((graph - engine).abs());
            }
        }
        assert!(
            worst < 1e-12,
            "the two annotation fades have drifted apart by {worst}"
        );
    }

    /// MEASUREMENT: how far the cursor overlay's idea of the zoom drifts from
    /// the picture's. Run with `--ignored --nocapture`.
    #[test]
    #[ignore = "measurement: run with --ignored --nocapture"]
    fn the_cursor_overlay_zoom_is_diffed_against_the_pictures() {
        let mut ramped = fixture();
        ramped.zoom_regions = vec![serde_json::from_value(serde_json::json!({
            "start": 0.0, "end": SECONDS, "scale": 2.5,
            "rampIn": 0.2, "rampOut": 0.2, "centerX": 0.5, "centerY": 0.5
        }))
        .expect("zoom fixture")];

        println!(
            "{:>8} {:>10} {:>10} {:>10}",
            "t", "picture", "cursor", "delta"
        );
        let mut worst = 0.0f64;
        for step in 0..=20 {
            let t = SECONDS * f64::from(step) / 20.0;
            let picture = engine_zoom_at(&ramped, t);
            let cursor = cursor_zoom_at(&ramped, t);
            let delta = (picture - cursor).abs();
            worst = worst.max(delta);
            println!("{t:>8.3} {picture:>10.4} {cursor:>10.4} {delta:>10.4}");
        }
        println!("worst zoom disagreement: {worst:.4}");
    }
    /// understood; every fixture that disagrees is a reason not to delete yet.
    #[test]
    #[ignore = "measurement: run with --ignored --nocapture"]
    fn every_fixture_is_diffed_across_both_renderers() {
        let Ok(ctx) = GpuContext::new_blocking(GpuOptions {
            require_hardware: false,
            ..Default::default()
        }) else {
            eprintln!("skipping: no adapter");
            return;
        };
        let Some(ffmpeg) = ffmpeg_binary() else {
            eprintln!("skipping: no ffmpeg sidecar");
            return;
        };
        let scratch = Scratch::new("fixtures");
        let input = scratch.0.join("in.mp4");
        source(&ctx, &input);

        println!(
            "{:<18} {:>7} {:>10} {:>10}",
            "fixture", "frames", "mean |dY|", "worst"
        );
        let mut worst_fixture = ("", 0.0f64);
        for (name, state) in fixtures() {
            let graph_out = scratch.0.join(format!("{name}-graph.mp4"));
            let engine_out = scratch.0.join(format!("{name}-engine.mp4"));
            if let Err(e) = render_graph(&ffmpeg, &input, &graph_out, &state) {
                println!("{name:<18} GRAPH FAILED: {e}");
                continue;
            }
            match crate::export_engine::export_video(
                &state,
                &engine_spec(&input, &engine_out),
                &mut never_cancels,
            ) {
                Ok(_) => {}
                Err(e) => {
                    println!("{name:<18} ENGINE FAILED: {e}");
                    continue;
                }
            }
            let delta = compare_files(&graph_out, &engine_out).expect("both decode");
            println!(
                "{name:<18} {:>7} {:>10.2} {:>10.2}",
                delta.compared, delta.mean_abs, delta.worst_frame
            );
            if delta.mean_abs > worst_fixture.1 {
                worst_fixture = (name, delta.mean_abs);
            }
        }
        println!(
            "worst fixture: {} at {:.2}",
            worst_fixture.0, worst_fixture.1
        );
    }
}
