use crate::format::aligned_bytes_per_row;

/// Copies rendered frames back to system memory, reusing its staging buffer: a
/// buffer per frame is a canvas-sized allocation, 2 GB/s of churn at 4K60.
#[derive(Debug, Default)]
pub struct Readback {
    /// Keyed by byte length: the padded row stride makes size the only thing
    /// that matters, not the dimensions it came from.
    buffer: Option<(u64, wgpu::Buffer)>,
    allocations: u64,
}

impl Readback {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Reads `texture` into `out` as tightly packed RGBA, replacing it. The
    /// padding `copy_texture_to_buffer` needs is stripped, so no caller sees it.
    pub fn read(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        texture: &wgpu::Texture,
        out: &mut Vec<u8>,
    ) {
        let (width, height) = (texture.width(), texture.height());
        let bytes_per_row = aligned_bytes_per_row(width, texture.format());
        let needed = u64::from(bytes_per_row) * u64::from(height);
        let reuse = matches!(&self.buffer, Some((len, _)) if *len == needed);
        if !reuse {
            self.allocations += 1;
            self.buffer = Some((
                needed,
                device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("recast-readback"),
                    size: needed,
                    usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                    mapped_at_creation: false,
                }),
            ));
        }
        let Some((_, buffer)) = &self.buffer else {
            unreachable!("the readback buffer was just created")
        };

        let mut encoder =
            device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer,
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
        queue.submit([encoder.finish()]);

        let slice = buffer.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        // The export is a batch job: a blocking wait beats a callback dance.
        let _ = device.poll(wgpu::PollType::wait_indefinitely());
        out.clear();
        if let Ok(mapped) = slice.get_mapped_range() {
            let row = width as usize * 4;
            out.reserve(row * height as usize);
            for y in 0..height as usize {
                let start = y * bytes_per_row as usize;
                out.extend_from_slice(&mapped[start..start + row]);
            }
        }
        buffer.unmap();
    }

    /// Staging buffers allocated. Size alone cannot show reuse: a reallocated
    /// buffer is the same size as the one it replaced.
    #[must_use]
    pub fn allocations(&self) -> u64 {
        self.allocations
    }

    #[must_use]
    pub fn buffer_bytes(&self) -> u64 {
        self.buffer.as_ref().map_or(0, |(len, _)| *len)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{GpuContext, GpuOptions, OUTPUT_FORMAT};

    fn context() -> Option<GpuContext> {
        GpuContext::new_blocking(GpuOptions {
            require_hardware: false,
            ..Default::default()
        })
        .ok()
    }

    /// 3 pixels of RGBA is 12 bytes, which `copy_texture_to_buffer` pads to
    /// 256. A reader that hands back the padding corrupts every frame.
    fn filled(ctx: &GpuContext, width: u32, height: u32, value: u8) -> wgpu::Texture {
        let texture = ctx.device().create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: OUTPUT_FORMAT,
            usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let rows = vec![value; width as usize * height as usize * 4];
        ctx.queue().write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &rows,
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
    fn a_frame_comes_back_tightly_packed_with_the_row_padding_stripped() {
        let Some(ctx) = context() else { return };
        let texture = filled(&ctx, 3, 2, 0x7B);
        let mut reader = Readback::new();
        let mut out = Vec::new();
        reader.read(ctx.device(), ctx.queue(), &texture, &mut out);
        assert_eq!(out.len(), 3 * 2 * 4, "padding leaked into the result");
        assert!(out.iter().all(|&b| b == 0x7B), "wrong pixels: {out:?}");
    }

    /// The bug this guards: a staging buffer per frame is a canvas-sized
    /// allocation every frame, which at 4K60 is gigabytes a second.
    #[test]
    fn a_steady_loop_allocates_one_staging_buffer() {
        let Some(ctx) = context() else { return };
        let texture = filled(&ctx, 4, 4, 0x11);
        let mut reader = Readback::new();
        let mut out = Vec::new();
        for _ in 0..20 {
            reader.read(ctx.device(), ctx.queue(), &texture, &mut out);
        }
        assert_eq!(reader.allocations(), 1);
        assert_eq!(out.len(), 4 * 4 * 4);
    }

    #[test]
    fn a_different_size_gets_a_buffer_that_fits_it() {
        let Some(ctx) = context() else { return };
        let mut reader = Readback::new();
        let mut out = Vec::new();
        reader.read(ctx.device(), ctx.queue(), &filled(&ctx, 4, 4, 1), &mut out);
        let small = reader.buffer_bytes();
        reader.read(
            ctx.device(),
            ctx.queue(),
            &filled(&ctx, 64, 64, 2),
            &mut out,
        );
        assert!(reader.buffer_bytes() > small);
        assert_eq!(reader.allocations(), 2);
        assert_eq!(out.len(), 64 * 64 * 4);
    }

    #[test]
    fn reading_replaces_the_output_rather_than_appending_to_it() {
        let Some(ctx) = context() else { return };
        let mut reader = Readback::new();
        let mut out = vec![0xFF; 999];
        reader.read(ctx.device(), ctx.queue(), &filled(&ctx, 2, 2, 5), &mut out);
        assert_eq!(out.len(), 2 * 2 * 4);
    }
}
