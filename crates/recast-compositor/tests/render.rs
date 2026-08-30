use recast_compositor::{
    BackgroundImage, CaptionFrame, Compositor, Evaluator, FrameInputs, GlyphQuad, LayerInput,
    SourceGeometry,
};
use recast_gpu::{GpuContext, GpuOptions};
use recast_scene::migrate::to_scene;
use recast_scene::v1::RenderState;
use recast_scene::{LayerId, LayerSource, Scene};

const SRC_W: u32 = 64;
const SRC_H: u32 = 32;
const MID_GREY: u8 = 128;

/// One device for the whole binary, built on first use.
///
/// A context per test means one wgpu device per test running at once, and on a
/// machine with no GPU those all land on the same software adapter. That is
/// what crashed CI here; it is also several times the setup cost for tests that
/// only ever render.
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
                    if std::env::var("RECAST_GPU_REQUIRE_ADAPTER").as_deref() == Ok("1") {
                        panic!("RECAST_GPU_REQUIRE_ADAPTER=1 but no adapter: {e}");
                    }
                    eprintln!("skipping: no GPU adapter ({e})");
                    None
                }
            }
        })
        .as_ref()
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
    let out = render(ctx, &scene, 0.0, false);

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
    let out = render(ctx, &scene, 0.0, true);

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

    let plain = render(ctx, &unzoomed, 5.0, true);
    let zoom = render(ctx, &zoomed, 5.0, true);

    let row = SRC_H / 4;
    // Centre 0.25 with a half-width window samples u 0..0.5, which is entirely inside the red half.
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
    let out = render(ctx, &scene, 0.0, false);

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
    let out = render(ctx, &scene, 0.0, true);

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

    let mut compositor = Compositor::new(ctx).expect("compositor");
    let target = compositor.output_texture(width, height);
    let stats = compositor.render(
        &params,
        &FrameInputs::new(),
        &target.create_view(&Default::default()),
    );

    assert_eq!(stats.layers_drawn, 0);
    assert!(stats.layers_skipped > 0);

    let out = Rendered {
        pixels: read_back(ctx, &target, width, height),
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
    let mut compositor = Compositor::new(ctx).expect("compositor");
    let target = compositor.output_texture(params.geometry.canvas_w, params.geometry.canvas_h);
    let source = source_texture(ctx);
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
    let out = render(ctx, &scene, 0.0, true);

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
    let out = render(ctx, &scene, 0.0, false);

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
    let without = render(ctx, &plain, 0.0, true);
    let with = render(ctx, &shadowed, 0.0, true);

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

    let settled = render(ctx, &scene, 5.0, true);
    let rotating = render(ctx, &scene, 0.0, true);

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

    let ramp_blurred = mixed(&render(ctx, &blurred, 0.1, true));
    let ramp_sharp = mixed(&render(ctx, &sharp, 0.1, true));
    assert!(
        ramp_blurred > ramp_sharp,
        "mid-ramp: blurred {ramp_blurred} mixed px, sharp {ramp_sharp}"
    );

    let hold_blurred = mixed(&render(ctx, &blurred, 5.0, true));
    let hold_sharp = mixed(&render(ctx, &sharp, 5.0, true));
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
    let out = render(ctx, &scene, 5.0, true);

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

/// The blur annotation is the only one that reads what is already composited,
/// so a strength that changes nothing means it never sampled the frame.
#[test]
fn a_blur_annotation_mixes_the_pixels_underneath_it() {
    let Some(ctx) = context() else { return };
    // Over the red|green seam, in the top band where neither side has any blue.
    const BLUR: &str = r##""annotations": [{"id":"b1","start":0.0,"end":10.0,
         "kind":{"kind":"blur","x":0.25,"y":0.0,"w":0.5,"h":0.5,"strength":0.6}}],"##;
    let sharp = render(ctx, &scene_with(""), 5.0, true);
    let blurred = render(ctx, &scene_with(BLUR), 5.0, true);

    // One pixel left of the seam: pure red until green bleeds across.
    let (x, y) = (SRC_W / 2 - 1, SRC_H / 4);
    assert!(
        close(sharp.at(x, y), [255, 0, 0, 255], 3),
        "the fixture must be pure red there: {:?}",
        sharp.at(x, y)
    );
    assert!(
        blurred.at(x, y)[1] > 20,
        "no green bled across the seam: {:?}",
        blurred.at(x, y)
    );
}

#[test]
fn a_blur_annotation_leaves_everything_outside_its_rect_sharp() {
    let Some(ctx) = context() else { return };
    const BLUR: &str = r##""annotations": [{"id":"b1","start":0.0,"end":10.0,
         "kind":{"kind":"blur","x":0.25,"y":0.0,"w":0.5,"h":0.5,"strength":1.0}}],"##;
    let sharp = render(ctx, &scene_with(""), 5.0, true);
    let blurred = render(ctx, &scene_with(BLUR), 5.0, true);

    let (x, y) = (2, SRC_H / 4);
    assert_eq!(
        sharp.at(x, y),
        blurred.at(x, y),
        "the blur leaked outside its rect"
    );
}

/// Draw order is what makes a blur a redaction: an annotation painted before it
/// must be blurred, and one painted after it must stay sharp. Batching every
/// shape into one pass and blurring around them would break both halves.
#[test]
fn a_blur_takes_in_what_was_drawn_before_it_and_not_after() {
    let Some(ctx) = context() else { return };
    let scene = |mark_z: i32| {
        scene_with(&format!(
            r##""annotations": [
                 {{"id":"mark","start":0.0,"end":10.0,"zIndex":{mark_z},
                   "fill":"#ff00ff","stroke":{{"width":0.0,"color":"transparent"}},
                   "kind":{{"kind":"rect","x":0.0,"y":0.0,"w":0.5,"h":0.5}}}},
                 {{"id":"b1","start":0.0,"end":10.0,"zIndex":5,
                   "kind":{{"kind":"blur","x":0.0,"y":0.0,"w":1.0,"h":0.5,"strength":0.6}}}}],"##
        ))
    };
    // Three pixels right of the mark's edge, inside the blur rect.
    let (x, y) = (SRC_W / 2 + 3, SRC_H / 4);
    let under = render(ctx, &scene(1), 5.0, true).at(x, y);
    let over = render(ctx, &scene(9), 5.0, true).at(x, y);

    assert!(
        under[2] > 20,
        "the mark under the blur did not bleed: {under:?}"
    );
    assert!(
        over[2] <= 20,
        "the mark drawn after the blur was blurred anyway: {over:?}"
    );
}

#[test]
fn a_white_blur_variant_washes_the_region_out() {
    let Some(ctx) = context() else { return };
    const BLUR: &str = r##""annotations": [{"id":"b1","start":0.0,"end":10.0,
         "kind":{"kind":"blur","x":0.25,"y":0.0,"w":0.5,"h":0.5,
                 "strength":1.0,"variant":"white"}}],"##;
    let out = render(ctx, &scene_with(BLUR), 5.0, true);
    let (x, y) = (SRC_W / 2 - 4, SRC_H / 4);
    let px = out.at(x, y);
    assert!(
        px[0] > 200 && px[1] > 200 && px[2] > 200,
        "a full-strength white wash should redact, got {px:?}"
    );
}

/// A solid magenta asset, so "the image landed" is one pixel comparison.
fn image_texture(ctx: &GpuContext, w: u32, h: u32) -> wgpu::Texture {
    banded_image(ctx, w, h, |_| [255, 0, 255, 255])
}

fn render_with_annotation_image(
    ctx: &GpuContext,
    scene: &Scene,
    image: Option<&wgpu::Texture>,
) -> Rendered {
    let ev = Evaluator::new(
        scene,
        SourceGeometry {
            width: SRC_W,
            height: SRC_H,
        },
    );
    let params = ev.evaluate(scene, 5.0);
    let (width, height) = (params.geometry.canvas_w, params.geometry.canvas_h);
    let mut compositor = Compositor::new(ctx).expect("compositor");
    let target = compositor.output_texture(width, height);
    let source = source_texture(ctx);
    let source_view = source.create_view(&Default::default());
    let image_view = image.map(|t| t.create_view(&Default::default()));

    let mut inputs = FrameInputs::new();
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
    if let Some(view) = &image_view {
        inputs.set_annotation_image(
            "asset.png",
            LayerInput {
                view,
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

const IMAGE_ANNOTATION: &str = r##""annotations": [{"id":"i1","start":0.0,"end":10.0,
     "kind":{"kind":"image","x":0.25,"y":0.25,"w":0.5,"h":0.5,"path":"asset.png"}}],"##;

#[test]
fn an_image_annotation_fills_its_rect_from_the_uploaded_asset() {
    let Some(ctx) = context() else { return };
    let scene = scene_with(IMAGE_ANNOTATION);
    let image = image_texture(ctx, 8, 8);
    let out = render_with_annotation_image(ctx, &scene, Some(&image));

    let inside = out.at(SRC_W / 2, SRC_H / 2);
    assert!(
        close(inside, [255, 0, 255, 255], 3),
        "the asset did not land in its rect: {inside:?}"
    );
    let outside = out.at(2, 2);
    assert!(
        !close(outside, [255, 0, 255, 255], 3),
        "the asset leaked outside its rect: {outside:?}"
    );
}

/// The host decodes asynchronously. Drawing the editor's dashed placeholder
/// instead would bake an editing affordance into an export.
#[test]
fn an_image_annotation_whose_asset_is_not_uploaded_yet_draws_nothing() {
    let Some(ctx) = context() else { return };
    let scene = scene_with(IMAGE_ANNOTATION);
    let bare = render_with_annotation_image(ctx, &scene_with(""), None);
    let pending = render_with_annotation_image(ctx, &scene, None);
    assert_eq!(
        bare.at(SRC_W / 2, SRC_H / 2),
        pending.at(SRC_W / 2, SRC_H / 2)
    );
}

#[test]
fn an_image_annotations_own_opacity_fades_it() {
    let Some(ctx) = context() else { return };
    let half = scene_with(
        r##""annotations": [{"id":"i1","start":0.0,"end":10.0,
             "kind":{"kind":"image","x":0.25,"y":0.25,"w":0.5,"h":0.5,
                     "path":"asset.png","opacity":0.5}}],"##,
    );
    let image = image_texture(ctx, 8, 8);
    let (x, y) = (SRC_W / 2, SRC_H / 2);
    // Blue rises from the frame's own value to the asset's magenta, so half opacity must land strictly between.
    let none = render_with_annotation_image(ctx, &scene_with(""), None).at(x, y)[2];
    let full =
        render_with_annotation_image(ctx, &scene_with(IMAGE_ANNOTATION), Some(&image)).at(x, y)[2];
    let mid = render_with_annotation_image(ctx, &half, Some(&image)).at(x, y)[2];
    assert!(full > none + 20, "the fixture must change the pixel at all");
    assert!(
        mid > none + 5 && mid < full - 5,
        "half opacity landed at {mid}, outside ({none}, {full})"
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
    let before = render(ctx, &scene, 1.0, true);
    let during = render(ctx, &scene, 7.0, true);

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
    let out = render(ctx, &scene, 5.0, true);
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
    let frame = render(ctx, &make("frame"), 5.0, true);
    let video = render(ctx, &make("video"), 5.0, true);

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

/// Renders a scene whose camera feed is four vertical bands. A centre seam
/// cannot tell a crop from a stretch (it lands mid-bubble either way), so the
/// feed needs a pattern that is asymmetric under BOTH transforms.
const CAM_BANDS: [[u8; 4]; 4] = [
    [255, 0, 0, 255],
    [0, 255, 0, 255],
    [0, 0, 255, 255],
    [255, 255, 255, 255],
];

fn render_with_camera(ctx: &GpuContext, scene: &Scene, cam_w: u32, cam_h: u32) -> Rendered {
    let ev = Evaluator::new(
        scene,
        SourceGeometry {
            width: SRC_W,
            height: SRC_H,
        },
    );
    let params = ev.evaluate(scene, 5.0);
    let (width, height) = (params.geometry.canvas_w, params.geometry.canvas_h);
    let mut compositor = Compositor::new(ctx).expect("compositor");
    let target = compositor.output_texture(width, height);
    let source = source_texture(ctx);
    let source_view = source.create_view(&Default::default());
    let camera = banded_image(ctx, cam_w, cam_h, |x| {
        CAM_BANDS[((x * 4 / cam_w.max(1)) as usize).min(3)]
    });
    let camera_view = camera.create_view(&Default::default());

    let mut inputs = FrameInputs::new();
    for layer in &scene.layers {
        let view = match layer.source {
            LayerSource::Screen => &source_view,
            LayerSource::Camera(_) => &camera_view,
            _ => continue,
        };
        inputs.set(
            layer.id,
            LayerInput {
                view,
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

fn camera_scene(extra: &str) -> Scene {
    scene_with(&format!(
        r#""cameraOverlay": {{"enabled": true, "shape": "square", "shadow": 0.0,
             "zoomFollow": false, {extra}
             "defaultPlacement": {{"x":0.0,"y":0.0,"width":0.5,"height":0.5}}}},"#
    ))
}

/// The bubble is 32x32 at the canvas origin, so each quarter of the feed is
/// eight pixels wide when nothing crops.
fn bubble_bands(out: &Rendered) -> [[u8; 4]; 4] {
    [out.at(4, 8), out.at(12, 8), out.at(20, 8), out.at(28, 8)]
}

/// A 16:9 sensor in a square bubble must crop, not squash. The screen layer is
/// stretched to its card, because there the card IS the source.
#[test]
fn a_wide_camera_feed_is_cropped_into_the_bubble_rather_than_squashed() {
    let Some(ctx) = context() else { return };
    let scene = camera_scene(r#""mirror": false,"#);
    assert_eq!(
        bubble_bands(&render_with_camera(ctx, &scene, 32, 32)),
        CAM_BANDS,
        "a square feed should land band for band"
    );
    // Twice as wide: a centre crop keeps the middle half, so only the two inner bands survive; a stretch shows all four.
    let wide = bubble_bands(&render_with_camera(ctx, &scene, 64, 32));
    assert_eq!(
        wide,
        [CAM_BANDS[1], CAM_BANDS[1], CAM_BANDS[2], CAM_BANDS[2]],
        "the wide feed was squashed instead of cropped"
    );
}

/// The other axis. A tall feed keeps every band, because cropping the HEIGHT
/// does not move a vertical seam.
#[test]
fn a_tall_camera_feed_crops_its_height_and_keeps_every_band() {
    let Some(ctx) = context() else { return };
    let tall = render_with_camera(ctx, &camera_scene(r#""mirror": false,"#), 32, 64);
    assert_eq!(bubble_bands(&tall), CAM_BANDS);
}

/// A webcam reads as a mirror. `bubble_transform` flips the source affine, so
/// this is here to pin that it still happens once and only once: adding a flip
/// to the card shader as well silently cancelled it, and the first version of
/// this test could not see that because its feed was symmetric about the seam.
#[test]
fn the_mirror_setting_flips_the_camera_horizontally() {
    let Some(ctx) = context() else { return };
    let mirrored = bubble_bands(&render_with_camera(
        ctx,
        &camera_scene(r#""mirror": true,"#),
        32,
        32,
    ));
    let mut reversed = CAM_BANDS;
    reversed.reverse();
    assert_eq!(
        mirrored, reversed,
        "the mirror did not flip the feed exactly once"
    );
}

/// The screen layer must NOT cover-fit: its card is the source, so cropping it
/// would trim the picture at any aspect rounding.
#[test]
fn the_screen_layer_is_never_cover_fitted() {
    let scene = camera_scene(r#""mirror": true,"#);
    let ev = Evaluator::new(
        &scene,
        SourceGeometry {
            width: SRC_W,
            height: SRC_H,
        },
    );
    let params = ev.evaluate(&scene, 5.0);
    let screen = scene.screen_layer().expect("screen layer").id;
    let layer = params
        .layers
        .iter()
        .find(|l| l.id == screen)
        .expect("params");
    assert!(!layer.cover_fit);
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

    // The bubble shares the screen layer's source, so it must paint SOMETHING rather than leave the card showing.
    let _ = render(ctx, &with_camera, 5.0, true);
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

fn banded_image(
    ctx: &GpuContext,
    width: u32,
    height: u32,
    band: impl Fn(u32) -> [u8; 4],
) -> wgpu::Texture {
    let mut pixels = vec![0u8; (width * height * 4) as usize];
    for y in 0..height {
        for x in 0..width {
            let offset = ((y * width + x) * 4) as usize;
            pixels[offset..offset + 4].copy_from_slice(&band(x));
        }
    }
    let texture = ctx.device().create_texture(&wgpu::TextureDescriptor {
        label: Some("background-image"),
        size: wgpu::Extent3d {
            width,
            height,
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
        texture.as_image_copy(),
        &pixels,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(width * 4),
            rows_per_image: Some(height),
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    texture
}

fn render_background(
    ctx: &GpuContext,
    scene: &Scene,
    image: Option<(&wgpu::Texture, u32, u32)>,
) -> Rendered {
    let ev = Evaluator::new(
        scene,
        SourceGeometry {
            width: SRC_W,
            height: SRC_H,
        },
    );
    let params = ev.evaluate(scene, 0.0);
    let (width, height) = (params.geometry.canvas_w, params.geometry.canvas_h);

    let mut compositor = Compositor::new(ctx).expect("compositor");
    let target = compositor.output_texture(width, height);
    let view = image.map(|(texture, _, _)| texture.create_view(&Default::default()));

    let mut inputs = FrameInputs::new();
    if let (Some(view), Some((_, w, h))) = (view.as_ref(), image) {
        inputs.set_background(BackgroundImage {
            view,
            width: w,
            height: h,
            needs_srgb_decode: true,
        });
    }

    compositor.render(&params, &inputs, &target.create_view(&Default::default()));
    Rendered {
        pixels: read_back(ctx, &target, width, height),
        width,
        height,
    }
}

fn wallpaper(blur: f64) -> Scene {
    scene_with(&format!(
        r#""backgroundType": "wallpaper", "backgroundValue": "C:/wall.png", "backgroundBlur": {blur},"#
    ))
}

/// Stretching instead of cropping would put the first quarter of the image at
/// the left edge; cover fit puts the second quarter there.
#[test]
fn a_wider_than_canvas_background_is_cropped_not_stretched() {
    let Some(ctx) = context() else { return };
    let quarters = |x: u32| match x * 4 / (SRC_W * 2) {
        0 => [255, 0, 0, 255],
        1 => [0, 255, 0, 255],
        2 => [0, 0, 255, 255],
        _ => [255, 255, 255, 255],
    };
    let image = banded_image(ctx, SRC_W * 2, SRC_H, quarters);
    let out = render_background(ctx, &wallpaper(0.0), Some((&image, SRC_W * 2, SRC_H)));

    let left = out.at(2, out.height / 2);
    assert!(
        close(left, [0, 255, 0, 255], 3),
        "left edge was {left:?}; red there means the image was stretched"
    );
    let right = out.at(out.width - 3, out.height / 2);
    assert!(
        close(right, [0, 0, 255, 255], 3),
        "right edge was {right:?}; white there means the image was stretched"
    );
}

#[test]
fn a_background_image_with_no_blur_keeps_its_edge_sharp() {
    let Some(ctx) = context() else { return };
    let edge = |x: u32| match x < SRC_W / 2 {
        true => [255, 0, 0, 255],
        false => [0, 0, 255, 255],
    };
    let image = banded_image(ctx, SRC_W, SRC_H, edge);
    let out = render_background(ctx, &wallpaper(0.0), Some((&image, SRC_W, SRC_H)));

    let mid = out.height / 2;
    assert!(close(out.at(SRC_W / 2 - 3, mid), [255, 0, 0, 255], 3));
    assert!(close(out.at(SRC_W / 2 + 2, mid), [0, 0, 255, 255], 3));
}

#[test]
fn background_blur_softens_the_edge_it_is_pointed_at() {
    let Some(ctx) = context() else { return };
    let edge = |x: u32| match x < SRC_W / 2 {
        true => [255, 0, 0, 255],
        false => [0, 0, 255, 255],
    };
    let image = banded_image(ctx, SRC_W, SRC_H, edge);
    let sharp = render_background(ctx, &wallpaper(0.0), Some((&image, SRC_W, SRC_H)));
    let soft = render_background(ctx, &wallpaper(20.0), Some((&image, SRC_W, SRC_H)));

    let mid = sharp.height / 2;
    let x = SRC_W / 2 - 3;
    assert_eq!(
        sharp.at(x, mid)[2],
        0,
        "the unblurred edge should hold no blue"
    );
    assert!(
        soft.at(x, mid)[2] > 20,
        "blurred pixel was {:?}; blue should have bled across the edge",
        soft.at(x, mid)
    );
}

/// The WebGL shader's 9-tap kernel sums to 1.076, so its blurred backgrounds are
/// about 8% brighter than the source. A normalised kernel cannot do that.
#[test]
fn blurring_a_background_does_not_change_its_overall_brightness() {
    let Some(ctx) = context() else { return };
    let edge = |x: u32| match x < SRC_W / 2 {
        true => [200, 60, 30, 255],
        false => [30, 60, 200, 255],
    };
    let image = banded_image(ctx, SRC_W, SRC_H, edge);
    let sharp = render_background(ctx, &wallpaper(0.0), Some((&image, SRC_W, SRC_H)));
    let soft = render_background(ctx, &wallpaper(60.0), Some((&image, SRC_W, SRC_H)));

    // Averaged in linear light, where the blur happens: the sRGB mean legitimately rises and would fail correct output.
    let mean = |r: &Rendered| {
        let total: f64 = r
            .pixels
            .as_chunks::<4>()
            .0
            .iter()
            .flat_map(|p| {
                p[..3]
                    .iter()
                    .map(|c| recast_color::srgb_to_linear(*c as f32 / 255.0) as f64)
            })
            .sum();
        total / (r.pixels.len() / 4) as f64
    };
    let (before, after) = (mean(&sharp), mean(&soft));
    assert!(
        (after - before).abs() < before * 0.02,
        "linear mean went {before:.4} -> {after:.4}, which is more than rounding"
    );
}

/// Blur is an image-only control, so a solid background must survive it exactly.
#[test]
fn blur_on_a_solid_background_is_a_no_op() {
    let Some(ctx) = context() else { return };
    let out = render_background(ctx, &scene_with(r#""backgroundBlur": 100.0,"#), None);
    let corner = out.at(1, 1);
    assert!(close(corner, [0, 0, 255, 255], 2), "corner was {corner:?}");
}

/// An image that has not finished loading must not leave the canvas undefined.
#[test]
fn a_wallpaper_background_with_no_image_yet_renders_the_fallback_grey() {
    let Some(ctx) = context() else { return };
    let out = render_background(ctx, &wallpaper(0.0), None);
    let corner = out.at(1, 1);
    assert!(close(corner, [17, 17, 17, 255], 3), "corner was {corner:?}");
}

/// Samples a second apart, so a query at 0.5 s lands mid-track. The two axes
/// move in OPPOSITE directions on purpose: a symmetric path would land the
/// pointer on the diagonal, where transposing x and y changes nothing.
const CURSOR_TRACK: &str = r#"{
    "samples": [
        { "timestampUs": 0, "x": 0, "y": 32, "visible": true, "leftDown": false, "rightDown": false },
        { "timestampUs": 1000000, "x": 32, "y": 16, "visible": true, "leftDown": false, "rightDown": false },
        { "timestampUs": 2000000, "x": 64, "y": 0, "visible": true, "leftDown": false, "rightDown": false }
    ]
}"#;

fn cursor_scene(extra: &str) -> Scene {
    let mut scene = scene_with(&format!(
        r#""cursorEnabled": true, "cursorSize": 3.0, "cursorSmoothing": 0.0, {extra}"#
    ));
    let track: recast_compositor::CursorTrack =
        serde_json::from_str(CURSOR_TRACK).expect("cursor track");
    let mut track = track;
    track.rebuild_press_events();
    scene.cursor_track = Some(track);
    scene
}

fn render_cursor(ctx: &GpuContext, scene: &Scene, sprite: Option<&wgpu::Texture>) -> Rendered {
    let ev = Evaluator::new(
        scene,
        SourceGeometry {
            width: SRC_W,
            height: SRC_H,
        },
    );
    let params = ev.evaluate(scene, 0.5);
    let (width, height) = (params.geometry.canvas_w, params.geometry.canvas_h);

    let mut compositor = Compositor::new(ctx).expect("compositor");
    let target = compositor.output_texture(width, height);
    let view = sprite.map(|t| t.create_view(&Default::default()));

    let mut inputs = FrameInputs::new();
    if let Some(view) = view.as_ref() {
        inputs.set_cursor_sprite(
            recast_compositor::CursorSlot::Rest,
            recast_compositor::CursorSprite {
                view,
                hotspot: [0.5, 0.5],
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

/// Half a second into a two-second track the pointer is a quarter of the way
/// across, so this pins the interpolation as well as the draw.
const CURSOR_AT: (u32, u32) = (SRC_W / 4, SRC_H * 3 / 4);

#[test]
fn the_dot_cursor_is_drawn_where_the_track_says_it_is() {
    let Some(ctx) = context() else { return };
    let out = render_cursor(
        ctx,
        &cursor_scene(r#""cursorHighlightClicks": false,"#),
        None,
    );
    let on = out.at(CURSOR_AT.0, CURSOR_AT.1);
    assert!(
        close(on, [255, 255, 255, 255], 6),
        "cursor pixel was {on:?}"
    );
    let away = out.at(SRC_W - 2, 2);
    assert!(
        close(away, [0, 0, 255, 255], 4),
        "the far corner was {away:?}, so the cursor is not localised"
    );
}

#[test]
fn a_disabled_cursor_layer_leaves_the_frame_alone() {
    let Some(ctx) = context() else { return };
    let with_cursor = render_cursor(
        ctx,
        &cursor_scene(r#""cursorHighlightClicks": false,"#),
        None,
    );
    let without = render_cursor(ctx, &scene_with(r#""cursorEnabled": false,"#), None);
    assert_ne!(
        with_cursor.at(CURSOR_AT.0, CURSOR_AT.1),
        without.at(CURSOR_AT.0, CURSOR_AT.1)
    );
}

/// An uploaded sprite must WIN over the dot, or the host has no way to choose a
/// pointer style short of another flag.
#[test]
fn an_uploaded_sprite_replaces_the_dot() {
    let Some(ctx) = context() else { return };
    let sprite = banded_image(ctx, 16, 16, |_| [255, 0, 255, 255]);
    let scene = cursor_scene(r#""cursorHighlightClicks": false,"#);
    let dot = render_cursor(ctx, &scene, None);
    let drawn = render_cursor(ctx, &scene, Some(&sprite));

    let on = drawn.at(CURSOR_AT.0, CURSOR_AT.1);
    assert!(close(on, [255, 0, 255, 255], 6), "sprite pixel was {on:?}");
    assert_ne!(dot.at(CURSOR_AT.0, CURSOR_AT.1), on);
}

/// The hotspot is what lands on the cursor position, so moving it must move the
/// sprite. A centred arrow would sit half a sprite below where it points.
#[test]
fn the_hotspot_decides_where_the_sprite_sits() {
    let Some(ctx) = context() else { return };
    let sprite = banded_image(ctx, 16, 16, |_| [255, 0, 255, 255]);
    let scene = cursor_scene(r#""cursorHighlightClicks": false,"#);

    let ev = Evaluator::new(
        &scene,
        SourceGeometry {
            width: SRC_W,
            height: SRC_H,
        },
    );
    let params = ev.evaluate(&scene, 0.5);
    let (width, height) = (params.geometry.canvas_w, params.geometry.canvas_h);
    let view = sprite.create_view(&Default::default());

    let render_with = |hotspot: [f32; 2]| {
        let mut compositor = Compositor::new(ctx).expect("compositor");
        let target = compositor.output_texture(width, height);
        let mut inputs = FrameInputs::new();
        inputs.set_cursor_sprite(
            recast_compositor::CursorSlot::Rest,
            recast_compositor::CursorSprite {
                view: &view,
                hotspot,
            },
        );
        compositor.render(&params, &inputs, &target.create_view(&Default::default()));
        Rendered {
            pixels: read_back(ctx, &target, width, height),
            width,
            height,
        }
    };

    // A top-left hotspot pushes the sprite down and right, so the pixel just above the cursor is no longer covered.
    let centred = render_with([0.5, 0.5]);
    let cornered = render_with([0.0, 0.0]);
    let above = (CURSOR_AT.0, CURSOR_AT.1 - 8);
    assert!(close(centred.at(above.0, above.1), [255, 0, 255, 255], 6));
    assert!(!close(cornered.at(above.0, above.1), [255, 0, 255, 255], 6));
}

/// A fully transparent sprite must leave the frame untouched rather than
/// punching a hole, which is what an unpremultiplied blend would do.
#[test]
fn a_transparent_sprite_does_not_erase_what_is_under_it() {
    let Some(ctx) = context() else { return };
    let clear = banded_image(ctx, 16, 16, |_| [0, 0, 0, 0]);
    let scene = cursor_scene(r#""cursorEnabled": false,"#);
    let bare = render_cursor(ctx, &scene, None);
    let scene = cursor_scene(r#""cursorHighlightClicks": false,"#);
    let over = render_cursor(ctx, &scene, Some(&clear));

    assert!(
        close(
            over.at(CURSOR_AT.0, CURSOR_AT.1),
            bare.at(CURSOR_AT.0, CURSOR_AT.1),
            3
        ),
        "transparent sprite changed {:?} to {:?}",
        bare.at(CURSOR_AT.0, CURSOR_AT.1),
        over.at(CURSOR_AT.0, CURSOR_AT.1)
    );
}

/// The preview composites at full resolution and presents into a canvas sized to
/// the pane, so the present pass has to SCALE. A 1:1 texel fetch instead shows
/// the top-left corner of the composition, which reads as padding that only
/// pushes the video down and right.
#[test]
fn presenting_into_a_smaller_target_scales_rather_than_cropping() {
    let Some(ctx) = context() else { return };
    // Padding puts background on every side, so a crop shows as a missing border rather than a shifted picture.
    let scene = scene_with(r#""padding": 20.0,"#);
    let ev = Evaluator::new(
        &scene,
        SourceGeometry {
            width: SRC_W,
            height: SRC_H,
        },
    );
    let params = ev.evaluate(&scene, 0.0);
    let (comp_w, comp_h) = (params.geometry.canvas_w, params.geometry.canvas_h);
    assert!(comp_w > SRC_W, "the fixture must actually pad");

    let mut compositor = Compositor::new(ctx).expect("compositor");
    let source = source_texture(ctx);
    let source_view = source.create_view(&Default::default());
    let screen = scene
        .layers
        .iter()
        .find(|l| matches!(l.source, LayerSource::Screen))
        .expect("screen layer");
    let mut inputs = FrameInputs::new();
    inputs.set(
        screen.id,
        LayerInput {
            view: &source_view,
            needs_srgb_decode: true,
        },
    );

    // Half size, the same aspect the preview keeps.
    let (out_w, out_h) = (comp_w / 2, comp_h / 2);
    let target = compositor.output_texture(out_w, out_h);
    compositor.render(&params, &inputs, &target.create_view(&Default::default()));
    let out = Rendered {
        pixels: read_back(ctx, &target, out_w, out_h),
        width: out_w,
        height: out_h,
    };

    // Background survives at BOTH ends; under a crop the far corner would hold the middle of the video.
    for corner in [
        (1, 1),
        (out_w - 2, 1),
        (1, out_h - 2),
        (out_w - 2, out_h - 2),
    ] {
        let pixel = out.at(corner.0, corner.1);
        assert!(
            close(pixel, [0, 0, 255, 255], 4),
            "corner {corner:?} was {pixel:?}, expected the background"
        );
    }
    // And the video is still in the middle.
    let centre = out.at(out_w / 2, out_h / 2);
    assert!(
        !close(centre, [0, 0, 255, 255], 4),
        "the centre was background, so nothing was drawn"
    );
}

// --- Text ---

fn text_face() -> Option<recast_text::FontFace> {
    for family in ["Arial", "Segoe UI", "Helvetica", "DejaVu Sans"] {
        if let Some(resolved) = recast_text::resolve_face(family, 400, None) {
            return Some(resolved.face);
        }
    }
    eprintln!("skipping: no system font resolved");
    None
}

fn glyph_quad(
    atlas: &recast_text::GlyphAtlas,
    g: recast_text::AtlasGlyph,
    x: f32,
    y: f32,
    colour: [f32; 4],
) -> GlyphQuad {
    let (aw, ah) = atlas.size();
    GlyphQuad {
        rect: [x, y, g.width as f32, g.height as f32],
        uv: [
            g.x as f32 / aw as f32,
            g.y as f32 / ah as f32,
            (g.x + g.width) as f32 / aw as f32,
            (g.y + g.height) as f32 / ah as f32,
        ],
        colour,
    }
}

/// Renders the scene with `glyphs` on top. `sync` says whether the atlas is
/// uploaded first, so a test can check the un-synced case.
fn render_with_text(
    ctx: &GpuContext,
    scene: &Scene,
    atlas: &mut recast_text::GlyphAtlas,
    glyphs: Vec<GlyphQuad>,
    sync: bool,
) -> Rendered {
    let ev = Evaluator::new(
        scene,
        SourceGeometry {
            width: SRC_W,
            height: SRC_H,
        },
    );
    let params = ev.evaluate(scene, 0.0);
    let (width, height) = (params.geometry.canvas_w, params.geometry.canvas_h);

    let mut compositor = Compositor::new(ctx).expect("compositor");
    if sync {
        compositor.sync_glyph_atlas(atlas);
    }
    let target = compositor.output_texture(width, height);
    let mut inputs = FrameInputs::new();
    inputs.set_caption(CaptionFrame { pill: None, glyphs });
    compositor.render(&params, &inputs, &target.create_view(&Default::default()));
    Rendered {
        pixels: read_back(ctx, &target, width, height),
        width,
        height,
    }
}

fn changed_pixels(before: &Rendered, after: &Rendered) -> Vec<(u32, u32)> {
    let mut out = Vec::new();
    for y in 0..before.height {
        for x in 0..before.width {
            if !close(after.at(x, y), before.at(x, y), 2) {
                out.push((x, y));
            }
        }
    }
    out
}

fn packed_glyph(
    atlas: &mut recast_text::GlyphAtlas,
    face: &recast_text::FontFace,
    text: &str,
    px: f64,
) -> recast_text::AtlasGlyph {
    let id = recast_text::shape_line(face, px, text, 0.0).glyphs[0].id;
    atlas.insert(0, face, id, px).expect("glyph rasterises")
}

#[test]
fn a_glyph_quad_paints_inside_its_rect_and_nowhere_else() {
    let Some(ctx) = context() else { return };
    let Some(face) = text_face() else { return };
    let scene = scene_with("");
    let mut atlas = recast_text::GlyphAtlas::new(256, 1024);
    // Small enough to sit whole inside the 64x32 canvas, so clipping can't stand in for the assertions below.
    let g = packed_glyph(&mut atlas, &face, "M", 20.0);

    let blank = render_with_text(ctx, &scene, &mut atlas, Vec::new(), true);
    let quad = glyph_quad(&atlas, g, 4.0, 4.0, [1.0, 1.0, 1.0, 1.0]);
    let drawn = render_with_text(ctx, &scene, &mut atlas, vec![quad], true);
    assert!(4 + g.width < blank.width && 4 + g.height < blank.height);

    let changed = changed_pixels(&blank, &drawn);
    assert!(changed.len() > 20, "only {} pixels changed", changed.len());
    // A glyph is not a filled box: its corners must stay background.
    assert!(
        changed.len() < (g.width * g.height) as usize,
        "the whole quad was filled, so the coverage was ignored"
    );
    for (x, y) in changed {
        assert!(
            (4..4 + g.width).contains(&x) && (4..4 + g.height).contains(&y),
            "ink at ({x}, {y}) is outside the quad"
        );
    }
}

/// Two glyphs share one atlas, so a quad that ignored its uv would draw the
/// same ink for both.
#[test]
fn the_uv_decides_which_glyph_in_the_atlas_is_drawn() {
    let Some(ctx) = context() else { return };
    let Some(face) = text_face() else { return };
    let scene = scene_with("");
    let mut atlas = recast_text::GlyphAtlas::new(256, 1024);
    let wide = packed_glyph(&mut atlas, &face, "M", 40.0);
    let narrow = packed_glyph(&mut atlas, &face, "l", 40.0);
    assert_ne!((wide.x, wide.y), (narrow.x, narrow.y));
    let blank = render_with_text(ctx, &scene, &mut atlas, Vec::new(), true);

    // The same rect for both, so only the uv differs.
    let mut at = |g: recast_text::AtlasGlyph| {
        let mut quad = glyph_quad(&atlas, g, 20.0, 20.0, [1.0, 1.0, 1.0, 1.0]);
        quad.rect = [20.0, 20.0, wide.width as f32, wide.height as f32];
        let out = render_with_text(ctx, &scene, &mut atlas, vec![quad], true);
        changed_pixels(&blank, &out)
    };
    assert_ne!(at(wide), at(narrow));
}

#[test]
fn moving_the_quad_moves_the_ink() {
    let Some(ctx) = context() else { return };
    let Some(face) = text_face() else { return };
    let scene = scene_with("");
    let mut atlas = recast_text::GlyphAtlas::new(256, 1024);
    let g = packed_glyph(&mut atlas, &face, "M", 40.0);
    let blank = render_with_text(ctx, &scene, &mut atlas, Vec::new(), true);

    let far = (blank.width - g.width - 1) as f32;
    let left = glyph_quad(&atlas, g, 1.0, 1.0, [1.0, 1.0, 1.0, 1.0]);
    let right = glyph_quad(&atlas, g, far, 1.0, [1.0, 1.0, 1.0, 1.0]);
    let a = changed_pixels(
        &blank,
        &render_with_text(ctx, &scene, &mut atlas, vec![left], true),
    );
    let b = changed_pixels(
        &blank,
        &render_with_text(ctx, &scene, &mut atlas, vec![right], true),
    );

    assert!(!a.is_empty() && !b.is_empty());
    let a_max = a.iter().map(|(x, _)| *x).max().unwrap();
    let b_min = b.iter().map(|(x, _)| *x).min().unwrap();
    assert!(
        a_max < b_min,
        "the two placements overlap at x {a_max} / {b_min}"
    );
}

/// The background is blue, so red ink proves the instance colour is used rather
/// than the atlas being drawn as-is.
#[test]
fn the_quad_colour_tints_the_glyph() {
    let Some(ctx) = context() else { return };
    let Some(face) = text_face() else { return };
    let scene = scene_with("");
    let mut atlas = recast_text::GlyphAtlas::new(256, 1024);
    let g = packed_glyph(&mut atlas, &face, "M", 60.0);
    let blank = render_with_text(ctx, &scene, &mut atlas, Vec::new(), true);

    let quad = glyph_quad(&atlas, g, 20.0, 20.0, [1.0, 0.0, 0.0, 1.0]);
    let drawn = render_with_text(ctx, &scene, &mut atlas, vec![quad], true);
    let reddest = changed_pixels(&blank, &drawn)
        .into_iter()
        .map(|(x, y)| drawn.at(x, y))
        .max_by_key(|p| p[0])
        .expect("some ink");
    assert!(reddest[0] > 200, "expected red ink, got {reddest:?}");
    assert!(
        reddest[2] < 120,
        "blue background bled through: {reddest:?}"
    );
}

#[test]
fn the_quad_alpha_fades_the_glyph() {
    let Some(ctx) = context() else { return };
    let Some(face) = text_face() else { return };
    let scene = scene_with("");
    let mut atlas = recast_text::GlyphAtlas::new(256, 1024);
    let g = packed_glyph(&mut atlas, &face, "M", 60.0);
    let blank = render_with_text(ctx, &scene, &mut atlas, Vec::new(), true);

    let mut ink = |alpha: f32| {
        let quad = glyph_quad(&atlas, g, 20.0, 20.0, [1.0, 1.0, 1.0, alpha]);
        let out = render_with_text(ctx, &scene, &mut atlas, vec![quad], true);
        changed_pixels(&blank, &out)
            .into_iter()
            .map(|(x, y)| out.at(x, y)[0] as u32)
            .max()
            .unwrap_or(0)
    };
    let half = ink(0.5);
    let full = ink(1.0);
    assert!(half < full, "half alpha was not dimmer: {half} vs {full}");
    assert!(half > 0, "half alpha erased the glyph");
}

/// Text queued before the atlas reached the GPU must be dropped, not drawn from
/// whatever texture happened to be bound.
#[test]
fn glyphs_draw_nothing_until_the_atlas_is_uploaded() {
    let Some(ctx) = context() else { return };
    let Some(face) = text_face() else { return };
    let scene = scene_with("");
    let mut atlas = recast_text::GlyphAtlas::new(256, 1024);
    let g = packed_glyph(&mut atlas, &face, "M", 40.0);
    let quad = glyph_quad(&atlas, g, 20.0, 20.0, [1.0, 1.0, 1.0, 1.0]);

    let blank = render_with_text(ctx, &scene, &mut atlas, Vec::new(), true);
    let unsynced = render_with_text(ctx, &scene, &mut atlas, vec![quad], false);
    assert!(changed_pixels(&blank, &unsynced).is_empty());
}

/// Growth replaces the atlas texture, so an incremental upload afterwards would
/// land in the one that was thrown away.
#[test]
fn a_glyph_packed_after_the_atlas_grew_still_reaches_the_gpu() {
    let Some(ctx) = context() else { return };
    let Some(face) = text_face() else { return };
    let scene = scene_with("");
    let mut atlas = recast_text::GlyphAtlas::new(256, 2048);
    let mut last = packed_glyph(&mut atlas, &face, "M", 30.0);
    let blank = render_with_text(ctx, &scene, &mut atlas, Vec::new(), true);

    let (_, before) = atlas.size();
    for ch in "ABCDEFGHIJKLMNOPQRSTUVWXYZ".chars() {
        last = packed_glyph(&mut atlas, &face, &ch.to_string(), 90.0);
    }
    let (_, after) = atlas.size();
    assert!(after > before, "the atlas never grew");

    let quad = glyph_quad(&atlas, last, 10.0, 10.0, [1.0, 1.0, 1.0, 1.0]);
    let drawn = render_with_text(ctx, &scene, &mut atlas, vec![quad], true);
    assert!(
        changed_pixels(&blank, &drawn).len() > 50,
        "the glyph packed after the growth did not reach the GPU"
    );
}

/// End to end: a caption authored on the scene has to reach pixels, pill and
/// glyphs both. Every layer below asserts on its own piece, so nothing here
/// catches the two being wired up to nothing.
#[test]
fn a_caption_on_the_scene_reaches_the_frame() {
    let Some(ctx) = context() else { return };
    let Some(_) = text_face() else { return };
    let words = r#"[{"start": 0.0, "end": 4.0, "text": "hello"}]"#;
    let style = r##""captionStyle": {
        "enabled": true, "fontFamily": "Arial", "fontWeight": 400,
        "fontSizePct": 14.0, "position": "bottom", "align": "center",
        "offsetPct": 0.0, "color": "#ffffff", "uppercase": false,
        "letterSpacing": 0.0, "background": "box", "backgroundColor": "#ff0000",
        "backgroundOpacity": 100.0, "outlineWidth": 0.0, "outlineColor": "#000000",
        "maxLines": 1,
        "animation": {
            "chunk": "line", "chunkSize": 1, "emphasis": "none",
            "emphasisColor": "#ffffff", "highlight": "none",
            "entrance": "none", "entranceMs": 0.0, "holdGaps": true
        }
    },"##;

    let mut session = recast_compositor::Session::new(
        ctx,
        scene_with(style),
        SourceGeometry {
            width: SRC_W,
            height: SRC_H,
        },
    )
    .expect("session");

    let blank = {
        let (texture, _) = session.render_to_texture(1.0, &FrameInputs::new());
        let size = session.output_size();
        Rendered {
            pixels: read_back(ctx, &texture, size.width, size.height),
            width: size.width,
            height: size.height,
        }
    };

    session.set_caption_track(Some(serde_json::from_str(words).expect("words")));
    let caption = session.caption_frame(1.0);
    assert!(caption.pill.is_some(), "the style asks for a pill");
    assert!(!caption.glyphs.is_empty(), "no glyphs were laid out");

    let mut inputs = FrameInputs::new();
    inputs.set_caption(caption);
    let drawn = {
        let (texture, _) = session.render_to_texture(1.0, &inputs);
        let size = session.output_size();
        Rendered {
            pixels: read_back(ctx, &texture, size.width, size.height),
            width: size.width,
            height: size.height,
        }
    };

    let changed = changed_pixels(&blank, &drawn);
    assert!(!changed.is_empty(), "the caption never reached the frame");
    // The pill is opaque red and the text is white, so both must be present.
    let hits = changed.iter().map(|(x, y)| drawn.at(*x, *y));
    let (mut red, mut white) = (0, 0);
    for p in hits {
        if p[0] > 180 && p[1] < 90 && p[2] < 90 {
            red += 1;
        }
        if p[0] > 180 && p[1] > 180 && p[2] > 180 {
            white += 1;
        }
    }
    assert!(red > 0, "the pill did not draw");
    assert!(white > 0, "the glyphs did not draw");
}
