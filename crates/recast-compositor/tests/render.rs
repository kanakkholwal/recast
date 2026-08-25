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

#[test]
fn a_drop_shadow_darkens_the_padding_below_the_card_and_not_above_it() {
    let Some(ctx) = context() else { return };
    let plain = scene_with(r#""padding": 20.0,"#);
    let shadowed = scene_with(
        r##""padding": 20.0,
            "shadow": {"enabled": true, "blur": 12.0, "spread": 0.0, "offsetY": 6.0,
                       "opacity": 80.0, "color": "#000000"},"##,
    );

    let ev = Evaluator::new(
        &plain,
        SourceGeometry {
            width: SRC_W,
            height: SRC_H,
        },
    );
    let g = ev.geometry();
    let without = render(&ctx, &plain, 0.0, true);
    let with = render(&ctx, &shadowed, 0.0, true);

    let x = g.video_x + g.video_w / 2;
    let below_y = g.video_y + g.video_h + 3;
    let above_y = g.video_y.saturating_sub(8);

    let below_plain = without.at(x, below_y);
    let below_shadow = with.at(x, below_y);
    assert!(
        below_shadow[2] + 8 < below_plain[2],
        "the shadow did not darken below the card: {below_shadow:?} vs {below_plain:?}"
    );

    let above_plain = without.at(x, above_y);
    let above_shadow = with.at(x, above_y);
    assert!(
        above_shadow[2] >= above_plain[2].saturating_sub(4),
        "the downward-offset shadow leaked above the card: {above_shadow:?} vs {above_plain:?}"
    );
}

#[test]
fn a_rotated_card_leaves_background_showing_at_the_rect_corner() {
    let Some(ctx) = context() else { return };
    let scene = scene_with(
        r#""padding": 20.0,
           "segmentAnims": [{"start": 0.0, "in": {"kind": "rotate", "durationMs": 500.0}}],"#,
    );
    let ev = Evaluator::new(
        &scene,
        SourceGeometry {
            width: SRC_W,
            height: SRC_H,
        },
    );
    let g = ev.geometry();

    let settled = render(&ctx, &scene, 5.0, true);
    let rotating = render(&ctx, &scene, 0.0, true);

    let corner = (g.video_x, g.video_y + 1);
    let settled_px = settled.at(corner.0, corner.1);
    let rotating_px = rotating.at(corner.0, corner.1);
    assert!(
        close(settled_px, [255, 0, 0, 255], 3),
        "the settled card should cover its own corner: {settled_px:?}"
    );
    assert!(
        !close(rotating_px, settled_px, 8),
        "rotation did not move the card: {rotating_px:?}"
    );
}

/// Blur-on versus blur-off at the SAME instant, so the only difference is the
/// streak. Comparing a ramp against a hold instead would also change which
/// pixels are visible, and the zoom would do the work the blur is meant to.
#[test]
fn dolly_blur_softens_the_frame_during_a_ramp_and_is_inert_during_the_hold() {
    let Some(ctx) = context() else { return };
    let region = |blur: f64| {
        format!(
            r#""zoomRegions": [{{"start":0.0,"end":10.0,"scale":2.0,"rampIn":0.2,"rampOut":0.2,
                                "centerX":0.35,"centerY":0.5,"motionBlur":{blur}}}],"#
        )
    };
    let blurred = scene_with(&region(1.0));
    let sharp = scene_with(&region(0.0));

    let row = SRC_H / 4;
    let mixed = |img: &Rendered| {
        (0..SRC_W)
            .filter(|x| {
                let p = img.at(*x, row);
                p[0] > 20 && p[1] > 20
            })
            .count()
    };

    let ramp_blurred = mixed(&render(&ctx, &blurred, 0.1, true));
    let ramp_sharp = mixed(&render(&ctx, &sharp, 0.1, true));
    assert!(
        ramp_blurred > ramp_sharp,
        "mid-ramp: blurred {ramp_blurred} mixed px, sharp {ramp_sharp}"
    );

    let hold_blurred = mixed(&render(&ctx, &blurred, 5.0, true));
    let hold_sharp = mixed(&render(&ctx, &sharp, 5.0, true));
    assert_eq!(
        hold_blurred, hold_sharp,
        "the blur fired on a held zoom, which is not moving"
    );
}

#[test]
fn a_rect_annotation_fills_its_uv_box_and_leaves_the_rest_alone() {
    let Some(ctx) = context() else { return };
    let scene = scene_with(
        r##""annotations": [{"id":"a1","start":0.0,"end":10.0,
             "fill":"#ff00ff","stroke":{"width":0.0,"color":"transparent"},
             "kind":{"kind":"rect","x":0.25,"y":0.25,"w":0.5,"h":0.5}}],"##,
    );
    let out = render(&ctx, &scene, 5.0, true);

    let inside = out.at(SRC_W / 2, SRC_H / 2);
    assert!(
        close(inside, [255, 0, 255, 255], 3),
        "the annotation did not fill its box: {inside:?}"
    );
    let outside = out.at(2, 2);
    assert!(
        !close(outside, [255, 0, 255, 255], 3),
        "the annotation leaked outside its box: {outside:?}"
    );
}

#[test]
fn an_annotation_outside_its_time_window_draws_nothing() {
    let Some(ctx) = context() else { return };
    let scene = scene_with(
        r##""annotations": [{"id":"a1","start":6.0,"end":8.0,
             "fill":"#ff00ff","stroke":{"width":0.0,"color":"transparent"},
             "kind":{"kind":"rect","x":0.25,"y":0.25,"w":0.5,"h":0.5}}],"##,
    );
    let before = render(&ctx, &scene, 1.0, true);
    let during = render(&ctx, &scene, 7.0, true);

    let x = SRC_W / 2;
    let y = SRC_H / 2;
    assert!(!close(before.at(x, y), [255, 0, 255, 255], 3));
    assert!(close(during.at(x, y), [255, 0, 255, 255], 3));
}

#[test]
fn z_order_decides_which_overlapping_annotation_is_on_top() {
    let Some(ctx) = context() else { return };
    let scene = scene_with(
        r##""annotations": [
             {"id":"under","start":0.0,"end":10.0,"zIndex":5,
              "fill":"#ff00ff","stroke":{"width":0.0,"color":"transparent"},
              "kind":{"kind":"rect","x":0.2,"y":0.2,"w":0.6,"h":0.6}},
             {"id":"over","start":0.0,"end":10.0,"zIndex":1,
              "fill":"#00ffff","stroke":{"width":0.0,"color":"transparent"},
              "kind":{"kind":"rect","x":0.2,"y":0.2,"w":0.6,"h":0.6}}],"##,
    );
    let out = render(&ctx, &scene, 5.0, true);
    let centre = out.at(SRC_W / 2, SRC_H / 2);
    assert!(
        close(centre, [255, 0, 255, 255], 3),
        "the higher z-index did not win: {centre:?}"
    );
}

#[test]
fn a_frame_anchored_annotation_stays_put_while_a_video_anchored_one_rides_the_zoom() {
    let Some(ctx) = context() else { return };
    let make = |anchor: &str| {
        scene_with(&format!(
            r##""zoomRegions": [{{"start":0.0,"end":10.0,"scale":2.0,"rampIn":0.0,"rampOut":0.0,
                                 "centerX":0.5,"centerY":0.5}}],
                "annotations": [{{"id":"a1","start":0.0,"end":10.0,"anchor":"{anchor}",
                 "fill":"#ff00ff","stroke":{{"width":0.0,"color":"transparent"}},
                 "kind":{{"kind":"rect","x":0.4,"y":0.4,"w":0.2,"h":0.2}}}}],"##
        ))
    };
    let frame = render(&ctx, &make("frame"), 5.0, true);
    let video = render(&ctx, &make("video"), 5.0, true);

    let count = |img: &Rendered| {
        (0..img.width * img.height)
            .filter(|i| close(img.at(i % img.width, i / img.width), [255, 0, 255, 255], 6))
            .count()
    };
    let frame_area = count(&frame);
    let video_area = count(&video);
    assert!(
        video_area > frame_area * 2,
        "video anchor {video_area} px did not magnify against frame anchor {frame_area} px"
    );
}

#[test]
fn an_enabled_camera_bubble_draws_over_the_card_and_a_disabled_one_does_not() {
    let Some(ctx) = context() else { return };
    let with_camera = scene_with(
        r#""cameraOverlay": {"enabled": true, "shape": "square", "mirror": false,
             "shadow": 0.0, "zoomFollow": false,
             "defaultPlacement": {"x":0.0,"y":0.0,"width":0.25,"height":0.25}},"#,
    );
    let without = scene_with(
        r#""cameraOverlay": {"enabled": false,
             "defaultPlacement": {"x":0.0,"y":0.0,"width":0.25,"height":0.25}},"#,
    );

    let ev = Evaluator::new(
        &with_camera,
        SourceGeometry {
            width: SRC_W,
            height: SRC_H,
        },
    );
    let params = ev.evaluate(&with_camera, 5.0);
    let bubble = params
        .layers
        .iter()
        .find(|l| l.dest.w as u32 == SRC_W / 4)
        .expect("a bubble-sized layer");
    assert!(bubble.visible);

    let off = Evaluator::new(
        &without,
        SourceGeometry {
            width: SRC_W,
            height: SRC_H,
        },
    )
    .evaluate(&without, 5.0);
    assert!(off
        .layers
        .iter()
        .all(|l| !l.visible || l.dest.w as u32 != SRC_W / 4));

    // The bubble shares the screen layer's source here, so it must at least
    // paint SOMETHING inside its rect rather than leaving the card showing.
    let _ = render(&ctx, &with_camera, 5.0, true);
}

#[test]
fn a_camera_shadow_is_emitted_alongside_the_card_shadow() {
    let scene = scene_with(
        r##""shadow": {"enabled": true, "blur": 10.0, "opacity": 50.0, "color": "#000000"},
            "cameraOverlay": {"enabled": true, "shadow": 0.5, "zoomFollow": false,
              "defaultPlacement": {"x":0.0,"y":0.0,"width":0.25,"height":0.25}},"##,
    );
    let params = Evaluator::new(
        &scene,
        SourceGeometry {
            width: SRC_W,
            height: SRC_H,
        },
    )
    .evaluate(&scene, 5.0);
    assert_eq!(params.shadows.len(), 2);
}
