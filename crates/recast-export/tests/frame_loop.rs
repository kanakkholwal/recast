use recast_compositor::{
    PlaneData, PlaneLayout, RenderSource, Session, SourceColor, SourceGeometry, SourcePlanes,
};
use recast_export::{FrameLoop, FrameWalk, NoPictures, PictureSource, RenderError};
use recast_gpu::{GpuContext, GpuOptions};
use recast_scene::migrate::to_scene;
use recast_scene::Scene;
use std::convert::Infallible;

const SRC_W: u32 = 64;
const SRC_H: u32 = 32;

/// One device for the binary. A context per test is a device per test, which is
/// what crashed CI on the software adapter.
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
    "padding": 8.0, "cursorEnabled": false, "cursorSize": 3.0, "cursorSmoothing": 50.0,
    "cursorHighlightClicks": true, "cursorHighlightColor": "#3b82f6",
    "cursorHighlightOpacity": 40.0, "cursorHideWhenIdle": false, "cursorIdleTimeout": 3.0,
    "zoomRegions": []
}"##;

fn scene() -> Scene {
    let state = serde_json::from_str(BASE).expect("fixture parses");
    to_scene(&state)
}

fn session(ctx: &GpuContext) -> Session {
    Session::new(
        ctx,
        scene(),
        SourceGeometry {
            width: SRC_W,
            height: SRC_H,
        },
    )
    .expect("session")
}

/// A flat NV12 picture, so the loop has something real to upload.
struct Flat {
    bytes: Vec<u8>,
    calls: u32,
}

impl Flat {
    fn new(luma: u8) -> Self {
        let mut bytes = vec![luma; (SRC_W * SRC_H) as usize];
        bytes.resize(PlaneLayout::Nv12.packed_len(SRC_W, SRC_H), 128);
        Self { bytes, calls: 0 }
    }
}

impl PictureSource for Flat {
    type Error = std::convert::Infallible;

    fn picture_at(&mut self, _source_time: f64) -> Result<Option<SourcePlanes<'_>>, Self::Error> {
        self.calls += 1;
        Ok(Some(SourcePlanes {
            width: SRC_W,
            height: SRC_H,
            layout: PlaneLayout::Nv12,
            color: SourceColor::default(),
            data: PlaneData::Packed(&self.bytes),
        }))
    }
}

#[test]
fn the_loop_writes_one_frame_per_walk_step() {
    let Some(ctx) = context() else { return };
    let mut session = session(ctx);
    let walk = FrameWalk::new(0.2, (30, 1));
    let mut seen = Vec::new();

    let count = FrameLoop::new()
        .run(
            &mut session,
            &mut Flat::new(200),
            walk,
            ctx.device(),
            ctx.queue(),
            |index, _| {
                seen.push(index);
                Ok::<_, Infallible>(())
            },
        )
        .expect("rendered");

    assert_eq!(count, walk.len());
    assert_eq!(seen, (0..walk.len()).collect::<Vec<_>>());
}

#[test]
fn every_frame_is_canvas_sized_rgba() {
    let Some(ctx) = context() else { return };
    let mut session = session(ctx);
    let size = RenderSource::output_size(&session);
    let want = (size.width * size.height * 4) as usize;
    let mut lengths = Vec::new();

    FrameLoop::new()
        .run(
            &mut session,
            &mut Flat::new(200),
            FrameWalk::new(0.1, (30, 1)),
            ctx.device(),
            ctx.queue(),
            |_, pixels| {
                lengths.push(pixels.len());
                Ok::<_, Infallible>(())
            },
        )
        .expect("rendered");

    assert!(!lengths.is_empty());
    assert!(
        lengths.iter().all(|&l| l == want),
        "{lengths:?} want {want}"
    );
}

/// The picture has to actually reach the canvas. A loop that drew only the
/// background and dropped every upload would pass a frame-count test.
#[test]
fn the_uploaded_picture_reaches_the_frame() {
    let Some(ctx) = context() else { return };
    let mut without = Vec::new();
    FrameLoop::new()
        .run(
            &mut session(ctx),
            &mut NoPictures,
            FrameWalk::new(0.04, (30, 1)),
            ctx.device(),
            ctx.queue(),
            |_, pixels| {
                without = pixels.to_vec();
                Ok::<_, Infallible>(())
            },
        )
        .expect("rendered");

    let mut with = Vec::new();
    FrameLoop::new()
        .run(
            &mut session(ctx),
            &mut Flat::new(235),
            FrameWalk::new(0.04, (30, 1)),
            ctx.device(),
            ctx.queue(),
            |_, pixels| {
                with = pixels.to_vec();
                Ok::<_, Infallible>(())
            },
        )
        .expect("rendered");

    assert_ne!(with, without, "the picture never reached the canvas");
    let bright = with
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|p| p[0] > 200 && p[1] > 200)
        .count();
    assert!(bright > 0, "the white picture drew nothing bright");
}

/// The whole point of holding the buffers: an export is thousands of frames.
#[test]
fn a_steady_loop_allocates_one_source_texture() {
    let Some(ctx) = context() else { return };
    let mut frames = FrameLoop::new();
    frames
        .run(
            &mut session(ctx),
            &mut Flat::new(200),
            FrameWalk::new(1.0, (30, 1)),
            ctx.device(),
            ctx.queue(),
            |_, _| Ok::<_, Infallible>(()),
        )
        .expect("rendered");
    assert_eq!(frames.source_allocations(), 1);
}

#[test]
fn a_sink_failure_stops_the_loop_rather_than_finishing_the_export() {
    let Some(ctx) = context() else { return };
    let mut rendered = 0u64;
    let error = FrameLoop::new()
        .run(
            &mut session(ctx),
            &mut Flat::new(200),
            FrameWalk::new(1.0, (30, 1)),
            ctx.device(),
            ctx.queue(),
            |index, _| {
                rendered += 1;
                if index == 3 {
                    return Err(std::io::Error::other("disk full"));
                }
                Ok(())
            },
        )
        .expect_err("the sink failed");

    // Structure, not a string: the error names the frame and keeps the cause.
    assert!(
        matches!(error, RenderError::Sink { index: 3, .. }),
        "{error:?}"
    );
    let cause = std::error::Error::source(&error).expect("the sink error is the cause");
    assert!(cause.to_string().contains("disk full"), "{cause}");
    assert_eq!(rendered, 4, "the loop kept going after the sink failed");
}

#[test]
fn a_document_with_nothing_to_render_writes_no_frames() {
    let Some(ctx) = context() else { return };
    let mut called = false;
    let count = FrameLoop::new()
        .run(
            &mut session(ctx),
            &mut NoPictures,
            FrameWalk::new(0.0, (30, 1)),
            ctx.device(),
            ctx.queue(),
            |_, _| {
                called = true;
                Ok::<_, Infallible>(())
            },
        )
        .expect("rendered");
    assert_eq!(count, 0);
    assert!(!called);
}

/// Output time and source time are different axes, so the loop asks the picture
/// source on the source axis, once per output frame.
#[test]
fn the_picture_source_is_asked_once_per_output_frame() {
    let Some(ctx) = context() else { return };
    let walk = FrameWalk::new(0.5, (30, 1));
    let mut pictures = Flat::new(200);
    FrameLoop::new()
        .run(
            &mut session(ctx),
            &mut pictures,
            walk,
            ctx.device(),
            ctx.queue(),
            |_, _| Ok::<_, Infallible>(()),
        )
        .expect("rendered");
    assert_eq!(u64::from(pictures.calls), walk.len());
}
