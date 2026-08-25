use recast_compositor::{Compositor, Evaluator, FrameInputs, LayerInput, SourceGeometry};
use recast_gpu::{GpuContext, GpuOptions};
use recast_scene::migrate::to_scene;
use recast_scene::v1::RenderState;
use recast_scene::{LayerId, LayerSource, Scene};

const SRC_W: u32 = 64;
const SRC_H: u32 = 32;
const MID_GREY: u8 = 128;

fn context() -> Option<GpuContext> {
    match GpuContext::new_blocking(GpuOptions {
        require_hardware: false,
        ..Default::default()
    }) {
        Ok(ctx) => Some(ctx),
        Err(e) => {
            if std::env::var("RECAST_GPU_REQUIRE_ADAPTER").as_deref() == Ok("1") {
                panic!("RECAST_GPU_REQUIRE_ADAPTER=1 but no adapter: {e}");
            }
            eprintln!("skipping: no GPU adapter ({e})");
            None
        }
    }
}

const BASE: &str = r##"{
    "trimStart": 0.0, "trimEnd": 10.0,
    "backgroundType": "color", "backgroundValue": "#0000ff", "backgroundBlur": 0.0,
    "padding": 0.0, "cursorEnabled": false, "cursorSize": 3.0, "cursorSmoothing": 50.0,
    "cursorHighlightClicks": true, "cursorHighlightColor": "#3b82f6",
    "cursorHighlightOpacity": 40.0, "cursorHideWhenIdle": false, "cursorIdleTimeout": 3.0,
    "zoomRegions": []
}"##;

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

/// Top half: pure red | pure green, so a horizontal zoom shows as a moved seam.
/// Bottom half: mid grey, which is the only value that can catch a dropped sRGB
/// decode or encode. Pure primaries are fixed points of the transfer curve and
/// would pass whatever the colour pipeline did.
fn source_texture(ctx: &GpuContext) -> wgpu::Texture {
    let mut pixels = vec![0u8; (SRC_W * SRC_H * 4) as usize];
    for y in 0..SRC_H {
        for x in 0..SRC_W {
            let offset = ((y * SRC_W + x) * 4) as usize;
            let colour = if y >= SRC_H / 2 {
                [MID_GREY, MID_GREY, MID_GREY, 255]
            } else if x < SRC_W / 2 {
                [255, 0, 0, 255]
            } else {
                [0, 255, 0, 255]
            };
            pixels[offset..offset + 4].copy_from_slice(&colour);
        }
    }

    let texture = ctx.device().create_texture(&wgpu::TextureDescriptor {
        label: Some("source"),
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
        &pixels,
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
        label: Some("readback"),
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

struct Rendered {
    pixels: Vec<u8>,
    width: u32,
    height: u32,
}

impl Rendered {
    fn at(&self, x: u32, y: u32) -> [u8; 4] {
        let offset = ((y * self.width + x) * 4) as usize;
        [
            self.pixels[offset],
            self.pixels[offset + 1],
            self.pixels[offset + 2],
            self.pixels[offset + 3],
        ]
    }
}

fn render(ctx: &GpuContext, scene: &Scene, output_time: f64, with_source: bool) -> Rendered {
    let ev = Evaluator::new(
        scene,
        SourceGeometry {
            width: SRC_W,
            height: SRC_H,
        },
    );
    let params = ev.evaluate(scene, output_time);
    let (width, height) = (params.geometry.canvas_w, params.geometry.canvas_h);

    let mut compositor = Compositor::new(ctx).expect("compositor");
    let target = compositor.output_texture(width, height);
    let source = source_texture(ctx);
    let source_view = source.create_view(&Default::default());

    let mut inputs = FrameInputs::new();
    if with_source {
        let screen = scene
            .layers
            .iter()
            .find(|l| matches!(l.source, LayerSource::Screen))
            .expect("screen layer");
        inputs.set(
            screen.id,
            LayerInput {
                view: &source_view,
                needs_srgb_decode: true,
            },
        );
    }

    compositor.render(&params, &inputs, &target.create_view(&Default::default()));
    Rendered {
        pixels: read_back(ctx, &target, width, height),
        width,
        height,
    }
}

fn close(got: [u8; 4], want: [u8; 4], tolerance: u8) -> bool {
    got.iter()
        .zip(want)
        .all(|(g, w)| g.abs_diff(w) <= tolerance)
}

#[test]
fn a_solid_background_renders_the_authored_colour() {
    let Some(ctx) = context() else { return };
    let scene = scene_with(r#""padding": 20.0,"#);
    let out = render(&ctx, &scene, 0.0, false);

    let corner = out.at(1, 1);
    assert!(
        close(corner, [0, 0, 255, 255], 2),
        "background corner was {corner:?}, expected blue"
    );
}

#[test]
fn the_source_lands_in_the_video_rect_with_its_colours_intact() {
    let Some(ctx) = context() else { return };
    let scene = scene_with(r#""padding": 20.0,"#);
    let ev = Evaluator::new(
        &scene,
        SourceGeometry {
            width: SRC_W,
            height: SRC_H,
        },
    );
    let g = ev.geometry();
    let out = render(&ctx, &scene, 0.0, true);

    let left = out.at(g.video_x + 2, g.video_y + g.video_h / 4);
    let right = out.at(g.video_x + g.video_w - 3, g.video_y + g.video_h / 4);
    assert!(close(left, [255, 0, 0, 255], 2), "left half was {left:?}");
    assert!(
        close(right, [0, 255, 0, 255], 2),
        "right half was {right:?}"
    );

    let outside = out.at(1, 1);
    assert!(
        close(outside, [0, 0, 255, 255], 2),
        "the card leaked into the padding: {outside:?}"
    );
}

#[test]
fn a_zoom_moves_the_seam_because_the_shader_samples_a_smaller_window() {
    let Some(ctx) = context() else { return };
    let unzoomed = scene_with("");
    let zoomed = scene_with(
        r#""zoomRegions": [{"start":0.0,"end":10.0,"scale":2.0,"rampIn":0.0,"rampOut":0.0,"centerX":0.25,"centerY":0.5}],"#,
    );

    let plain = render(&ctx, &unzoomed, 5.0, true);
    let zoom = render(&ctx, &zoomed, 5.0, true);

    let row = SRC_H / 4;
    // Centre 0.25 with a half-width window puts the sampled window at u 0..0.5,
    // which is entirely inside the red half.
    let sample = zoom.at(SRC_W - 3, row);
    assert!(
        close(sample, [255, 0, 0, 255], 2),
        "zoomed right edge was {sample:?}, expected red"
    );
    let plain_sample = plain.at(SRC_W - 3, row);
    assert!(
        close(plain_sample, [0, 255, 0, 255], 2),
        "unzoomed right edge was {plain_sample:?}, expected green"
    );
}

#[test]
fn a_gradient_background_varies_along_its_angle() {
    let Some(ctx) = context() else { return };
    let scene = scene_with(
        r#""backgroundType": "gradient",
           "backgroundValue": "linear-gradient(90deg, #ff0000 0%, #0000ff 100%)",
           "padding": 20.0,"#,
    );
    let out = render(&ctx, &scene, 0.0, false);

    let left = out.at(1, out.height / 2);
    let right = out.at(out.width - 2, out.height / 2);
    assert!(left[0] > 200 && left[2] < 60, "left end was {left:?}");
    assert!(right[2] > 200 && right[0] < 60, "right end was {right:?}");
}

#[test]
fn a_corner_radius_cuts_the_card_corner_without_touching_its_centre() {
    let Some(ctx) = context() else { return };
    let scene = scene_with(r#""padding": 20.0, "borderRadius": 40.0,"#);
    let ev = Evaluator::new(
        &scene,
        SourceGeometry {
            width: SRC_W,
            height: SRC_H,
        },
    );
    let g = ev.geometry();
    let out = render(&ctx, &scene, 0.0, true);

    let corner = out.at(g.video_x, g.video_y);
    assert!(
        close(corner, [0, 0, 255, 255], 4),
        "the card corner was not rounded away: {corner:?}"
    );
    let centre = out.at(g.video_x + 2, g.video_y + g.video_h / 4);
    assert!(
        close(centre, [255, 0, 0, 255], 2),
        "the radius ate the card body: {centre:?}"
    );
}

#[test]
fn a_layer_with_no_decoded_frame_is_skipped_rather_than_drawn_black() {
    let Some(ctx) = context() else { return };
    let scene = scene_with(r#""padding": 20.0,"#);
    let ev = Evaluator::new(
        &scene,
        SourceGeometry {
            width: SRC_W,
            height: SRC_H,
        },
    );
    let params = ev.evaluate(&scene, 0.0);
    let (width, height) = (params.geometry.canvas_w, params.geometry.canvas_h);

    let mut compositor = Compositor::new(&ctx).expect("compositor");
    let target = compositor.output_texture(width, height);
    let stats = compositor.render(
        &params,
        &FrameInputs::new(),
        &target.create_view(&Default::default()),
    );

    assert_eq!(stats.layers_drawn, 0);
    assert!(stats.layers_skipped > 0);

    let out = Rendered {
        pixels: read_back(&ctx, &target, width, height),
        width,
        height,
    };
    let centre = out.at(width / 2, height / 2);
    assert!(
        close(centre, [0, 0, 255, 255], 2),
        "a missing frame punched a hole: {centre:?}"
    );
}

#[test]
fn an_unknown_layer_id_in_the_inputs_does_not_draw_anything() {
    let Some(ctx) = context() else { return };
    let scene = scene_with("");
    let ev = Evaluator::new(
        &scene,
        SourceGeometry {
            width: SRC_W,
            height: SRC_H,
        },
    );
    let params = ev.evaluate(&scene, 0.0);
    let mut compositor = Compositor::new(&ctx).expect("compositor");
    let target = compositor.output_texture(params.geometry.canvas_w, params.geometry.canvas_h);
    let source = source_texture(&ctx);
    let view = source.create_view(&Default::default());

    let mut inputs = FrameInputs::new();
    inputs.set(
        LayerId(9999),
        LayerInput {
            view: &view,
            needs_srgb_decode: true,
        },
    );

    let stats = compositor.render(&params, &inputs, &target.create_view(&Default::default()));
    assert_eq!(stats.layers_drawn, 0);
}

/// The only assertion in this file that can see the colour pipeline. Mid grey
/// goes sRGB -> linear on the way in and linear -> sRGB on the way out; drop
/// either and it lands at 55 or 187 instead of 128.
#[test]
fn a_mid_grey_source_survives_the_linear_working_space_unchanged() {
    let Some(ctx) = context() else { return };
    let scene = scene_with("");
    let out = render(&ctx, &scene, 0.0, true);

    let grey = out.at(SRC_W / 2, SRC_H * 3 / 4);
    assert!(
        close(grey, [MID_GREY, MID_GREY, MID_GREY, 255], 2),
        "mid grey came back as {grey:?}; the sRGB decode or encode is missing"
    );
}

#[test]
fn a_mid_grey_background_survives_the_linear_working_space_unchanged() {
    let Some(ctx) = context() else { return };
    let scene = scene_with(r##""backgroundValue": "#808080", "padding": 20.0,"##);
    let out = render(&ctx, &scene, 0.0, false);

    let grey = out.at(1, 1);
    assert!(
        close(grey, [MID_GREY, MID_GREY, MID_GREY, 255], 2),
        "mid grey background came back as {grey:?}"
    );
}
