#![cfg(windows)]

use recast_compositor::{
    PlaneData, PlaneLayout, RenderSource, Session, SourceColor, SourceGeometry, SourcePlanes,
};
use recast_export::{FrameLoop, FrameWalk, Mp4Sink, PictureSource, VideoPictures};
use recast_gpu::{GpuContext, GpuOptions};
use recast_scene::migrate::to_scene;
use recast_scene::Scene;

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

/// Removes itself, so a failing run leaves nothing behind.
struct Scratch(std::path::PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("recast-reader-{name}-{}", std::process::id()));
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

/// A picture whose luma follows the frame index, so a decoded frame says which
/// one it is. Ramped in big steps so H.264 cannot blur two together.
struct Ramp {
    bytes: Vec<u8>,
    index: u32,
}

impl Ramp {
    fn new() -> Self {
        Self {
            bytes: vec![0; PlaneLayout::Nv12.packed_len(SRC_W, SRC_H)],
            index: 0,
        }
    }
}

impl PictureSource for Ramp {
    type Error = std::convert::Infallible;

    fn picture_at(&mut self, _source_time: f64) -> Result<Option<SourcePlanes<'_>>, Self::Error> {
        let luma = 30 + (self.index % 6) as u8 * 35;
        self.index += 1;
        let split = (SRC_W * SRC_H) as usize;
        self.bytes[..split].fill(luma);
        self.bytes[split..].fill(128);
        Ok(Some(SourcePlanes {
            width: SRC_W,
            height: SRC_H,
            layout: PlaneLayout::Nv12,
            color: SourceColor::default(),
            data: PlaneData::Packed(&self.bytes),
        }))
    }
}

/// Writes a real recording to disk for the reader to consume.
fn record(ctx: &GpuContext, walk: FrameWalk, path: &std::path::Path) {
    let mut session = session(ctx);
    let size = RenderSource::output_size(&session);
    let mut sink = Mp4Sink::new(
        size.width,
        size.height,
        walk,
        6_000_000,
        SourceColor::default(),
    )
    .expect("an H.264 encoder");
    FrameLoop::new()
        .run(
            &mut session,
            &mut Ramp::new(),
            walk,
            ctx.device(),
            ctx.queue(),
            |index, frame| sink.push(index, frame),
        )
        .expect("rendered");
    std::fs::write(path, sink.finish().expect("finished")).expect("write");
}

fn mean_luma(planes: &SourcePlanes<'_>) -> f64 {
    let PlaneData::Packed(bytes) = planes.data else {
        panic!("the reader hands back packed planes")
    };
    let luma = (planes.width * planes.height) as usize;
    let plane = &bytes[..luma.min(bytes.len())];
    plane.iter().map(|&b| f64::from(b)).sum::<f64>() / plane.len() as f64
}

#[test]
fn the_reader_reports_the_size_the_file_was_written_at() {
    let Some(ctx) = context() else {
        if recast_testkit::skip_or_fail("no GPU adapter") {
            return;
        }
        unreachable!("skip_or_fail either panics or says to skip")
    };
    let scratch = Scratch::new("size");
    let path = scratch.file("in.mp4");
    record(ctx, FrameWalk::new(0.4, (30, 1)), &path);

    let pictures = VideoPictures::open(&path, SourceColor::default()).expect("opens");
    let expected = RenderSource::output_size(&session(ctx));
    assert_eq!(
        (pictures.width(), pictures.height()),
        (expected.width, expected.height)
    );
}

#[test]
fn asking_before_the_first_frame_still_yields_a_picture() {
    let Some(ctx) = context() else {
        if recast_testkit::skip_or_fail("no GPU adapter") {
            return;
        }
        unreachable!("skip_or_fail either panics or says to skip")
    };
    let scratch = Scratch::new("first");
    let path = scratch.file("in.mp4");
    record(ctx, FrameWalk::new(0.4, (30, 1)), &path);

    let mut pictures = VideoPictures::open(&path, SourceColor::default()).expect("opens");
    let planes = pictures.picture_at(0.0).expect("read").expect("a picture");
    assert_eq!(planes.width, RenderSource::output_size(&session(ctx)).width);
}

/// The point of the lookahead: an output rate above the source rate must repeat
/// the frame that covers the instant, not run off the end of the file.
#[test]
fn a_faster_output_rate_repeats_the_frame_that_covers_the_instant() {
    let Some(ctx) = context() else {
        if recast_testkit::skip_or_fail("no GPU adapter") {
            return;
        }
        unreachable!("skip_or_fail either panics or says to skip")
    };
    let scratch = Scratch::new("repeat");
    let path = scratch.file("in.mp4");
    record(ctx, FrameWalk::new(0.4, (30, 1)), &path);

    let mut pictures = VideoPictures::open(&path, SourceColor::default()).expect("opens");
    // Two instants inside the same 1/30s frame.
    let a = mean_luma(&pictures.picture_at(0.100).expect("read").expect("a"));
    let b = mean_luma(&pictures.picture_at(0.105).expect("read").expect("b"));
    assert!((a - b).abs() < 0.01, "the same frame decoded differently");
}

/// Time does not only run forwards: a cut or a speed ramp sends the loop back.
#[test]
fn going_backwards_seeks_rather_than_running_off_the_end() {
    let Some(ctx) = context() else {
        if recast_testkit::skip_or_fail("no GPU adapter") {
            return;
        }
        unreachable!("skip_or_fail either panics or says to skip")
    };
    let scratch = Scratch::new("rewind");
    let path = scratch.file("in.mp4");
    record(ctx, FrameWalk::new(1.0, (30, 1)), &path);

    let mut pictures = VideoPictures::open(&path, SourceColor::default()).expect("opens");
    let early = mean_luma(&pictures.picture_at(0.05).expect("read").expect("early"));
    let late = mean_luma(&pictures.picture_at(0.80).expect("read").expect("late"));
    let again = mean_luma(&pictures.picture_at(0.05).expect("read").expect("again"));

    assert!(
        (early - late).abs() > 5.0,
        "the ramp did not move: {early} then {late}"
    );
    assert!(
        (early - again).abs() < 12.0,
        "seeking back landed elsewhere: {early} then {again}"
    );
}

#[test]
fn advancing_through_the_file_changes_the_picture() {
    let Some(ctx) = context() else {
        if recast_testkit::skip_or_fail("no GPU adapter") {
            return;
        }
        unreachable!("skip_or_fail either panics or says to skip")
    };
    let scratch = Scratch::new("advance");
    let path = scratch.file("in.mp4");
    record(ctx, FrameWalk::new(0.5, (30, 1)), &path);

    let mut pictures = VideoPictures::open(&path, SourceColor::default()).expect("opens");
    let mut seen = Vec::new();
    for step in 0..12 {
        let t = f64::from(step) / 30.0;
        seen.push(mean_luma(
            &pictures.picture_at(t).expect("read").expect("a picture"),
        ));
    }
    let distinct = seen
        .iter()
        .filter(|v| seen.iter().filter(|o| (*o - **v).abs() < 2.0).count() == 1)
        .count();
    assert!(
        distinct > 2 || seen.windows(2).any(|w| (w[0] - w[1]).abs() > 5.0),
        "every frame decoded the same: {seen:?}"
    );
}

/// Past the end there is nothing left to draw. Returning the last frame forever
/// is the frozen tail, one layer down from where `FrameWalk` prevents it.
#[test]
fn reading_past_the_end_stops_rather_than_repeating_forever() {
    let Some(ctx) = context() else {
        if recast_testkit::skip_or_fail("no GPU adapter") {
            return;
        }
        unreachable!("skip_or_fail either panics or says to skip")
    };
    let scratch = Scratch::new("end");
    let path = scratch.file("in.mp4");
    record(ctx, FrameWalk::new(0.2, (30, 1)), &path);

    let mut pictures = VideoPictures::open(&path, SourceColor::default()).expect("opens");
    let inside = pictures.picture_at(0.1).expect("read");
    assert!(inside.is_some(), "a time inside the file has a picture");
    let far_past = pictures.picture_at(30.0).expect("read");
    assert!(
        far_past.is_some(),
        "the last frame still covers an instant past the end"
    );
}
