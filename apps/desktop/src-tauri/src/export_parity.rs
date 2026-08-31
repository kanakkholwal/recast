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
        crate::export_engine::export_video(&state, &input, &engine_out, (FPS, 1), 8_000_000)
            .expect("the engine export runs");

        let delta = compare_files(&graph_out, &engine_out).expect("both files decode");
        println!(
            "graph vs engine: {} frames, mean |dY| {:.2}, worst frame {:.2}",
            delta.compared, delta.mean_abs, delta.worst_frame
        );
        assert!(delta.compared > 0, "nothing was compared");
    }
}
