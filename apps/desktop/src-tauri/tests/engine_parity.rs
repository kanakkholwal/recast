//! The parity number phase 0 could not produce: the SAME fixture rendered by the
//! FFmpeg export graph and by the wgpu compositor, diffed. Reports rather than
//! gates, because linear-light compositing is a deliberate correction and the
//! two engines are expected to differ; the point is that the difference is
//! measured and explained instead of assumed.
//!
//! Run with `--ignored --nocapture` to read the table.
//!
//! READ THE NUMBER CAREFULLY. Every comparable fixture here composites OPAQUE
//! layers, so none of them exercises alpha blending, and linear-light and
//! sRGB-space compositing produce the same answer when nothing is blended. A
//! small delta is therefore evidence that geometry, sampling and the transfer
//! functions agree; it is NOT evidence that the linear-space change is
//! invisible. The fixtures that would show it (a semi-transparent annotation, a
//! drop shadow, a blur) all rasterise outside `build_export_plan_with`, so they
//! cannot be driven from here at all.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use recast_compositor::{Compositor, Evaluator, FrameInputs, LayerInput, SourceGeometry};
use recast_gpu::{GpuContext, GpuOptions};
use recast_lib::render::graph::{
    compute_canvas_geometry, RenderGraph, RenderState, SourceVideoMetadata,
};
use recast_scene::migrate::to_scene;
use recast_scene::LayerSource;
use recast_testkit::{compare, media, SourceSpec};

const SRC_W: u32 = 320;
const SRC_H: u32 = 180;
const FPS: u32 = 30;
const SAMPLE_FRAME: u64 = 15;

struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("recast-parity-{name}"));
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

fn fixtures() -> Vec<(&'static str, RenderState)> {
    let base = RenderState {
        trim_start: 0.0,
        trim_end: 1.5,
        cursor_enabled: false,
        ..Default::default()
    };

    let mut padded = base.clone();
    padded.padding = 8.0;
    padded.background_type = "color".into();
    padded.background_value = "#1e293b".into();

    let mut zoomed = padded.clone();
    zoomed.zoom_regions = vec![serde_json::from_value(serde_json::json!({
        "start": 0.0,
        "end": 1.5,
        "scale": 2.0,
        "rampIn": 0.0,
        "rampOut": 0.0,
        "centerX": 0.5,
        "centerY": 0.5
    }))
    .expect("zoom fixture")];

    let mut gradient = base.clone();
    gradient.padding = 8.0;
    gradient.background_type = "gradient".into();
    gradient.background_value = "linear-gradient(90deg, #ff0000 0%, #0000ff 100%)".into();

    let mut portrait = padded.clone();
    portrait.output_aspect = Some("9:16".into());

    vec![
        ("plain", base),
        ("padded-color", padded),
        ("zoomed", zoomed),
        ("gradient", gradient),
        ("portrait-9x16", portrait),
    ]
}

/// The fixture rendered through the real export filter graph.
fn render_ffmpeg(ffmpeg: &Path, source: &Path, state: &RenderState) -> Result<Vec<u8>, String> {
    let geom = compute_canvas_geometry(SRC_W, SRC_H, state.padding, state.output_aspect.as_deref());
    let plan = RenderGraph::from_state(state)
        .build_export_plan_with(
            SourceVideoMetadata {
                width: SRC_W,
                height: SRC_H,
                fps: FPS as f64,
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
        .map(|s| s.to_string())
        .collect();
    args.push(source.to_string_lossy().into_owned());
    for input in &plan.extra_inputs {
        for arg in ["-framerate", "30", "-loop", "1", "-i"] {
            args.push(arg.to_string());
        }
        args.push(input.to_string_lossy().into_owned());
    }
    if let Some(fc) = &plan.filter_complex {
        args.push("-filter_complex".into());
        args.push(fc.clone());
        args.push("-map".into());
        args.push(format!("[{}]", plan.video_map.trim_matches(['[', ']'])));
    }
    for arg in [
        "-frames:v",
        "45",
        "-an",
        "-f",
        "rawvideo",
        "-pix_fmt",
        "rgba",
        "-",
    ] {
        args.push(arg.to_string());
    }

    let output = Command::new(ffmpeg)
        .args(&args)
        .stdin(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }

    let frame_bytes = (geom.canvas_w * geom.canvas_h * 4) as usize;
    output
        .stdout
        .chunks(frame_bytes)
        .filter(|c| c.len() == frame_bytes)
        .nth(SAMPLE_FRAME as usize)
        .map(<[u8]>::to_vec)
        .ok_or_else(|| format!("frame {SAMPLE_FRAME} missing"))
}

/// The decoded source frame the compositor composites, taken from the same file
/// FFmpeg reads so neither engine gets a different picture to start from.
fn decode_source_frame(ffmpeg: &Path, source: &Path) -> Result<Vec<u8>, String> {
    let frames = media::read_frames(ffmpeg, source, SRC_W, SRC_H)?;
    frames
        .into_iter()
        .nth(SAMPLE_FRAME as usize)
        .ok_or_else(|| format!("source frame {SAMPLE_FRAME} missing"))
}

fn upload(ctx: &GpuContext, pixels: &[u8]) -> wgpu::Texture {
    let texture = ctx.device().create_texture(&wgpu::TextureDescriptor {
        label: Some("parity-source"),
        size: wgpu::Extent3d {
            width: SRC_W,
            height: SRC_H,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    ctx.queue().write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        pixels,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(SRC_W * 4),
            rows_per_image: Some(SRC_H),
        },
        wgpu::Extent3d {
            width: SRC_W,
            height: SRC_H,
            depth_or_array_layers: 1,
        },
    );
    texture
}

fn read_back(ctx: &GpuContext, texture: &wgpu::Texture, width: u32, height: u32) -> Vec<u8> {
    let bytes_per_row = recast_gpu::aligned_bytes_per_row(width, wgpu::TextureFormat::Rgba8Unorm);
    let buffer = ctx.device().create_buffer(&wgpu::BufferDescriptor {
        label: Some("parity-readback"),
        size: (bytes_per_row * height) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = ctx.device().create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(bytes_per_row),
                rows_per_image: Some(height),
            },
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    ctx.queue().submit([encoder.finish()]);

    let slice = buffer.slice(..);
    slice.map_async(wgpu::MapMode::Read, |_| {});
    ctx.device()
        .poll(wgpu::PollType::wait_indefinitely())
        .expect("poll");
    let mapped = slice.get_mapped_range().expect("map readback");
    let mut out = Vec::with_capacity((width * height * 4) as usize);
    for row in 0..height {
        let start = (row * bytes_per_row) as usize;
        out.extend_from_slice(&mapped[start..start + (width * 4) as usize]);
    }
    drop(mapped);
    buffer.unmap();
    out
}

fn render_compositor(ctx: &GpuContext, state: &RenderState, source_pixels: &[u8]) -> Vec<u8> {
    let scene = to_scene(state);
    let evaluator = Evaluator::new(
        &scene,
        SourceGeometry {
            width: SRC_W,
            height: SRC_H,
        },
    );
    let params = evaluator.evaluate(&scene, SAMPLE_FRAME as f64 / FPS as f64);
    let (width, height) = (params.geometry.canvas_w, params.geometry.canvas_h);

    let mut compositor = Compositor::new(ctx).expect("compositor");
    let target = compositor.output_texture(width, height);
    let source = upload(ctx, source_pixels);
    let view = source.create_view(&Default::default());

    let screen = scene
        .layers
        .iter()
        .find(|l| matches!(l.source, LayerSource::Screen))
        .expect("screen layer");
    let mut inputs = FrameInputs::new();
    inputs.set(
        screen.id,
        LayerInput {
            view: &view,
            needs_srgb_decode: true,
        },
    );

    compositor.render(&params, &inputs, &target.create_view(&Default::default()));
    read_back(ctx, &target, width, height)
}

#[test]
#[ignore = "measurement, not a gate: run with --ignored --nocapture"]
fn the_two_engines_are_diffed_fixture_by_fixture() {
    let Some(ffmpeg) = media::ffmpeg_path() else {
        eprintln!("skipping: no ffmpeg");
        return;
    };
    let Ok(ctx) = GpuContext::new_blocking(GpuOptions {
        require_hardware: false,
        ..Default::default()
    }) else {
        eprintln!("skipping: no GPU adapter");
        return;
    };

    let scratch = Scratch::new("engines");
    let source_path = scratch.0.join("source.mp4");
    media::write_source(
        &ffmpeg,
        SourceSpec {
            width: SRC_W,
            height: SRC_H,
            fps: FPS,
            duration_secs: 1.6,
            ..Default::default()
        },
        &source_path,
    )
    .expect("synthetic source");

    let source_pixels = decode_source_frame(&ffmpeg, &source_path).expect("decode source frame");

    println!();
    println!("adapter: {}", ctx.info().name);
    println!();
    println!("| fixture | max channel | mean channel | differing px | of total |");
    println!("|---|---|---|---|---|");

    let mut worst_mean = 0.0f64;
    for (name, state) in fixtures() {
        let ffmpeg_frame = match render_ffmpeg(&ffmpeg, &source_path, &state) {
            Ok(frame) => frame,
            Err(e) => {
                println!("| {name} | ffmpeg failed: {e} | | | |");
                continue;
            }
        };
        let compositor_frame = render_compositor(&ctx, &state, &source_pixels);

        let geom =
            compute_canvas_geometry(SRC_W, SRC_H, state.padding, state.output_aspect.as_deref());
        if state.background_type == "gradient" && is_flat(&ffmpeg_frame, geom.canvas_w) {
            println!(
                "| {name} | NOT COMPARABLE: the FFmpeg side rendered a flat background | | | |"
            );
            continue;
        }

        match compare::frame_delta(&ffmpeg_frame, &compositor_frame) {
            Some(delta) => {
                worst_mean = worst_mean.max(delta.mean_channel);
                println!(
                    "| {name} | {} | {:.3} | {} | {} |",
                    delta.max_channel,
                    delta.mean_channel,
                    delta.differing_pixels,
                    delta.total_pixels
                );
            }
            None => println!(
                "| {name} | SIZE MISMATCH: ffmpeg {} bytes, compositor {} bytes | | | |",
                ffmpeg_frame.len(),
                compositor_frame.len()
            ),
        }
    }
    println!();
    println!("worst mean channel delta: {worst_mean:.3}");
    println!();
}

/// True when the TOP ROW does not vary. Gradients and images are pre-rasterised
/// by `commands::export::raster`, which is private and is NOT reachable through
/// `build_export_plan_with`, so a gradient fixture comes out of this harness
/// flat. The top row is entirely background whenever there is padding, so its
/// spread is the cheapest reliable check. Detected and reported rather than
/// diffed, because a 255-channel delta against a background that was never drawn
/// is not a parity signal.
fn is_flat(frame: &[u8], width: u32) -> bool {
    let row: Vec<&[u8]> = frame.chunks_exact(4).take(width as usize).collect();
    let Some(first) = row.first() else {
        return true;
    };
    let spread = |channel: usize| {
        let values = row.iter().map(|p| p[channel]);
        let max = values.clone().max().unwrap_or(first[channel]);
        let min = values.min().unwrap_or(first[channel]);
        max.abs_diff(min)
    };
    (0..3).all(|channel| spread(channel) < 8)
}

/// Whatever the pixels do, the two engines must agree on the canvas SIZE. A
/// disagreement there is a geometry bug, not a colour-space difference, and it
/// would make every delta above meaningless.
#[test]
fn both_engines_agree_on_the_canvas_geometry_for_every_fixture() {
    for (name, state) in fixtures() {
        let ffmpeg_geom =
            compute_canvas_geometry(SRC_W, SRC_H, state.padding, state.output_aspect.as_deref());
        let scene = to_scene(&state);
        let compositor_geom = Evaluator::new(
            &scene,
            SourceGeometry {
                width: SRC_W,
                height: SRC_H,
            },
        )
        .geometry();

        assert_eq!(
            (ffmpeg_geom.canvas_w, ffmpeg_geom.canvas_h),
            (compositor_geom.canvas_w, compositor_geom.canvas_h),
            "{name}: canvas size"
        );
        assert_eq!(
            (ffmpeg_geom.video_x, ffmpeg_geom.video_y),
            (compositor_geom.video_x, compositor_geom.video_y),
            "{name}: video origin"
        );
    }
}
