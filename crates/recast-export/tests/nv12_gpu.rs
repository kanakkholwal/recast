//! The GPU converter against the CPU one it replaces. Byte-identical is the
//! bar, not "close": the export goldens compare encoded bytes.

use recast_compositor::SourceColor;
use recast_export::{rgba_to_nv12, GpuNv12};
use recast_gpu::{GpuContext, GpuOptions, OUTPUT_FORMAT};

fn context() -> Option<GpuContext> {
    GpuContext::new_blocking(GpuOptions {
        require_hardware: false,
        ..Default::default()
    })
    .ok()
}

/// A frame with structure rather than a flat fill: a constant colour would pass
/// even with the chroma averaging wrong.
fn frame(width: u32, height: u32) -> Vec<u8> {
    let mut rgba = Vec::with_capacity((width * height * 4) as usize);
    for y in 0..height {
        for x in 0..width {
            // Per-pixel variation inside every 2x2 block, so averaging is exercised.
            rgba.extend_from_slice(&[
                (x * 7 + y * 3) as u8,
                (x * 13 + y * 5) as u8,
                (x.wrapping_mul(29) ^ y.wrapping_mul(11)) as u8,
                255,
            ]);
        }
    }
    rgba
}

/// Uploads `rgba` as the compositor's output format.
fn texture(ctx: &GpuContext, rgba: &[u8], width: u32, height: u32) -> wgpu::Texture {
    let texture = ctx.device().create_texture(&wgpu::TextureDescriptor {
        label: Some("nv12-test"),
        size: wgpu::Extent3d {
            width,
            height,
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
        rgba,
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

#[test]
fn the_gpu_conversion_is_byte_identical_to_the_cpu_one() {
    let Some(ctx) = context() else { return };
    let color = SourceColor::default();
    let mut gpu = GpuNv12::new(ctx.device());

    for (width, height) in [(8, 4), (64, 32), (320, 180), (1280, 720)] {
        let rgba = frame(width, height);
        let texture = texture(&ctx, &rgba, width, height);

        let mut on_gpu = Vec::new();
        assert!(
            gpu.convert(ctx.device(), ctx.queue(), &texture, &color, &mut on_gpu),
            "{width}x{height} was refused by the GPU path"
        );
        let mut on_cpu = Vec::new();
        rgba_to_nv12(&mut on_cpu, &rgba, width, height, &color).expect("cpu converted");

        assert_eq!(on_gpu.len(), on_cpu.len(), "{width}x{height}: length");
        let differing = on_gpu.iter().zip(&on_cpu).filter(|(a, b)| a != b).count();
        assert_eq!(differing, 0, "{width}x{height}: {differing} bytes differ");
    }
}

/// The shader writes whole `u32`s, so a width that is not four pixels must be
/// refused rather than write past the row.
#[test]
fn a_shape_the_shader_cannot_pack_is_refused() {
    assert!(!GpuNv12::handles(6, 4));
    assert!(!GpuNv12::handles(8, 3));
    assert!(!GpuNv12::handles(2, 2));
    assert!(GpuNv12::handles(8, 4));
}

#[test]
fn an_odd_shape_falls_back_instead_of_converting() {
    let Some(ctx) = context() else { return };
    let (width, height) = (6u32, 4u32);
    let rgba = frame(width, height);
    let texture = texture(&ctx, &rgba, width, height);

    let mut gpu = GpuNv12::new(ctx.device());
    let mut out = vec![0xAB];
    assert!(!gpu.convert(
        ctx.device(),
        ctx.queue(),
        &texture,
        &SourceColor::default(),
        &mut out
    ));
    assert_eq!(out, vec![0xAB], "a refusal must leave the buffer alone");
}

/// A steady export must not allocate per frame, which is what the CPU path's
/// churn cost. Size alone cannot prove reuse, so the counter does.
#[test]
fn a_steady_size_allocates_its_buffers_once() {
    let Some(ctx) = context() else { return };
    let (width, height) = (64u32, 32u32);
    let rgba = frame(width, height);
    let texture = texture(&ctx, &rgba, width, height);

    let mut gpu = GpuNv12::new(ctx.device());
    let mut out = Vec::new();
    for _ in 0..8 {
        assert!(gpu.convert(
            ctx.device(),
            ctx.queue(),
            &texture,
            &SourceColor::default(),
            &mut out
        ));
    }
    assert_eq!(
        gpu.allocations(),
        1,
        "the buffers were reallocated per frame"
    );
}
