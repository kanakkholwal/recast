//! Measurement, not a gate. Run with `--release --ignored --nocapture`.
//!
//! Every number is also printed as a `bench|` row so CI can lift the table out
//! per OS without parsing prose. The numbers are only comparable within one
//! runner: a hosted CI GPU is not the machine anyone exports on.

/// One measurement, in the shape the CI step turns into a table row.
fn bench(name: &str, case: &str, ms: f64) {
    println!("bench|{name}|{case}|{ms:.3}");
}

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
        bench("nv12-cpu", label, ms);
    }
}

/// The whole export loop, both ways: this is the number an export feels.
#[test]
#[ignore = "measurement: run with --release --ignored --nocapture"]
fn export_loop_cost_per_frame() {
    let Some(ctx) = GpuContext::new_blocking(GpuOptions {
        require_hardware: false,
        ..Default::default()
    })
    .ok() else {
        if recast_testkit::skip_or_fail("no GPU adapter") {
            return;
        }
        unreachable!("skip_or_fail either panics or says to skip")
    };
    for (w, h, label) in [(1280, 720, "720p"), (1920, 1080, "1080p")] {
        let mut timings = Vec::new();
        for gpu in [false, true] {
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
            let color = SourceColor::default();
            let mut nv12 = Vec::new();
            let mut frames = match gpu {
                true => FrameLoop::with_nv12(color),
                false => FrameLoop::new(),
            };

            let start = std::time::Instant::now();
            frames
                .run(
                    &mut session,
                    &mut pictures,
                    walk,
                    &ctx,
                    recast_export::Extras::default(),
                    |_, frame| {
                        // The CPU path still owes the conversion the sink would do.
                        if let recast_export::Frame::Rgba(rgba) = frame {
                            nv12.clear();
                            rgba_to_nv12(&mut nv12, rgba, size.width, size.height, &color)?;
                        }
                        Ok::<_, Nv12Error>(())
                    },
                )
                .expect("rendered");
            timings.push(start.elapsed().as_secs_f64() * 1000.0 / walk.len() as f64);
        }
        let (cpu, on_gpu) = (timings[0], timings[1]);
        bench("loop-cpu", label, cpu);
        bench("loop-gpu", label, on_gpu);
        println!(
            "{label:>6}: cpu loop {cpu:6.2} ms/frame ({:5.1} fps)   gpu loop {on_gpu:6.2} ms/frame ({:5.1} fps)   {:4.1}x",
            1000.0 / cpu,
            1000.0 / on_gpu,
            cpu / on_gpu.max(0.0001)
        );
    }
}

/// The GPU converter against the CPU one, on the same frames. The number here
/// decides whether the export loop should use it.
#[test]
#[ignore = "measurement: run with --release --ignored --nocapture"]
fn gpu_nv12_cost_per_frame() {
    use recast_export::GpuNv12;
    use recast_gpu::OUTPUT_FORMAT;

    let Some(ctx) = GpuContext::new_blocking(GpuOptions {
        require_hardware: false,
        ..Default::default()
    })
    .ok() else {
        if recast_testkit::skip_or_fail("no GPU adapter") {
            return;
        }
        unreachable!("skip_or_fail either panics or says to skip")
    };
    let color = SourceColor::default();
    let mut gpu = GpuNv12::new(ctx.device());

    for (w, h, label) in [
        (1280, 720, "720p"),
        (1920, 1080, "1080p"),
        (3840, 2160, "4K"),
    ] {
        let rgba = vec![128u8; (w * h * 4) as usize];
        let texture = ctx.device().create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: OUTPUT_FORMAT,
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
            &rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(w * 4),
                rows_per_image: Some(h),
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );

        let mut out = Vec::new();
        // One warm run so shader compilation is not counted as frame cost.
        gpu.convert(ctx.device(), ctx.queue(), &texture, &color, &mut out);
        let frames = 10;
        let start = std::time::Instant::now();
        for _ in 0..frames {
            gpu.convert(ctx.device(), ctx.queue(), &texture, &color, &mut out);
        }
        let on_gpu = start.elapsed().as_secs_f64() * 1000.0 / f64::from(frames);
        let on_cpu = time_conversion(w, h, 10);
        println!(
            "{label:>6}: cpu {on_cpu:7.2} ms   gpu {on_gpu:7.2} ms   {:5.1}x faster",
            on_cpu / on_gpu.max(0.0001)
        );
        bench("nv12-gpu", label, on_gpu);
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
        if recast_testkit::skip_or_fail("no GPU adapter") {
            return;
        }
        unreachable!("skip_or_fail either panics or says to skip")
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
        let mut cpu_loop = FrameLoop::new();
        cpu_loop
            .run(
                &mut session,
                &mut pictures,
                walk,
                &ctx,
                recast_export::Extras::default(),
                |_, frame| {
                    let t = std::time::Instant::now();
                    nv12.clear();
                    rgba_to_nv12(&mut nv12, frame.bytes(), size.width, size.height, &color)?;
                    convert += t.elapsed();
                    Ok::<_, Nv12Error>(())
                },
            )
            .expect("rendered");
        let total = start.elapsed().as_secs_f64() * 1000.0 / walk.len() as f64;
        let conv = convert.as_secs_f64() * 1000.0 / walk.len() as f64;
        bench("render+readback", label, total - conv);
        println!(
            "{label:>6} {}x{}: total {total:6.2} ms/frame  (render+readback {:6.2}, nv12 {conv:6.2})  -> {:5.1} fps",
            size.width,
            size.height,
            total - conv,
            1000.0 / total
        );
    }
}
