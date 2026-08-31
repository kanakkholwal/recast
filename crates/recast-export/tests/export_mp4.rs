#![cfg(windows)]

use recast_codec_mf::VideoReader;
use recast_compositor::{
    PlaneData, PlaneLayout, RenderSource, Session, SourceColor, SourceGeometry, SourcePlanes,
};
use recast_export::{FrameLoop, FrameWalk, Mp4Sink, PictureSource};
use recast_gpu::{GpuContext, GpuOptions};
use recast_scene::migrate::to_scene;
use recast_scene::Scene;

// A real export size: a hardware H.264 encoder refuses a thumbnail-sized one.
const SRC_W: u32 = 640;
const SRC_H: u32 = 360;

fn context() -> Option<&'static GpuContext> {
    static SHARED: std::sync::OnceLock<Option<GpuContext>> = std::sync::OnceLock::new();
    SHARED
        .get_or_init(|| {
            GpuContext::new_blocking(GpuOptions {
                require_hardware: false,
                ..Default::default()
            })
            .map_err(|e| eprintln!("skipping: no GPU adapter ({e})"))
            .ok()
        })
        .as_ref()
}

const BASE: &str = r##"{
    "trimStart": 0.0, "trimEnd": 2.0,
    "backgroundType": "color", "backgroundValue": "#2200ff", "backgroundBlur": 0.0,
    "padding": 0.0, "cursorEnabled": false, "cursorSize": 3.0, "cursorSmoothing": 50.0,
    "cursorHighlightClicks": true, "cursorHighlightColor": "#3b82f6",
    "cursorHighlightOpacity": 40.0, "cursorHideWhenIdle": false, "cursorIdleTimeout": 3.0,
    "zoomRegions": []
}"##;

fn session(ctx: &GpuContext) -> Session {
    let state = serde_json::from_str(BASE).expect("fixture parses");
    let scene: Scene = to_scene(&state);
    Session::new(
        ctx,
        scene,
        SourceGeometry {
            width: SRC_W,
            height: SRC_H,
        },
    )
    .expect("session")
}

struct Flat(Vec<u8>);

impl Flat {
    fn new(luma: u8) -> Self {
        let mut bytes = vec![luma; (SRC_W * SRC_H) as usize];
        bytes.resize(PlaneLayout::Nv12.packed_len(SRC_W, SRC_H), 128);
        Self(bytes)
    }
}

impl PictureSource for Flat {
    type Error = std::convert::Infallible;

    fn picture_at(&mut self, _source_time: f64) -> Result<Option<SourcePlanes<'_>>, Self::Error> {
        Ok(Some(SourcePlanes {
            width: SRC_W,
            height: SRC_H,
            layout: PlaneLayout::Nv12,
            color: SourceColor::default(),
            data: PlaneData::Packed(&self.0),
        }))
    }
}

/// Removes itself, so a failing run does not leave a file behind. The audit
/// found 164 MB of temp directories left by tests that skipped this.
struct Scratch(std::path::PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("recast-export-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch dir");
        Self(dir)
    }

    fn file(&self, name: &str) -> std::path::PathBuf {
        self.0.join(name)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Renders `walk` through the engine into a finished MP4 on disk.
fn export(ctx: &GpuContext, walk: FrameWalk, path: &std::path::Path) -> u64 {
    export_with(ctx, walk, path, 200)
}

/// Mean luma of the first decoded frame. NV12 puts the luma plane first, so
/// this reads the picture rather than the container.
fn first_frame_luma(path: &std::path::Path) -> f64 {
    let mut reader = VideoReader::open(path).expect("the file opens");
    let frame = reader
        .next_frame()
        .expect("decode")
        .expect("at least one frame");
    let info = reader.info();
    let luma = (info.width * info.height) as usize;
    let plane = &frame.data[..luma.min(frame.data.len())];
    plane.iter().map(|&b| f64::from(b)).sum::<f64>() / plane.len() as f64
}

fn export_with(ctx: &GpuContext, walk: FrameWalk, path: &std::path::Path, luma: u8) -> u64 {
    let mut session = session(ctx);
    let size = RenderSource::output_size(&session);
    let mut sink = Mp4Sink::new(
        size.width,
        size.height,
        walk,
        4_000_000,
        SourceColor::default(),
    )
    .expect("an H.264 encoder");

    FrameLoop::new()
        .run(
            &mut session,
            &mut Flat::new(luma),
            walk,
            ctx.device(),
            ctx.queue(),
            |index, frame| sink.push(index, frame),
        )
        .expect("rendered");

    assert!(
        !sink.saw_reordering(),
        "the encoder reordered samples; this writer emits no composition offsets"
    );
    let bytes = sink.finish().expect("a finished file");
    std::fs::write(path, &bytes).expect("write");
    bytes.len() as u64
}

/// The whole point of Phase 6: engine pixels reach a real file, and a decoder
/// that never saw our code can read them back.
#[test]
fn an_export_round_trips_through_a_real_mp4() {
    let Some(ctx) = context() else {
        if recast_testkit::skip_or_fail("no GPU adapter") {
            return;
        }
        unreachable!("skip_or_fail either panics or says to skip")
    };
    let scratch = Scratch::new("roundtrip");
    let path = scratch.file("out.mp4");
    let walk = FrameWalk::new(0.5, (30, 1));

    let bytes = export(ctx, walk, &path);
    assert!(bytes > 0, "the file is empty");

    let mut reader = VideoReader::open(&path).expect("the file opens");
    let info = reader.info();
    let expected = RenderSource::output_size(&session(ctx));
    assert_eq!((info.width, info.height), (expected.width, expected.height));

    let mut decoded = 0u64;
    while reader.next_frame().expect("decode").is_some() {
        decoded += 1;
    }
    assert_eq!(decoded, walk.len(), "frames went missing");
}

#[test]
fn the_file_reports_the_frame_rate_it_was_exported_at() {
    let Some(ctx) = context() else {
        if recast_testkit::skip_or_fail("no GPU adapter") {
            return;
        }
        unreachable!("skip_or_fail either panics or says to skip")
    };
    let scratch = Scratch::new("fps");
    let path = scratch.file("out.mp4");
    export(ctx, FrameWalk::new(0.4, (60, 1)), &path);

    let reader = VideoReader::open(&path).expect("the file opens");
    let (num, den) = reader.info().frame_rate;
    let fps = f64::from(num) / f64::from(den.max(1));
    assert!(
        (fps - 60.0).abs() < 0.5,
        "reported {fps} fps, exported at 60"
    );
}

/// The judder bug in reverse: a 24 fps export must not come back as 25 or 30.
#[test]
fn a_non_default_frame_rate_survives_to_the_file() {
    let Some(ctx) = context() else {
        if recast_testkit::skip_or_fail("no GPU adapter") {
            return;
        }
        unreachable!("skip_or_fail either panics or says to skip")
    };
    let scratch = Scratch::new("24fps");
    let path = scratch.file("out.mp4");
    let walk = FrameWalk::new(0.5, (24, 1));
    export(ctx, walk, &path);

    let mut reader = VideoReader::open(&path).expect("the file opens");
    let (num, den) = reader.info().frame_rate;
    let fps = f64::from(num) / f64::from(den.max(1));
    assert!(
        (fps - 24.0).abs() < 0.5,
        "reported {fps} fps, exported at 24"
    );

    let mut decoded = 0u64;
    while reader.next_frame().expect("decode").is_some() {
        decoded += 1;
    }
    assert_eq!(decoded, walk.len());
}

#[test]
fn an_oversized_frame_is_refused_before_the_encoder_opens() {
    let error = Mp4Sink::new(
        70_000,
        1080,
        FrameWalk::new(1.0, (30, 1)),
        1_000_000,
        SourceColor::default(),
    )
    .err()
    .expect("70000 pixels does not fit an MP4 track header");
    assert!(format!("{error}").contains("70000"), "{error}");
}

/// Count, size and frame rate all pass for a file of pure black. This is the
/// test that fails if the picture never reaches the encoder.
#[test]
fn the_rendered_picture_survives_into_the_encoded_file() {
    let Some(ctx) = context() else {
        if recast_testkit::skip_or_fail("no GPU adapter") {
            return;
        }
        unreachable!("skip_or_fail either panics or says to skip")
    };
    let scratch = Scratch::new("pixels");
    let walk = FrameWalk::new(0.2, (30, 1));

    let dark = scratch.file("dark.mp4");
    let bright = scratch.file("bright.mp4");
    export_with(ctx, walk, &dark, 16);
    export_with(ctx, walk, &bright, 235);

    let dark_luma = first_frame_luma(&dark);
    let bright_luma = first_frame_luma(&bright);
    assert!(
        bright_luma - dark_luma > 20.0,
        "the picture did not reach the file: dark {dark_luma}, bright {bright_luma}"
    );
    assert!(
        dark_luma > 1.0,
        "the whole frame decoded as black: {dark_luma}"
    );
}

/// Every frame pushed becomes a sample once the encoder has been drained.
#[test]
fn finishing_drains_every_frame_the_encoder_was_holding() {
    let Some(ctx) = context() else {
        if recast_testkit::skip_or_fail("no GPU adapter") {
            return;
        }
        unreachable!("skip_or_fail either panics or says to skip")
    };
    let scratch = Scratch::new("drain");
    let walk = FrameWalk::new(0.5, (30, 1));
    let mut session = session(ctx);
    let size = RenderSource::output_size(&session);
    let mut sink = Mp4Sink::new(
        size.width,
        size.height,
        walk,
        4_000_000,
        SourceColor::default(),
    )
    .expect("an H.264 encoder");

    FrameLoop::new()
        .run(
            &mut session,
            &mut Flat::new(200),
            walk,
            ctx.device(),
            ctx.queue(),
            |index, frame| sink.push(index, frame),
        )
        .expect("rendered");

    let path = scratch.file("out.mp4");
    std::fs::write(&path, sink.finish().expect("finished")).expect("write");

    let mut reader = VideoReader::open(&path).expect("opens");
    let mut decoded = 0u64;
    while reader.next_frame().expect("decode").is_some() {
        decoded += 1;
    }
    assert_eq!(decoded, walk.len());
}

/// Converting on the GPU must produce the SAME file as converting the readback
/// on the CPU: the two feed one encoder, and the GPU pass exists only to be
/// faster. A sink that converted an already-NV12 frame again would land here.
#[test]
fn the_gpu_and_cpu_conversions_encode_the_same_file() {
    let Some(ctx) = context() else {
        if recast_testkit::skip_or_fail("no GPU adapter") {
            return;
        }
        unreachable!("skip_or_fail either panics or says to skip")
    };

    let encode = |on_gpu: bool| -> Vec<u8> {
        let mut session = session(ctx);
        let mut pictures = Flat::new(200);
        let walk = FrameWalk::new(0.4, (30, 1));
        let size = RenderSource::output_size(&session);
        let mut sink = Mp4Sink::new(
            size.width,
            size.height,
            walk,
            4_000_000,
            SourceColor::default(),
        )
        .expect("an encoder");
        let mut frames = match on_gpu {
            true => FrameLoop::with_nv12(SourceColor::default()),
            false => FrameLoop::new(),
        };
        frames
            .run(
                &mut session,
                &mut pictures,
                walk,
                ctx.device(),
                ctx.queue(),
                |index, frame| sink.push(index, frame),
            )
            .expect("rendered");
        sink.finish().expect("finished")
    };

    let on_cpu = encode(false);
    let on_gpu = encode(true);
    assert_eq!(
        on_gpu.len(),
        on_cpu.len(),
        "the two conversions produced different file lengths"
    );
    let differing = on_gpu.iter().zip(&on_cpu).filter(|(a, b)| a != b).count();
    assert_eq!(
        differing, 0,
        "{differing} bytes differ between the two paths"
    );
}

/// The loop has to actually take the GPU path for a shape the shader packs. A
/// sink opened for NV12 refuses an RGBA frame, so a silent fallback fails here
/// rather than quietly costing nine times the conversion.
#[test]
fn a_packable_shape_really_is_converted_on_the_gpu() {
    let Some(ctx) = context() else {
        if recast_testkit::skip_or_fail("no GPU adapter") {
            return;
        }
        unreachable!("skip_or_fail either panics or says to skip")
    };
    let mut session = session(ctx);
    let size = RenderSource::output_size(&session);
    assert!(
        recast_export::GpuNv12::handles(size.width, size.height),
        "the fixture size is not one the shader packs"
    );

    let mut pictures = Flat::new(200);
    let walk = FrameWalk::new(0.2, (30, 1));
    let mut layouts = Vec::new();
    FrameLoop::with_nv12(SourceColor::default())
        .run(
            &mut session,
            &mut pictures,
            walk,
            ctx.device(),
            ctx.queue(),
            |_, frame| {
                layouts.push(frame.layout());
                Ok::<_, std::convert::Infallible>(())
            },
        )
        .expect("rendered");

    assert!(!layouts.is_empty(), "nothing rendered");
    assert!(
        layouts
            .iter()
            .all(|l| *l == recast_export::PixelLayout::Nv12),
        "the loop fell back to the CPU readback: {layouts:?}"
    );
}
