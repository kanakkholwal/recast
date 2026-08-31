//! The decode path from a decoder's Y'CbCr planes to the linear working space.
//! The shader duplicates `recast-color`'s curves in WGSL; `every_transfer_function_matches_recast_color` is what makes that drift-safe.

use recast_color::{apply, ColorRange, MatrixCoefficients, Primaries, TransferFunction};
use recast_compositor::{
    decode_matrix, gamut_matrix, ChromaSiting, Compositor, Evaluator, FrameInputs, LayerInput,
    Plane, PlaneData, PlaneLayout, SourceColor, SourceGeometry, SourcePlanes, YuvError,
};
use recast_gpu::{GpuContext, GpuOptions, WORKING_FORMAT};
use recast_scene::migrate::to_scene;
use recast_scene::v1::RenderState;
use recast_scene::{LayerSource, Scene};

const W: u32 = 8;
const H: u32 = 8;

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

/// Packs one frame from a per-pixel code triple, taking each chroma sample from
/// the top-left pixel of the block it covers.
fn pack(layout: PlaneLayout, w: u32, h: u32, at: impl Fn(u32, u32) -> [u8; 3]) -> Vec<u8> {
    let mut out = vec![0u8; layout.packed_len(w, h)];
    for y in 0..h {
        for x in 0..w {
            out[(y * w + x) as usize] = at(x, y)[0];
        }
    }
    let (cw, ch, sample) = layout.plane_size(1, w, h);
    let (sx, sy) = (w / cw, h / ch);
    let base = (w * h) as usize;
    let plane = (cw * ch) as usize;
    for cy in 0..ch {
        for cx in 0..cw {
            let c = at(cx * sx, cy * sy);
            let index = (cy * cw + cx) as usize;
            if sample == 2 {
                out[base + index * 2] = c[1];
                out[base + index * 2 + 1] = c[2];
            } else {
                out[base + index] = c[1];
                out[base + plane + index] = c[2];
            }
        }
    }
    out
}

fn frame<'a>(
    data: &'a [u8],
    layout: PlaneLayout,
    color: SourceColor,
    w: u32,
    h: u32,
) -> SourcePlanes<'a> {
    SourcePlanes {
        width: w,
        height: h,
        layout,
        color,
        data: PlaneData::Packed(data),
    }
}

/// What the shader is held to. The same steps in the same order, on the CPU.
fn cpu_decode(color: &SourceColor, codes: [u8; 3]) -> [f32; 3] {
    let (matrix, bias) = decode_matrix(color);
    let normalised = codes.map(|c| c as f32 / 255.0);
    let rgb = apply(matrix, normalised);
    let encoded = [
        (rgb[0] + bias[0]).clamp(0.0, 1.0),
        (rgb[1] + bias[1]).clamp(0.0, 1.0),
        (rgb[2] + bias[2]).clamp(0.0, 1.0),
    ];
    let light = encoded.map(|c| color.transfer.to_linear(c));
    apply(gamut_matrix(color), light)
}

fn half_to_f32(bits: u16) -> f32 {
    let sign = if bits & 0x8000 != 0 { -1.0 } else { 1.0 };
    let exponent = ((bits >> 10) & 0x1f) as i32;
    let mantissa = (bits & 0x3ff) as f32;
    match exponent {
        0 => sign * mantissa * 2f32.powi(-24),
        31 => sign * f32::INFINITY,
        _ => sign * (1.0 + mantissa / 1024.0) * 2f32.powi(exponent - 15),
    }
}

struct Decoded {
    pixels: Vec<f32>,
    width: u32,
}

impl Decoded {
    fn at(&self, x: u32, y: u32) -> [f32; 3] {
        let o = ((y * self.width + x) * 4) as usize;
        [self.pixels[o], self.pixels[o + 1], self.pixels[o + 2]]
    }
}

fn decode(ctx: &GpuContext, source: &SourcePlanes<'_>) -> Decoded {
    let mut compositor = Compositor::new(ctx).expect("compositor");
    decode_with(ctx, &mut compositor, source)
}

fn decode_with(
    ctx: &GpuContext,
    compositor: &mut Compositor,
    source: &SourcePlanes<'_>,
) -> Decoded {
    let target = readable_target(ctx, source.width, source.height);
    compositor.decode_source(source, &target).expect("decode");
    read_back(ctx, &target, source.width, source.height)
}

/// `Compositor::source_texture` deliberately does not ask for `COPY_SRC`, since
/// nothing in the graph reads a source back. Only the tests do.
fn readable_target(ctx: &GpuContext, width: u32, height: u32) -> wgpu::Texture {
    ctx.device().create_texture(&wgpu::TextureDescriptor {
        label: Some("readable-source"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: WORKING_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    })
}

fn read_back(ctx: &GpuContext, texture: &wgpu::Texture, width: u32, height: u32) -> Decoded {
    let bytes_per_row = recast_gpu::aligned_bytes_per_row(width, WORKING_FORMAT);
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
    let mut pixels = Vec::with_capacity((width * height * 4) as usize);
    for row in 0..height {
        let start = (row * bytes_per_row) as usize;
        for channel in 0..(width * 4) as usize {
            let at = start + channel * 2;
            pixels.push(half_to_f32(u16::from_le_bytes([
                mapped[at],
                mapped[at + 1],
            ])));
        }
    }
    drop(mapped);
    buffer.unmap();
    Decoded { pixels, width }
}

fn close(got: [f32; 3], want: [f32; 3], tolerance: f32) -> bool {
    got.iter()
        .zip(want)
        .all(|(g, w)| (g - w).abs() <= tolerance)
}

/// Half floats carry about eleven bits of mantissa, and the plane textures
/// quantise to eight, so anything tighter than this is measuring the format.
const TOLERANCE: f32 = 2e-3;

/// Looser, because PQ raises its input to the power of 6.3 and a GPU `pow` is
/// not obliged to be as accurate as a CPU one. Still an order of magnitude
/// below the gap between any two of these curves, which is what it has to
/// separate.
const CURVE_TOLERANCE: f32 = 5e-3;

const BLACK: [u8; 3] = [16, 128, 128];
const WHITE: [u8; 3] = [235, 128, 128];
const RED: [u8; 3] = [81, 90, 240];
const GREEN: [u8; 3] = [145, 54, 34];
const BLUE: [u8; 3] = [41, 240, 110];
const MID: [u8; 3] = [126, 128, 128];
const BRIGHT: [u8; 3] = [200, 128, 128];

/// Four flat quadrants, so every sampled point sits well inside a block of one
/// colour and no chroma interpolation is in play.
fn quadrants(codes: [[u8; 3]; 4]) -> impl Fn(u32, u32) -> [u8; 3] {
    move |x, y| codes[((y >= H / 2) as usize) * 2 + (x >= W / 2) as usize]
}

const CENTRES: [(u32, u32); 4] = [(2, 2), (6, 2), (2, 6), (6, 6)];

#[test]
fn the_shader_decodes_the_same_light_the_cpu_matrix_does() {
    let Some(ctx) = context() else { return };
    let color = SourceColor::default();
    let codes = [RED, GREEN, BLUE, MID];
    let data = pack(PlaneLayout::Nv12, W, H, quadrants(codes));
    let out = decode(ctx, &frame(&data, PlaneLayout::Nv12, color, W, H));

    for (index, (x, y)) in CENTRES.into_iter().enumerate() {
        let want = cpu_decode(&color, codes[index]);
        let got = out.at(x, y);
        assert!(
            close(got, want, TOLERANCE),
            "{:?}: {got:?} vs {want:?}",
            codes[index]
        );
    }
}

/// The one test that keeps the WGSL curves and the Rust curves the same curve.
#[test]
fn every_transfer_function_matches_recast_color() {
    let Some(ctx) = context() else { return };
    for transfer in [
        TransferFunction::Srgb,
        TransferFunction::Linear,
        TransferFunction::Gamma22,
        TransferFunction::Rec709,
        TransferFunction::Pq,
        TransferFunction::Hlg,
    ] {
        let color = SourceColor {
            transfer,
            range: ColorRange::Full,
            ..Default::default()
        };
        // I444 so no chroma sample is shared and every pixel stands alone.
        let ramp = |x: u32, y: u32| [(((y * W + x) * 255) / (W * H - 1)) as u8, 128, 128];
        let data = pack(PlaneLayout::I444, W, H, ramp);
        let out = decode(ctx, &frame(&data, PlaneLayout::I444, color, W, H));

        for y in 0..H {
            for x in 0..W {
                let want = cpu_decode(&color, ramp(x, y));
                let got = out.at(x, y);
                assert!(
                    close(got, want, CURVE_TOLERANCE),
                    "{transfer:?} at {x},{y}: {got:?} vs {want:?}"
                );
            }
        }
    }
}

/// Every one of these curves is zero at zero, and each has a linear segment
/// near black to make it so. A curve missing its toe lifts black by only a few
/// thousandths, which the ramp above cannot separate from GPU noise and an eye
/// can separate immediately.
#[test]
fn every_transfer_function_leaves_black_at_black() {
    let Some(ctx) = context() else { return };
    for transfer in [
        TransferFunction::Srgb,
        TransferFunction::Linear,
        TransferFunction::Gamma22,
        TransferFunction::Rec709,
        TransferFunction::Pq,
        TransferFunction::Hlg,
    ] {
        let color = SourceColor {
            transfer,
            range: ColorRange::Full,
            ..Default::default()
        };
        let data = pack(PlaneLayout::I444, W, H, |_, _| [0, 128, 128]);
        let out = decode(ctx, &frame(&data, PlaneLayout::I444, color, W, H));
        let got = out.at(4, 4);
        assert!(
            close(got, [0.0; 3], 1e-3),
            "{transfer:?} lifted black to {got:?}"
        );
    }
}

#[test]
fn limited_range_black_and_white_reach_the_endpoints() {
    let Some(ctx) = context() else { return };
    let color = SourceColor::default();
    let data = pack(
        PlaneLayout::Nv12,
        W,
        H,
        quadrants([BLACK, WHITE, BLACK, WHITE]),
    );
    let out = decode(ctx, &frame(&data, PlaneLayout::Nv12, color, W, H));

    assert!(
        close(out.at(2, 2), [0.0; 3], TOLERANCE),
        "{:?}",
        out.at(2, 2)
    );
    assert!(
        close(out.at(6, 2), [1.0; 3], TOLERANCE),
        "{:?}",
        out.at(6, 2)
    );
}

/// Footroom below 16 exists so an overshoot survives the encoder. It is not
/// blacker than black once it lands in a display space.
#[test]
fn codes_below_the_footroom_clamp_rather_than_going_negative() {
    let Some(ctx) = context() else { return };
    let color = SourceColor::default();
    let data = pack(PlaneLayout::Nv12, W, H, quadrants([[0, 128, 128]; 4]));
    let out = decode(ctx, &frame(&data, PlaneLayout::Nv12, color, W, H));
    let got = out.at(4, 4);
    assert!(close(got, [0.0; 3], 1e-4), "{got:?}");
}

#[test]
fn nv12_and_i420_agree_on_the_same_samples() {
    let Some(ctx) = context() else { return };
    let color = SourceColor::default();
    let codes = quadrants([RED, GREEN, BLUE, MID]);
    let nv12 = pack(PlaneLayout::Nv12, W, H, &codes);
    let i420 = pack(PlaneLayout::I420, W, H, &codes);
    let a = decode(ctx, &frame(&nv12, PlaneLayout::Nv12, color, W, H));
    let b = decode(ctx, &frame(&i420, PlaneLayout::I420, color, W, H));

    for (x, y) in CENTRES {
        assert!(
            close(a.at(x, y), b.at(x, y), 1e-4),
            "at {x},{y}: {:?} vs {:?}",
            a.at(x, y),
            b.at(x, y)
        );
    }
}

/// If the matrix never reached the shader, every one of these would agree.
#[test]
fn the_matrix_coefficients_change_what_the_codes_mean() {
    let Some(ctx) = context() else { return };
    let data = pack(PlaneLayout::Nv12, W, H, quadrants([RED; 4]));
    let mut seen: Vec<[f32; 3]> = Vec::new();
    for matrix in [
        MatrixCoefficients::Bt709,
        MatrixCoefficients::Bt601,
        MatrixCoefficients::Bt2020Ncl,
    ] {
        let color = SourceColor {
            matrix,
            ..Default::default()
        };
        let out = decode(ctx, &frame(&data, PlaneLayout::Nv12, color, W, H));
        let got = out.at(4, 4);
        assert!(
            seen.iter().all(|prior| !close(*prior, got, 5e-3)),
            "{matrix:?} decoded to {got:?}, same as an earlier matrix"
        );
        seen.push(got);
    }
}

#[test]
fn full_range_and_limited_range_read_the_same_codes_differently() {
    let Some(ctx) = context() else { return };
    let data = pack(PlaneLayout::Nv12, W, H, quadrants([BRIGHT; 4]));
    let limited = decode(
        ctx,
        &frame(&data, PlaneLayout::Nv12, SourceColor::default(), W, H),
    );
    let full = decode(
        ctx,
        &frame(
            &data,
            PlaneLayout::Nv12,
            SourceColor {
                range: ColorRange::Full,
                ..Default::default()
            },
            W,
            H,
        ),
    );
    let (a, b) = (limited.at(4, 4), full.at(4, 4));
    assert!(a[1] > b[1] + 0.05, "limited should stretch: {a:?} vs {b:?}");
}

/// The plane textures are cached on the frame's shape. A cache keyed on size alone survives every other test here, because each one builds its own compositor.
#[test]
fn one_compositor_decodes_frames_of_changing_shape() {
    let Some(ctx) = context() else { return };
    let color = SourceColor::default();
    let mut compositor = Compositor::new(ctx).expect("compositor");

    let big = pack(PlaneLayout::Nv12, W, H, quadrants([RED, GREEN, BLUE, MID]));
    let first = decode_with(
        ctx,
        &mut compositor,
        &frame(&big, PlaneLayout::Nv12, color, W, H),
    );

    // Same size, different layout: the planes reallocate for the channel count even though the shape didn't move.
    let planar = pack(PlaneLayout::I420, W, H, quadrants([RED, GREEN, BLUE, MID]));
    let swapped = decode_with(
        ctx,
        &mut compositor,
        &frame(&planar, PlaneLayout::I420, color, W, H),
    );
    for (index, (x, y)) in CENTRES.into_iter().enumerate() {
        let want = cpu_decode(&color, [RED, GREEN, BLUE, MID][index]);
        assert!(
            close(swapped.at(x, y), want, TOLERANCE),
            "{:?}",
            swapped.at(x, y)
        );
    }

    let small = pack(PlaneLayout::I420, 4, 4, |_, _| GREEN);
    let middle = decode_with(
        ctx,
        &mut compositor,
        &frame(&small, PlaneLayout::I420, color, 4, 4),
    );
    assert!(
        close(middle.at(1, 1), cpu_decode(&color, GREEN), TOLERANCE),
        "{:?}",
        middle.at(1, 1)
    );

    // Back to the first shape, which must reallocate again rather than read the smaller planes it just wrote.
    let again = decode_with(
        ctx,
        &mut compositor,
        &frame(&big, PlaneLayout::Nv12, color, W, H),
    );
    for (x, y) in CENTRES {
        assert!(
            close(first.at(x, y), again.at(x, y), 1e-4),
            "at {x},{y}: {:?} vs {:?}",
            first.at(x, y),
            again.at(x, y)
        );
    }
}

/// A quarter-texel shift is invisible on a flat block and obvious on an edge,
/// which is why the fixture puts the edge on a chroma boundary.
#[test]
fn chroma_siting_moves_the_colour_edge() {
    let Some(ctx) = context() else { return };
    let edge = |x: u32, _y: u32| if x < W / 2 { RED } else { BLUE };
    let data = pack(PlaneLayout::Nv12, W, H, edge);
    let left = decode(
        ctx,
        &frame(&data, PlaneLayout::Nv12, SourceColor::default(), W, H),
    );
    let centred = decode(
        ctx,
        &frame(
            &data,
            PlaneLayout::Nv12,
            SourceColor {
                siting: ChromaSiting::Center,
                ..Default::default()
            },
            W,
            H,
        ),
    );

    let column = (0..H)
        .map(|y| (left.at(W / 2, y), centred.at(W / 2, y)))
        .find(|(a, b)| !close(*a, *b, 5e-3));
    assert!(
        column.is_some(),
        "siting made no difference across the chroma edge"
    );
}

/// Not merely "the shift is nonzero": a quarter texel is the shift that puts
/// the sample ON the chroma sample rather than a fraction past it, so column 4
/// has to come back as pure blue and column 3 as the exact midpoint.
#[test]
fn co_siting_lands_on_a_chroma_sample_rather_than_between_two() {
    let Some(ctx) = context() else { return };
    let color = SourceColor::default();
    let edge = |x: u32, _y: u32| if x < W / 2 { RED } else { BLUE };
    let data = pack(PlaneLayout::Nv12, W, H, edge);
    let out = decode(ctx, &frame(&data, PlaneLayout::Nv12, color, W, H));

    let on_sample = cpu_decode(&color, BLUE);
    assert!(
        close(out.at(4, 3), on_sample, TOLERANCE),
        "column 4 was {:?}, expected the chroma sample itself {on_sample:?}",
        out.at(4, 3)
    );

    let halfway = cpu_decode(
        &color,
        [
            RED[0],
            (RED[1] as u16 + BLUE[1] as u16).div_ceil(2) as u8,
            (RED[2] as u16 + BLUE[2] as u16).div_ceil(2) as u8,
        ],
    );
    assert!(
        close(out.at(3, 3), halfway, 4e-3),
        "column 3 was {:?}, expected the midpoint {halfway:?}",
        out.at(3, 3)
    );
}

#[test]
fn a_wide_gamut_source_is_brought_into_the_working_gamut() {
    let Some(ctx) = context() else { return };
    let data = pack(PlaneLayout::Nv12, W, H, quadrants([GREEN; 4]));
    let bt709 = decode(
        ctx,
        &frame(&data, PlaneLayout::Nv12, SourceColor::default(), W, H),
    );
    let bt2020 = decode(
        ctx,
        &frame(
            &data,
            PlaneLayout::Nv12,
            SourceColor {
                primaries: Primaries::Bt2020,
                ..Default::default()
            },
            W,
            H,
        ),
    );
    let (a, b) = (bt709.at(4, 4), bt2020.at(4, 4));
    assert!(!close(a, b, 1e-2), "the gamut matrix did nothing: {a:?}");
    assert!(
        close(
            b,
            cpu_decode(
                &SourceColor {
                    primaries: Primaries::Bt2020,
                    ..Default::default()
                },
                GREEN
            ),
            TOLERANCE
        ),
        "{b:?}"
    );
}

#[test]
fn a_mismatched_target_is_refused_rather_than_rendered_wrong() {
    let Some(ctx) = context() else { return };
    let mut compositor = Compositor::new(ctx).expect("compositor");
    let data = pack(PlaneLayout::Nv12, W, H, quadrants([MID; 4]));
    let source = frame(&data, PlaneLayout::Nv12, SourceColor::default(), W, H);

    let wrong_size = compositor.source_texture(W * 2, H);
    assert!(matches!(
        compositor.decode_source(&source, &wrong_size),
        Err(YuvError::TargetSize { .. })
    ));

    let wrong_format = ctx.device().create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width: W,
            height: H,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    assert_eq!(
        compositor.decode_source(&source, &wrong_format),
        Err(YuvError::TargetFormat)
    );
}

#[test]
fn a_padded_plane_decodes_the_same_as_a_tight_one() {
    let Some(ctx) = context() else { return };
    let color = SourceColor::default();
    let codes = quadrants([RED, GREEN, BLUE, MID]);
    let tight = pack(PlaneLayout::Nv12, W, H, &codes);
    let stride = W + 6;
    let mut luma = vec![0u8; (stride * H) as usize];
    let mut chroma = vec![0u8; (stride * H / 2) as usize];
    for y in 0..H {
        for x in 0..W {
            luma[(y * stride + x) as usize] = tight[(y * W + x) as usize];
        }
    }
    for y in 0..H / 2 {
        for x in 0..W {
            chroma[(y * stride + x) as usize] = tight[(W * H + y * W + x) as usize];
        }
    }
    let planes = [
        Plane {
            bytes: &luma,
            stride,
        },
        Plane {
            bytes: &chroma,
            stride,
        },
    ];
    let padded = SourcePlanes {
        width: W,
        height: H,
        layout: PlaneLayout::Nv12,
        color,
        data: PlaneData::Planar(&planes),
    };

    let a = decode(ctx, &frame(&tight, PlaneLayout::Nv12, color, W, H));
    let b = decode(ctx, &padded);
    for (x, y) in CENTRES {
        assert!(
            close(a.at(x, y), b.at(x, y), 1e-4),
            "at {x},{y}: {:?} vs {:?}",
            a.at(x, y),
            b.at(x, y)
        );
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

/// The handoff, and the bug it guards: the decoded texture is already linear,
/// so a layer pass that decodes sRGB again lands somewhere much darker.
#[test]
fn the_decoded_texture_composites_without_a_second_decode() {
    let Some(ctx) = context() else { return };
    let state: RenderState = serde_json::from_str(BASE).expect("state");
    let scene: Scene = to_scene(&state);
    let ev = Evaluator::new(
        &scene,
        SourceGeometry {
            width: W,
            height: H,
        },
    );
    let params = ev.evaluate(&scene, 0.0);
    let (cw, ch) = (params.geometry.canvas_w, params.geometry.canvas_h);

    let color = SourceColor::default();
    let data = pack(PlaneLayout::Nv12, W, H, quadrants([MID; 4]));
    let mut compositor = Compositor::new(ctx).expect("compositor");
    let source = compositor.source_texture(W, H);
    compositor
        .decode_source(&frame(&data, PlaneLayout::Nv12, color, W, H), &source)
        .expect("decode");
    let source_view = source.create_view(&Default::default());

    let target = compositor.output_texture(cw, ch);
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
            needs_srgb_decode: false,
        },
    );
    compositor.render(&params, &inputs, &target.create_view(&Default::default()));

    let bytes_per_row = recast_gpu::aligned_bytes_per_row(cw, wgpu::TextureFormat::Rgba8Unorm);
    let buffer = ctx.device().create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: (bytes_per_row * ch) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = ctx.device().create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &target,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(bytes_per_row),
                rows_per_image: Some(ch),
            },
        },
        wgpu::Extent3d {
            width: cw,
            height: ch,
            depth_or_array_layers: 1,
        },
    );
    ctx.queue().submit([encoder.finish()]);
    let slice = buffer.slice(..);
    slice.map_async(wgpu::MapMode::Read, |_| {});
    ctx.device()
        .poll(wgpu::PollType::wait_indefinitely())
        .expect("poll");
    let mapped = slice.get_mapped_range().expect("map");
    let middle = ((ch / 2) * bytes_per_row + (cw / 2) * 4) as usize;
    let got = [mapped[middle], mapped[middle + 1], mapped[middle + 2]];
    drop(mapped);
    buffer.unmap();

    let light = cpu_decode(&color, MID)[1];
    let want = (recast_color::linear_to_srgb(light) * 255.0).round() as u8;
    assert!(
        got.iter().all(|c| c.abs_diff(want) <= 2),
        "composited {got:?}, expected about {want}"
    );
}
