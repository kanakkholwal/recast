#![cfg(windows)]

use std::path::PathBuf;

use recast_codec::{ranked, VideoCodec};
use recast_codec_mf::{
    enumerate_encoders, D3dContext, EncodeConfig, EncodedSample, H264Encoder, VideoReader,
};
use recast_gpu::{
    import_shared_fence, import_shared_texture, GpuContext, GpuOptions, SharedFormat, SharedHandle,
    SharedTextureDesc,
};
use recast_mux::{annex_b_to_avcc, split_access_units, AvcConfig, Mp4Writer, VideoFormat};

const WIDTH: u32 = 320;
const HEIGHT: u32 = 240;
const FPS: u32 = 30;
const FRAMES: u32 = 20;
const FRAME_DURATION: i64 = 10_000_000 / FPS as i64;

/// Four flat bands (red, green, two greys) pinning all three colour decisions the video processor makes.
/// Red separates BT.709 from BT.601, which greys cannot; the bright grey separates full from studio range, which saturated colours cannot, since both clip to the same place.
const BANDS: &str = r#"
@vertex
fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    var p = array<vec2<f32>, 3>(vec2(-1.0, -3.0), vec2(-1.0, 1.0), vec2(3.0, 1.0));
    return vec4<f32>(p[i], 0.0, 1.0);
}

@fragment
fn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    if pos.x < 80.0 {
        return vec4<f32>(1.0, 0.0, 0.0, 1.0);
    }
    if pos.x < 160.0 {
        return vec4<f32>(0.0, 1.0, 0.0, 1.0);
    }
    if pos.x < 240.0 {
        return vec4<f32>(0.4, 0.4, 0.4, 1.0);
    }
    return vec4<f32>(0.9, 0.9, 0.9, 1.0);
}
"#;

/// BT.709 studio range: `Y = 16 + 219 * (0.2126 R + 0.7152 G + 0.0722 B)`.
fn expected_luma(r: f64, g: f64, b: f64) -> f64 {
    16.0 + 219.0 * (0.2126 * r + 0.7152 * g + 0.0722 * b)
}

fn grey_luma(level: f64) -> f64 {
    expected_luma(level, level, level)
}

/// A ramp, so a frame that arrives stale is identifiable rather than merely
/// wrong: the level it carries says which frame it came from.
fn frame_level(index: u32) -> f64 {
    0.15 + 0.035 * index as f64
}

struct Harness {
    d3d: D3dContext,
    context: GpuContext,
    surface: recast_codec_mf::SharedSurface,
    drawn: recast_codec_mf::SyncFence,
    consumed: recast_codec_mf::SyncFence,
    imported: recast_gpu::SharedTexture,
    gpu_drawn: recast_gpu::SharedFence,
    gpu_consumed: recast_gpu::SharedFence,
    nv12: recast_codec_mf::Nv12Converter,
    encoder: H264Encoder,
}

/// `None` skips, loudly. Everything here needs real hardware.
fn harness() -> Option<Harness> {
    let Ok(d3d) = D3dContext::new() else {
        eprintln!("skipping: no D3D11 device with video support");
        return None;
    };
    let context = match GpuContext::new_blocking(GpuOptions::default()) {
        Ok(context) if !context.is_software() => context,
        Ok(_) => {
            eprintln!("skipping: only a software adapter, which cannot share surfaces");
            return None;
        }
        Err(error) => {
            eprintln!("skipping: no wgpu adapter ({error})");
            return None;
        }
    };
    if !context.supports_zero_copy_import() {
        eprintln!("skipping: this adapter cannot import shared surfaces");
        return None;
    }

    let surface = d3d.shared_surface(WIDTH, HEIGHT).ok()?;
    let drawn = d3d.shared_fence().ok()?;
    let consumed = d3d.shared_fence().ok()?;
    let imported = import_shared_texture(
        &context,
        SharedHandle(surface.duplicate_handle().ok()?),
        SharedTextureDesc::new(WIDTH, HEIGHT, SharedFormat::Bgra8Unorm).as_render_target(),
    )
    .ok()?;
    let gpu_drawn =
        import_shared_fence(&context, SharedHandle(drawn.duplicate_handle().ok()?)).ok()?;
    let gpu_consumed =
        import_shared_fence(&context, SharedHandle(consumed.duplicate_handle().ok()?)).ok()?;
    let nv12 = d3d.nv12_converter(WIDTH, HEIGHT, (FPS, 1)).ok()?;

    let config = EncodeConfig {
        width: WIDTH,
        height: HEIGHT,
        frame_rate: (FPS, 1),
        bitrate: 8_000_000,
        keyframe_interval: 0,
    };
    let found = enumerate_encoders();
    let mut encoder = None;
    for descriptor in ranked(&found, VideoCodec::H264) {
        if let Ok(open) = H264Encoder::open_with_gpu(descriptor, config, &d3d) {
            if open.takes_textures() {
                eprintln!("encoding textures with {}", descriptor.name);
                encoder = Some(open);
                break;
            }
        }
    }
    let encoder = encoder.or_else(|| {
        eprintln!("skipping: no encoder took a D3D11 device");
        None
    })?;

    Some(Harness {
        d3d,
        context,
        surface,
        drawn,
        consumed,
        imported,
        gpu_drawn,
        gpu_consumed,
        nv12,
        encoder,
    })
}

/// Draws the band pattern into the shared surface.
fn draw_bands(context: &GpuContext, target: &wgpu::Texture) {
    let device = context.device();
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: None,
        source: wgpu::ShaderSource::Wgsl(BANDS.into()),
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: None,
        bind_group_layouts: &[],
        immediate_size: 0,
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: None,
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &module,
            entry_point: Some("vs"),
            buffers: &[],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &module,
            entry_point: Some("fs"),
            targets: &[Some(target.format().into())],
            compilation_options: Default::default(),
        }),
        primitive: Default::default(),
        depth_stencil: None,
        multisample: Default::default(),
        multiview_mask: None,
        cache: None,
    });

    let view = target.create_view(&Default::default());
    let mut encoder = device.create_command_encoder(&Default::default());
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&pipeline);
        pass.draw(0..3, 0..1);
    }
    context.queue().submit([encoder.finish()]);
}

/// Fills the shared surface with one flat level. No pipeline: a clear is a real
/// GPU write, and this fixture is about which frame arrives, not about shading.
fn clear_to(context: &GpuContext, target: &wgpu::Texture, level: f64) {
    let view = target.create_view(&Default::default());
    let mut encoder = context.device().create_command_encoder(&Default::default());
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: None,
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: &view,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color {
                    r: level,
                    g: level,
                    b: level,
                    a: 1.0,
                }),
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    context.queue().submit([encoder.finish()]);
}

/// Renders every frame with wgpu straight into the D3D11 surface, converts and
/// encodes it without the picture ever reaching system memory, and muxes it.
fn encode_with(name: &str, mut render: impl FnMut(&Harness, u32)) -> Option<PathBuf> {
    let mut harness = harness()?;
    let mut writer = Mp4Writer::new(VideoFormat {
        width: WIDTH as u16,
        height: HEIGHT as u16,
        timescale: FPS,
    });
    let mut avc = AvcConfig::default();
    let mut push = |writer: &mut Mp4Writer, samples: Vec<EncodedSample>| {
        for sample in samples {
            for unit in split_access_units(&sample.data) {
                let converted = annex_b_to_avcc(&unit);
                if !converted.config.is_empty() {
                    avc = converted.config;
                }
                if !converted.sample.is_empty() {
                    writer.push_sample(&converted.sample, 1, converted.is_sync);
                }
            }
        }
    };

    for index in 0..FRAMES {
        let value = index as u64 + 1;
        // Both halves of the handshake: wait until the conversion took the previous frame, then signal that this one landed.
        if index > 0 {
            harness
                .gpu_consumed
                .queue_wait(&harness.context, value - 1)
                .ok()?;
        }
        render(&harness, index);
        harness
            .gpu_drawn
            .queue_signal(&harness.context, value)
            .ok()?;

        harness.d3d.wait_for(&harness.drawn, value).ok()?;
        // A fresh frame each time: the encoder is asynchronous and still holds the last one.
        let frame = harness.nv12.frame().ok()?;
        harness
            .d3d
            .convert(&harness.surface, &harness.nv12, &frame)
            .ok()?;
        harness.d3d.signal(&harness.consumed, value).ok()?;

        let produced = harness
            .encoder
            .encode_texture(&frame, index as i64 * FRAME_DURATION, FRAME_DURATION)
            .ok()?;
        push(&mut writer, produced);
    }
    let tail = harness.encoder.finish().ok()?;
    push(&mut writer, tail);
    writer.set_avc_config(avc);

    let data = writer.finish()?;
    let dir = std::env::temp_dir().join("recast-codec-mf-tests");
    std::fs::create_dir_all(&dir).ok()?;
    let path = dir.join(name);
    std::fs::write(&path, &data).ok()?;
    Some(path)
}

fn bands_file() -> Option<PathBuf> {
    encode_with("gpu-bands.mp4", |h, _| {
        draw_bands(&h.context, h.imported.texture())
    })
}

fn alternating_file() -> Option<PathBuf> {
    encode_with("gpu-alternating.mp4", |h, index| {
        clear_to(&h.context, h.imported.texture(), frame_level(index))
    })
}

/// Mean luma of a column band on the middle row.
fn band(luma: &[u8], from: usize, to: usize) -> f64 {
    let row = (HEIGHT as usize / 2) * WIDTH as usize;
    let slice = &luma[row + from..row + to];
    slice.iter().map(|v| *v as f64).sum::<f64>() / slice.len() as f64
}

#[test]
fn a_frame_drawn_by_wgpu_decodes_back_to_the_colours_it_was_given() {
    let Some(path) = bands_file() else { return };
    let mut reader = VideoReader::open(&path).expect("the file opens");
    let frame = reader
        .next_frame()
        .expect("a read")
        .expect("at least one frame");

    // Sampled well clear of the boundaries, where chroma subsampling and the encoder both blur across the edge.
    let wanted = [
        ("red", 20, 60, expected_luma(1.0, 0.0, 0.0)),
        ("green", 100, 140, expected_luma(0.0, 1.0, 0.0)),
        ("dark grey", 180, 220, grey_luma(0.4)),
        ("light grey", 260, 300, grey_luma(0.9)),
    ];
    for (name, from, to, want) in wanted {
        let got = band(&frame.data, from, to);
        assert!(
            (got - want).abs() < 6.0,
            "the {name} band decoded to {got}, wanted about {want}"
        );
    }
}

#[test]
fn every_frame_survives_the_texture_path() {
    let Some(path) = bands_file() else { return };
    let mut reader = VideoReader::open(&path).expect("the file opens");
    let mut count = 0;
    while reader.next_frame().expect("a read").is_some() {
        count += 1;
    }
    assert_eq!(
        count, FRAMES as usize,
        "frames went missing on the GPU path"
    );
}

/// Each frame is drawn a different level, so a frame that reaches the encoder
/// before its draw lands arrives carrying the PREVIOUS one. Drawing the same
/// picture every frame could never show that.
#[test]
fn no_frame_arrives_carrying_the_one_before_it() {
    let Some(path) = alternating_file() else {
        return;
    };
    let mut reader = VideoReader::open(&path).expect("the file opens");
    let mut index = 0u32;
    while let Some(frame) = reader.next_frame().expect("a read") {
        let want = grey_luma(frame_level(index));
        let got = band(&frame.data, 40, 280);
        // Tighter than one step of the ramp, which makes a frame from the wrong place a failure rather than noise.
        assert!(
            (got - want).abs() < 3.0,
            "frame {index} decoded to {got}, wanted about {want}; \
             the frame before it wanted {}",
            grey_luma(frame_level(index.saturating_sub(1)))
        );
        index += 1;
    }
    assert_eq!(index, FRAMES, "frames went missing");
}

/// NV12 carries chroma at half resolution in each direction and has nowhere to
/// put an odd row, so the surface has to round down rather than hand the encoder
/// a size it cannot represent.
#[test]
fn an_odd_sized_nv12_surface_is_rounded_to_even() {
    let Ok(d3d) = D3dContext::new() else { return };
    let Ok(converter) = d3d.nv12_converter(WIDTH + 1, HEIGHT + 1, (FPS, 1)) else {
        return;
    };
    assert_eq!(converter.size(), (WIDTH, HEIGHT));
}

#[test]
fn an_encoder_opened_without_a_device_refuses_a_texture() {
    let Ok(d3d) = D3dContext::new() else { return };
    let Ok(converter) = d3d.nv12_converter(WIDTH, HEIGHT, (FPS, 1)) else {
        return;
    };
    let Ok(frame) = converter.frame() else { return };
    let config = EncodeConfig {
        width: WIDTH,
        height: HEIGHT,
        frame_rate: (FPS, 1),
        bitrate: 4_000_000,
        keyframe_interval: 0,
    };
    let found = enumerate_encoders();
    let Some(mut encoder) = ranked(&found, VideoCodec::H264)
        .into_iter()
        .find_map(|descriptor| H264Encoder::open(descriptor, config).ok())
    else {
        return;
    };
    assert!(!encoder.takes_textures());
    assert!(encoder.encode_texture(&frame, 0, FRAME_DURATION).is_err());
}
