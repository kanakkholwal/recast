//! Measurement, not a gate. Run with `--release --ignored --nocapture`.

use recast_compositor::{
    PlaneData, PlaneLayout, RenderSource, Session, SourceColor, SourceGeometry, SourcePlanes,
};
use recast_export::{rgba_to_nv12, FrameLoop, FrameWalk, Nv12Error, PictureSource};
use recast_gpu::{GpuContext, GpuOptions};
use recast_scene::migrate::to_scene;

fn time_conversion(width: u32, height: u32, frames: u32) -> f64 {
    let rgba = vec![128u8; (width * height * 4) as usize];
    let mut out = Vec::new();
    let color = SourceColor::default();
    let start = std::time::Instant::now();
    for _ in 0..frames {
        out.clear();
        rgba_to_nv12(&mut out, &rgba, width, height, &color).expect("converted");
    }
    start.elapsed().as_secs_f64() * 1000.0 / f64::from(frames)
}

#[test]
#[ignore = "measurement: run with --release --ignored --nocapture"]
fn rgba_to_nv12_cost_per_frame() {
    for (w, h, label) in [
        (1280, 720, "720p"),
        (1920, 1080, "1080p"),
        (3840, 2160, "4K"),
    ] {
        let ms = time_conversion(w, h, 10);
        let budget_60 = 1000.0 / 60.0;
        println!(
            "{label:>6}: {ms:7.2} ms/frame   {:5.1}x the whole 60fps frame budget",
            ms / budget_60
        );
    }
}

const BASE: &str = r##"{
    "trimStart": 0.0, "trimEnd": 4.0,
    "backgroundType": "color", "backgroundValue": "#2200ff", "backgroundBlur": 0.0,
    "padding": 0.0, "cursorEnabled": false, "cursorSize": 3.0, "cursorSmoothing": 50.0,
    "cursorHighlightClicks": true, "cursorHighlightColor": "#3b82f6",
    "cursorHighlightOpacity": 40.0, "cursorHideWhenIdle": false, "cursorIdleTimeout": 3.0,
    "zoomRegions": []
}"##;

struct Flat {
    bytes: Vec<u8>,
    width: u32,
    height: u32,
}

impl PictureSource for Flat {
    type Error = std::convert::Infallible;

    fn picture_at(&mut self, _t: f64) -> Result<Option<SourcePlanes<'_>>, Self::Error> {
        Ok(Some(SourcePlanes {
            width: self.width,
            height: self.height,
            layout: PlaneLayout::Nv12,
            color: SourceColor::default(),
            data: PlaneData::Packed(&self.bytes),
        }))
    }
}

/// Render plus readback only: no colour conversion, no encoder. Isolates what
/// the engine costs from what the CPU path around it costs.
#[test]
#[ignore = "measurement: run with --release --ignored --nocapture"]
fn render_and_readback_cost_per_frame() {
    let Some(ctx) = GpuContext::new_blocking(GpuOptions {
        require_hardware: false,
        ..Default::default()
    })
    .ok() else {
        eprintln!("skipping: no adapter");
        return;
    };
    for (w, h, label) in [(1280, 720, "720p"), (1920, 1080, "1080p")] {
        let state = serde_json::from_str(BASE).expect("fixture");
        let mut session = Session::new(
            &ctx,
            to_scene(&state),
            SourceGeometry {
                width: w,
                height: h,
            },
        )
        .expect("session");
        let mut pictures = Flat {
            bytes: vec![128; PlaneLayout::Nv12.packed_len(w, h)],
            width: w,
            height: h,
        };
        let walk = FrameWalk::new(1.0, (30, 1));
        let size = RenderSource::output_size(&session);
        let mut convert = std::time::Duration::ZERO;
        let mut nv12 = Vec::new();
        let color = SourceColor::default();

        let start = std::time::Instant::now();
        FrameLoop::new()
            .run(
                &mut session,
                &mut pictures,
                walk,
                ctx.device(),
                ctx.queue(),
                |_, rgba| {
                    let t = std::time::Instant::now();
                    nv12.clear();
                    rgba_to_nv12(&mut nv12, rgba, size.width, size.height, &color)?;
                    convert += t.elapsed();
                    Ok::<_, Nv12Error>(())
                },
            )
            .expect("rendered");
        let total = start.elapsed().as_secs_f64() * 1000.0 / walk.len() as f64;
        let conv = convert.as_secs_f64() * 1000.0 / walk.len() as f64;
        println!(
            "{label:>6} {}x{}: total {total:6.2} ms/frame  (render+readback {:6.2}, nv12 {conv:6.2})  -> {:5.1} fps",
            size.width,
            size.height,
            total - conv,
            1000.0 / total
        );
    }
}
