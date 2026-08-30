//! Frame-time budgets for the compositor.
//!
//! The export target is roughly 8x realtime at 1080p60, which is 2 ms of
//! compositing per frame with everything else free. Nothing is free, so the
//! budget asserted here is deliberately loose: this is a REGRESSION gate, not a
//! performance target. It fires when a change makes the compositor several times
//! slower, which is the failure mode that ships unnoticed.
//!
//! `--ignored --nocapture` prints the measured numbers.

use std::time::Instant;

use recast_compositor::{Compositor, Evaluator, FrameInputs, LayerInput, SourceGeometry};
use recast_gpu::{GpuContext, GpuOptions};
use recast_scene::migrate::to_scene;
use recast_scene::v1::RenderState;
use recast_scene::{LayerSource, Scene};

/// Warm-up frames, discarded. The first render compiles pipelines and allocates,
/// and timing that measures the shader compiler rather than the compositor.
const WARMUP: usize = 10;
const SAMPLES: usize = 60;

/// Generous on purpose. A hardware GPU renders these in a fraction of it, and a
/// software adapter in CI is far slower; the gate has to hold on both without
/// being retuned every time a runner changes.
const BUDGET_MS: f64 = 12.0;

const BASE: &str = r##"{
    "trimStart": 0.0, "trimEnd": 10.0,
    "backgroundType": "color", "backgroundValue": "#1e293b", "backgroundBlur": 0.0,
    "padding": 0.0, "cursorEnabled": false, "cursorSize": 3.0, "cursorSmoothing": 50.0,
    "cursorHighlightClicks": true, "cursorHighlightColor": "#3b82f6",
    "cursorHighlightOpacity": 40.0, "cursorHideWhenIdle": false, "cursorIdleTimeout": 3.0,
    "zoomRegions": []
}"##;

/// One device for the whole binary. A context per test means one wgpu device
/// per test running at once, which on a machine with no GPU all land on the
/// same software adapter, and here would also charge every measurement for its
/// own device setup.
fn context() -> Option<&'static GpuContext> {
    static SHARED: std::sync::OnceLock<Option<GpuContext>> = std::sync::OnceLock::new();
    SHARED
        .get_or_init(|| {
            match GpuContext::new_blocking(GpuOptions {
                require_hardware: false,
                ..Default::default()
            }) {
                Ok(ctx) => Some(ctx),
                Err(e) => {
                    eprintln!("skipping: no GPU adapter ({e})");
                    None
                }
            }
        })
        .as_ref()
}

fn scene_with(extra: &str) -> Scene {
    let mut base: serde_json::Value = serde_json::from_str(BASE).expect("base json");
    let fragment = extra.trim().trim_end_matches(',');
    if !fragment.is_empty() {
        let overrides: serde_json::Value =
            serde_json::from_str(&format!("{{{fragment}}}")).expect("override json");
        let (Some(base), Some(overrides)) = (base.as_object_mut(), overrides.as_object()) else {
            panic!("fixtures must be JSON objects");
        };
        for (key, value) in overrides {
            base.insert(key.clone(), value.clone());
        }
    }
    let state: RenderState = serde_json::from_value(base).expect("render state");
    to_scene(&state)
}

fn source_texture(ctx: &GpuContext, width: u32, height: u32) -> wgpu::Texture {
    let size = wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };
    let texture = ctx.device().create_texture(&wgpu::TextureDescriptor {
        label: Some("perf-source"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let pixels = vec![128u8; (width * height * 4) as usize];
    ctx.queue().write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &pixels,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(width * 4),
            rows_per_image: Some(height),
        },
        size,
    );
    texture
}

/// Median milliseconds per frame, including the wait for the GPU to finish.
///
/// Timing without the wait measures how fast we can QUEUE work, which stays flat
/// while the shader behind it gets arbitrarily slower.
fn frame_time_ms(ctx: &GpuContext, scene: &Scene, width: u32, height: u32) -> f64 {
    let ev = Evaluator::new(scene, SourceGeometry { width, height });
    let mut compositor = Compositor::new(ctx).expect("compositor");
    let source = source_texture(ctx, width, height);
    let source_view = source.create_view(&Default::default());
    let screen = scene
        .layers
        .iter()
        .find(|l| matches!(l.source, LayerSource::Screen))
        .expect("screen layer");

    let params = ev.evaluate(scene, 1.0);
    let target = compositor.output_texture(params.geometry.canvas_w, params.geometry.canvas_h);
    let view = target.create_view(&Default::default());

    let mut once = |time: f64| {
        let params = ev.evaluate(scene, time);
        let mut inputs = FrameInputs::new();
        inputs.set(
            screen.id,
            LayerInput {
                view: &source_view,
                needs_srgb_decode: true,
            },
        );
        compositor.render(&params, &inputs, &view);
        ctx.device()
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("poll");
    };

    for i in 0..WARMUP {
        once(1.0 + i as f64 * 0.01);
    }
    let mut times = Vec::with_capacity(SAMPLES);
    for i in 0..SAMPLES {
        let start = Instant::now();
        once(2.0 + i as f64 * 0.01);
        times.push(start.elapsed().as_secs_f64() * 1000.0);
    }
    times.sort_by(|a, b| a.partial_cmp(b).expect("no NaN timings"));
    times[times.len() / 2]
}

fn cases() -> Vec<(&'static str, u32, u32, String)> {
    vec![
        ("1080p plain", 1920, 1080, String::new()),
        (
            "1080p padded + zoom",
            1920,
            1080,
            r##""padding": 40.0, "borderRadius": 24.0,
                "zoomRegions": [{"start":0.0,"end":10.0,"scale":1.6,"rampIn":0.5,
                                "rampOut":0.5,"centerX":0.35,"centerY":0.5}],"##
                .into(),
        ),
        (
            "1080p shadow + blur background",
            1920,
            1080,
            r##""padding": 40.0, "borderRadius": 24.0, "backgroundBlur": 30.0,
                "shadow": {"enabled": true, "blur": 24.0, "spread": 0.0, "offsetY": 10.0,
                           "opacity": 70.0, "color": "#000000"},"##
                .into(),
        ),
        ("4k plain", 3840, 2160, String::new()),
    ]
}

#[test]
fn every_case_renders_inside_the_frame_budget() {
    let Some(ctx) = context() else { return };
    let software = ctx.is_software();
    let mut over = Vec::new();
    for (name, width, height, extra) in cases() {
        let ms = frame_time_ms(ctx, &scene_with(&extra), width, height);
        // 4K is four times the pixels, and the budget scales with them rather than pretending one number fits every resolution.
        let budget = if width > 2000 {
            BUDGET_MS * 4.0
        } else {
            BUDGET_MS
        };
        eprintln!("{name}: {ms:.2} ms (budget {budget:.1})");
        if ms > budget {
            over.push(format!("{name}: {ms:.2} ms over a {budget:.1} ms budget"));
        }
    }
    if software {
        // Reported, not enforced: a software adapter's numbers say nothing about a user's machine, and gating makes CI a coin toss.
        eprintln!("software adapter: budgets measured but not enforced");
        return;
    }
    assert!(over.is_empty(), "over budget:\n  {}", over.join("\n  "));
}

/// The numbers, for the record rather than as a gate.
#[test]
#[ignore = "measurement, not a gate: run with --ignored --nocapture"]
fn print_the_frame_times() {
    let Some(ctx) = context() else { return };
    eprintln!("adapter: {:?}", ctx.info().name);
    for (name, width, height, extra) in cases() {
        let ms = frame_time_ms(ctx, &scene_with(&extra), width, height);
        eprintln!("{name:34} {ms:7.2} ms  {:6.1} fps", 1000.0 / ms);
    }
}
