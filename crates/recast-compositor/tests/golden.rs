//! Committed frames for the compositor.
//!
//! The property tests next door assert that a seam moved or a corner darkened.
//! They cannot see a half-pixel shift across the whole frame, a transfer
//! function applied twice, or a feature that quietly stopped drawing while its
//! own assertion still passed. That is what a golden is for.
//!
//! These are also the reference the WASM arm is held to: `packages/engine/test/golden`
//! renders the same fixtures through the browser build and compares them to the
//! same PNGs. Both arms read `goldens/fixtures.json` and the committed
//! `source.png` / `background.png`, so neither can drift into testing a
//! different scene than the other.
//!
//! `UPDATE_GOLDENS=1 cargo test -p recast-compositor --test golden` rewrites
//! them. Read the diff before committing it.

use std::path::{Path, PathBuf};

use recast_compositor::{
    BackgroundImage, Compositor, Evaluator, FrameInputs, LayerInput, SourceGeometry,
};
use recast_gpu::{GpuContext, GpuOptions};
use recast_scene::migrate::to_scene;
use recast_scene::v1::RenderState;
use recast_scene::{LayerSource, Scene};

const SRC_W: u32 = 128;
const SRC_H: u32 = 72;

/// Same tolerance for every fixture. Rasterisation differs by a least
/// significant bit between drivers, so exact equality would fail on a machine
/// that is not wrong. Anything a person would notice is far above this.
const MAX_CHANNEL: u8 = 4;
const MAX_MEAN: f64 = 0.35;

/// The fixture list, shared with the wasm arm. Read rather than compiled in, so
/// there is one definition of what a golden fixture IS.
#[derive(serde::Deserialize)]
struct Fixtures {
    base: serde_json::Map<String, serde_json::Value>,
    fixtures: Vec<Fixture>,
}

#[derive(serde::Deserialize)]
struct Fixture {
    name: String,
    time: f64,
    overrides: serde_json::Map<String, serde_json::Value>,
    /// The same scene with this fixture's feature turned OFF. Present only where
    /// there is a feature to turn off, and kept beside the fixture rather than in
    /// a second list that would repeat it.
    #[serde(default)]
    without: Option<serde_json::Map<String, serde_json::Value>>,
}

fn load_fixtures() -> Fixtures {
    let path = goldens_dir().join("fixtures.json");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    serde_json::from_str(&text).expect("fixtures.json")
}

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

fn scene_for(all: &Fixtures, fixture: &Fixture) -> Scene {
    let mut merged = all.base.clone();
    for (key, value) in &fixture.overrides {
        merged.insert(key.clone(), value.clone());
    }
    let state: RenderState =
        serde_json::from_value(serde_json::Value::Object(merged)).expect("render state");
    to_scene(&state)
}

/// A source with a diagonal, a colour split and a mid grey. The diagonal is what
/// makes a half-pixel geometry shift visible; the grey is the only value that
/// catches a transfer function applied twice, since the primaries are fixed
/// points of the curve.
fn source_pixels() -> Vec<u8> {
    let mut pixels = vec![0u8; (SRC_W * SRC_H * 4) as usize];
    for y in 0..SRC_H {
        for x in 0..SRC_W {
            let offset = ((y * SRC_W + x) * 4) as usize;
            let on_diagonal = (x * SRC_H).abs_diff(y * SRC_W) < SRC_W * 2;
            let colour = if on_diagonal {
                [255, 255, 0, 255]
            } else if y >= SRC_H * 2 / 3 {
                [128, 128, 128, 255]
            } else if x < SRC_W / 2 {
                [220, 40, 40, 255]
            } else {
                [40, 200, 90, 255]
            };
            pixels[offset..offset + 4].copy_from_slice(&colour);
        }
    }
    pixels
}

/// Hard-edged checks. A blur is only visible against an edge, and a background
/// fixture with nothing to blur is the shape of a test that proves nothing.
fn background_pixels() -> Vec<u8> {
    let mut pixels = vec![0u8; (SRC_W * SRC_H * 4) as usize];
    for y in 0..SRC_H {
        for x in 0..SRC_W {
            let offset = ((y * SRC_W + x) * 4) as usize;
            let dark = ((x / 8) + (y / 8)).is_multiple_of(2);
            let colour = if dark {
                [20, 30, 60, 255]
            } else {
                [230, 210, 120, 255]
            };
            pixels[offset..offset + 4].copy_from_slice(&colour);
        }
    }
    pixels
}

fn texture_from(ctx: &GpuContext, label: &str, pixels: &[u8]) -> wgpu::Texture {
    let size = wgpu::Extent3d {
        width: SRC_W,
        height: SRC_H,
        depth_or_array_layers: 1,
    };
    let texture = ctx.device().create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size,
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
        size,
    );
    texture
}

/// The input textures come from the committed PNGs, not from the generators
/// above, so the browser arm loads the identical bytes instead of porting the
/// pattern. `UPDATE_GOLDENS=1` rewrites them from the generators.
fn input_pixels(name: &str, generate: fn() -> Vec<u8>) -> Vec<u8> {
    let path = goldens_dir().join(format!("{name}.png"));
    match read_png(&path) {
        Some(frame) if (frame.width, frame.height) == (SRC_W, SRC_H) => frame.pixels,
        _ => generate(),
    }
}

fn source_texture(ctx: &GpuContext) -> wgpu::Texture {
    texture_from(ctx, "golden-source", &input_pixels("source", source_pixels))
}

fn background_texture(ctx: &GpuContext) -> wgpu::Texture {
    texture_from(
        ctx,
        "golden-background",
        &input_pixels("background", background_pixels),
    )
}

fn read_back(ctx: &GpuContext, texture: &wgpu::Texture, width: u32, height: u32) -> Vec<u8> {
    let bytes_per_row = recast_gpu::aligned_bytes_per_row(width, wgpu::TextureFormat::Rgba8Unorm);
    let buffer = ctx.device().create_buffer(&wgpu::BufferDescriptor {
        label: Some("golden-readback"),
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

struct Frame {
    pixels: Vec<u8>,
    width: u32,
    height: u32,
}

fn render(ctx: &GpuContext, scene: &Scene, output_time: f64) -> Frame {
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

    let background = background_texture(ctx);
    let background_view = background.create_view(&Default::default());

    let mut inputs = FrameInputs::new();
    inputs.set_background(BackgroundImage {
        view: &background_view,
        width: SRC_W,
        height: SRC_H,
        needs_srgb_decode: true,
    });
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

    compositor.render(&params, &inputs, &target.create_view(&Default::default()));
    Frame {
        pixels: read_back(ctx, &target, width, height),
        width,
        height,
    }
}

fn goldens_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("goldens")
}

fn write_png(path: &Path, frame: &Frame) {
    let file = std::fs::File::create(path).expect("create golden");
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), frame.width, frame.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .expect("png header")
        .write_image_data(&frame.pixels)
        .expect("png data");
}

fn read_png(path: &Path) -> Option<Frame> {
    let file = std::fs::File::open(path).ok()?;
    let decoder = png::Decoder::new(std::io::BufReader::new(file));
    let mut reader = decoder.read_info().ok()?;
    let mut pixels = vec![0u8; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut pixels).ok()?;
    pixels.truncate(info.buffer_size());
    Some(Frame {
        pixels,
        width: info.width,
        height: info.height,
    })
}

fn updating() -> bool {
    std::env::var("UPDATE_GOLDENS").as_deref() == Ok("1")
}

/// Which adapter a golden set was rendered on.
///
/// Rasterisation, filtering and fp rounding all differ between drivers by more
/// than a committed PNG can absorb, so a set made on one card is not a gate on
/// another. Recording it lets a mismatch REPORT the delta instead of failing a
/// machine that is not wrong, and keeps the gate real where it matches.
fn adapter_id(ctx: &GpuContext) -> String {
    let info = ctx.info();
    format!("{} / {:?}", info.name, info.backend)
}

fn adapter_file() -> PathBuf {
    goldens_dir().join("ADAPTER")
}

fn recorded_adapter() -> Option<String> {
    std::fs::read_to_string(adapter_file())
        .ok()
        .map(|s| s.trim().to_string())
}

#[test]
fn every_fixture_matches_its_golden() {
    let Some(ctx) = context() else { return };
    let dir = goldens_dir();
    std::fs::create_dir_all(&dir).expect("goldens dir");

    let here = adapter_id(ctx);
    let recorded = recorded_adapter();
    let gates = updating() || recorded.as_deref() == Some(here.as_str());

    let all = load_fixtures();
    let mut missing = Vec::new();
    let mut drifted = Vec::new();
    for fixture in &all.fixtures {
        let name = &fixture.name;
        let frame = render(ctx, &scene_for(&all, fixture), fixture.time);
        let path = dir.join(format!("{name}.png"));

        if updating() {
            write_png(&path, &frame);
            continue;
        }
        let Some(golden) = read_png(&path) else {
            missing.push(name.clone());
            continue;
        };
        if (golden.width, golden.height) != (frame.width, frame.height) {
            drifted.push(format!(
                "{name}: golden is {}x{}, render is {}x{}",
                golden.width, golden.height, frame.width, frame.height
            ));
            continue;
        }
        let delta = recast_testkit::compare::frame_delta(&golden.pixels, &frame.pixels)
            .expect("same-sized frames");
        if !delta.is_within(MAX_CHANNEL, MAX_MEAN) {
            drifted.push(format!(
                "{name}: max {} mean {:.3} over {} differing px",
                delta.max_channel, delta.mean_channel, delta.differing_pixels
            ));
        }
    }

    if updating() {
        // The input textures are committed too: the wasm arm loads these bytes
        // rather than porting the generators, so a change to the pattern reaches
        // both arms at once.
        for (name, generate) in [
            ("source", source_pixels as fn() -> Vec<u8>),
            ("background", background_pixels as fn() -> Vec<u8>),
        ] {
            write_png(
                &dir.join(format!("{name}.png")),
                &Frame {
                    pixels: generate(),
                    width: SRC_W,
                    height: SRC_H,
                },
            );
        }
        std::fs::write(adapter_file(), &here).expect("record the adapter");
        panic!("goldens rewritten on {here}; re-run without UPDATE_GOLDENS");
    }

    assert!(
        missing.is_empty(),
        "no golden committed for {missing:?}; run with UPDATE_GOLDENS=1 and read the diff"
    );
    if !gates {
        // Loud, and never silent: a suite that opts out quietly reads exactly
        // like one that passed.
        eprintln!(
            "goldens were rendered on {}, this is {here}: reporting only.\n  {}",
            recorded.as_deref().unwrap_or("an unrecorded adapter"),
            if drifted.is_empty() {
                "every fixture still matched".to_string()
            } else {
                drifted.join("\n  ")
            }
        );
        return;
    }
    assert!(
        drifted.is_empty(),
        "the compositor moved:\n  {}",
        drifted.join("\n  ")
    );
}

/// A fixture that renders the same pixels as another is testing nothing, and it
/// is not obvious from reading it. This is what caught border radius, drop
/// shadow and gradients being invisible to the FFmpeg-side harness: they were
/// pre-rasterised elsewhere, so the fixtures that set them were identical to the
/// ones that did not.
#[test]
fn every_fixture_renders_a_distinct_frame() {
    let Some(ctx) = context() else { return };
    let all = load_fixtures();
    let mut seen: Vec<(String, String)> = Vec::new();
    for fixture in &all.fixtures {
        let name = &fixture.name;
        let frame = render(ctx, &scene_for(&all, fixture), fixture.time);
        let digest = recast_testkit::compare::digest_hex(&frame.pixels);
        if let Some((other, _)) = seen.iter().find(|(_, d)| *d == digest) {
            panic!("{name} renders exactly what {other} does, so one of them proves nothing");
        }
        seen.push((name.clone(), digest));
    }
}

/// The golden set is only worth anything if it fails when the picture changes.
/// A tolerance chosen too loosely turns the whole file into decoration.
#[test]
fn the_tolerance_is_tight_enough_to_catch_a_small_shift() {
    let Some(ctx) = context() else { return };
    let all = load_fixtures();
    let plain = all
        .fixtures
        .iter()
        .find(|f| f.name == "plain")
        .expect("the plain fixture");
    let frame = render(ctx, &scene_for(&all, plain), plain.time);
    let mut nudged = frame.pixels.clone();
    // One row shifted by one pixel: the smallest geometry error worth calling a
    // regression, and invisible to any per-pixel assertion next door.
    let row = (frame.height / 2 * frame.width * 4) as usize;
    let width = (frame.width * 4) as usize;
    nudged.copy_within(row..row + width - 4, row + 4);

    let delta =
        recast_testkit::compare::frame_delta(&frame.pixels, &nudged).expect("same-sized frames");
    assert!(
        !delta.is_within(MAX_CHANNEL, MAX_MEAN),
        "a one-pixel shift on one row passed the gate: max {} mean {:.3}",
        delta.max_channel,
        delta.mean_channel
    );
}

/// A fixture has to differ from the same scene with its feature turned OFF, not
/// merely from the other fixtures. Distinctness is cheap to get by accident: a
/// different padding makes two frames differ while the feature under test draws
/// nothing at all.
#[test]
fn every_feature_fixture_differs_from_the_same_scene_without_it() {
    let Some(ctx) = context() else { return };
    let all = load_fixtures();
    let mut checked = 0;
    for fixture in &all.fixtures {
        let Some(without) = &fixture.without else {
            continue;
        };
        checked += 1;
        let name = &fixture.name;
        let off = Fixture {
            name: name.clone(),
            time: fixture.time,
            overrides: without.clone(),
            without: None,
        };
        let a = render(ctx, &scene_for(&all, fixture), fixture.time);
        let b = render(ctx, &scene_for(&all, &off), fixture.time);
        assert_eq!(
            (a.width, a.height),
            (b.width, b.height),
            "{name} changed size"
        );
        let delta =
            recast_testkit::compare::frame_delta(&a.pixels, &b.pixels).expect("same-sized frames");
        assert!(
            delta.differing_pixels > 0,
            "{name} renders exactly what the scene without it renders, so the fixture proves nothing"
        );
    }
    assert!(checked >= 5, "only {checked} fixtures declared a `without`");
}
