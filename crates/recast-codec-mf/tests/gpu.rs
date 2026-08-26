#![cfg(windows)]

use std::path::PathBuf;

use recast_codec::{ranked, VideoCodec};
use recast_codec_mf::{
    enumerate_encoders, D3dContext, EncodeConfig, H264Encoder, SyncFence, VideoReader,
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

/// Left half red, right half green. Two flat regions with very different luma,
/// so a flipped image, a wrong stride and a wrong colour matrix are each visible
/// in the decoded picture.
const SHADER: &str = r#"
@vertex
fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    var p = array<vec2<f32>, 3>(vec2(-1.0, -3.0), vec2(-1.0, 1.0), vec2(3.0, 1.0));
    return vec4<f32>(p[i], 0.0, 1.0);
}

@fragment
fn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    if pos.x < 160.0 {
        return vec4<f32>(1.0, 0.0, 0.0, 1.0);
    }
    return vec4<f32>(0.0, 1.0, 0.0, 1.0);
}
"#;

/// BT.709 studio range: `Y = 16 + 219 * (0.2126 R + 0.7152 G + 0.0722 B)`.
fn expected_luma(r: f64, g: f64, b: f64) -> f64 {
    16.0 + 219.0 * (0.2126 * r + 0.7152 * g + 0.0722 * b)
}

fn draw(context: &GpuContext, target: &wgpu::Texture) {
    let device = context.device();
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: None,
        source: wgpu::ShaderSource::Wgsl(SHADER.into()),
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

/// Renders every frame with wgpu straight into a D3D11 surface, converts and
/// encodes it without the picture ever reaching system memory, and muxes it.
fn encoded_on_gpu() -> Option<PathBuf> {
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
    let fence: SyncFence = d3d.shared_fence().ok()?;
    let imported = import_shared_texture(
        &context,
        SharedHandle(surface.duplicate_handle().ok()?),
        SharedTextureDesc::new(WIDTH, HEIGHT, SharedFormat::Bgra8Unorm).as_render_target(),
    )
    .ok()?;
    let gpu_fence =
        import_shared_fence(&context, SharedHandle(fence.duplicate_handle().ok()?)).ok()?;
    let nv12 = d3d.nv12_surface(WIDTH, HEIGHT, (FPS, 1)).ok()?;

    let config = EncodeConfig {
        width: WIDTH,
        height: HEIGHT,
        frame_rate: (FPS, 1),
        bitrate: 8_000_000,
    };
    let found = enumerate_encoders();
    let mut encoder = None;
    for descriptor in ranked(&found, VideoCodec::H264) {
        match H264Encoder::open_with_gpu(descriptor, config, &d3d) {
            Ok(open) if open.takes_textures() => {
                eprintln!("encoding textures with {}", descriptor.name);
                encoder = Some(open);
                break;
            }
            Ok(_) => continue,
            Err(_) => continue,
        }
    }
    let mut encoder = encoder.or_else(|| {
        eprintln!("skipping: no encoder took a D3D11 device");
        None
    })?;

    let mut writer = Mp4Writer::new(VideoFormat {
        width: WIDTH as u16,
        height: HEIGHT as u16,
        timescale: FPS,
    });
    let mut avc = AvcConfig::default();
    let mut push = |writer: &mut Mp4Writer, samples: Vec<recast_codec_mf::EncodedSample>| {
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
        draw(&context, imported.texture());
        // The signal is enqueued after the submit, and the wait before the
        // conversion, so the encoder cannot read a half-drawn surface. Without
        // this pair the picture comes back as whatever the surface held before,
        // and nothing reports an error.
        let value = index as u64 + 1;
        gpu_fence.queue_signal(&context, value).ok()?;
        d3d.wait_for(&fence, value).ok()?;
        d3d.convert(&surface, &nv12).ok()?;
        let produced = encoder
            .encode_texture(&nv12, index as i64 * FRAME_DURATION, FRAME_DURATION)
            .ok()?;
        push(&mut writer, produced);
    }
    let tail = encoder.finish().ok()?;
    push(&mut writer, tail);
    writer.set_avc_config(avc);

    let data = writer.finish()?;
    let dir = std::env::temp_dir().join("recast-codec-mf-tests");
    std::fs::create_dir_all(&dir).ok()?;
    let path = dir.join("gpu.mp4");
    std::fs::write(&path, &data).ok()?;
    Some(path)
}

/// Mean luma of a column band on the middle row.
fn band(luma: &[u8], from: usize, to: usize) -> f64 {
    let row = (HEIGHT as usize / 2) * WIDTH as usize;
    let slice = &luma[row + from..row + to];
    slice.iter().map(|v| *v as f64).sum::<f64>() / slice.len() as f64
}

#[test]
fn a_frame_drawn_by_wgpu_encodes_and_decodes_back_to_its_colours() {
    let Some(path) = encoded_on_gpu() else { return };
    let mut reader = VideoReader::open(&path).expect("the file opens");
    let frame = reader
        .next_frame()
        .expect("a read")
        .expect("at least one frame");

    // Well clear of the boundary, where chroma subsampling and the encoder both
    // blur across the edge.
    let left = band(&frame.data, 20, 140);
    let right = band(&frame.data, 180, 300);
    let (want_left, want_right) = (
        expected_luma(1.0, 0.0, 0.0),
        expected_luma(0.0, 1.0, 0.0),
    );
    assert!(
        (left - want_left).abs() < 6.0,
        "the red half decoded to {left}, wanted about {want_left}"
    );
    assert!(
        (right - want_right).abs() < 6.0,
        "the green half decoded to {right}, wanted about {want_right}"
    );
}

#[test]
fn every_frame_survives_the_texture_path() {
    let Some(path) = encoded_on_gpu() else { return };
    let mut reader = VideoReader::open(&path).expect("the file opens");
    let mut count = 0;
    while reader.next_frame().expect("a read").is_some() {
        count += 1;
    }
    assert_eq!(count, FRAMES as usize, "frames went missing on the GPU path");
}

/// The picture is drawn once per frame, so the left half must stay red for all
/// of them. A missing fence shows up here as a frame that is still black.
#[test]
fn no_frame_comes_back_as_the_surface_before_it_was_drawn() {
    let Some(path) = encoded_on_gpu() else { return };
    let mut reader = VideoReader::open(&path).expect("the file opens");
    let want = expected_luma(1.0, 0.0, 0.0);
    let mut index = 0;
    while let Some(frame) = reader.next_frame().expect("a read") {
        let left = band(&frame.data, 20, 140);
        assert!(
            (left - want).abs() < 6.0,
            "frame {index} read {left} on the left, wanted about {want}"
        );
        index += 1;
    }
    assert!(index > 0, "nothing decoded");
}

#[test]
fn an_encoder_opened_without_a_device_refuses_a_texture() {
    let Ok(d3d) = D3dContext::new() else { return };
    let nv12 = match d3d.nv12_surface(WIDTH, HEIGHT, (FPS, 1)) {
        Ok(surface) => surface,
        Err(_) => return,
    };
    let config = EncodeConfig {
        width: WIDTH,
        height: HEIGHT,
        frame_rate: (FPS, 1),
        bitrate: 4_000_000,
    };
    let found = enumerate_encoders();
    let Some(mut encoder) = ranked(&found, VideoCodec::H264)
        .into_iter()
        .find_map(|descriptor| H264Encoder::open(descriptor, config).ok())
    else {
        return;
    };
    assert!(!encoder.takes_textures());
    assert!(encoder.encode_texture(&nv12, 0, FRAME_DURATION).is_err());
}
