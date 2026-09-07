//! RGBA to NV12 on the GPU, where the frame already is.
//!
//! The CPU converter is 74% of an export frame at 1080p (8.90 ms of 12.05 ms),
//! and it also forces the readback to carry 4 bytes a pixel instead of 1.5.

use recast_compositor::{encode_matrix, PlaneLayout, SourceColor};
use recast_gpu::OUTPUT_FORMAT;
use wgpu::util::DeviceExt;

/// Fixed-point fractional bits, matching the CPU encoder exactly. Anything else
/// would move a code value and break the byte-identical goldens.
const FRACTION_BITS: u32 = 16;

/// Blocks per workgroup edge, matching `@workgroup_size(8, 8, 1)`.
const WORKGROUP: u32 = 8;

/// The uniform the shader reads, laid out to its `Params`.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Params {
    row0: [i32; 4],
    row1: [i32; 4],
    row2: [i32; 4],
    /// x, y, z are the channel offsets; w is the width in pixels.
    offsets: [i32; 4],
    /// x is the height; the rest pads to 16 bytes.
    size: [u32; 4],
}

/// Converts the compositor's output texture straight into packed NV12.
pub struct GpuNv12 {
    pipeline: wgpu::ComputePipeline,
    layout: wgpu::BindGroupLayout,
    /// Sized by the packed NV12 length, so one entry covers a whole export.
    storage: Option<(u64, wgpu::Buffer)>,
    download: Option<(u64, wgpu::Buffer)>,
    allocations: u64,
}

impl GpuNv12 {
    #[must_use]
    pub fn new(device: &wgpu::Device) -> Self {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("recast-nv12"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/nv12.wgsl").into()),
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("recast-nv12"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("recast-nv12"),
            bind_group_layouts: &[Some(&layout)],
            ..Default::default()
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("recast-nv12"),
            layout: Some(&pipeline_layout),
            module: &module,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });
        Self {
            pipeline,
            layout,
            storage: None,
            download: None,
            allocations: 0,
        }
    }

    /// Whether this shape takes the GPU path.
    ///
    /// The shader writes whole `u32`s, so four luma bytes have to be one row's
    /// worth and the block has to be two rows tall. An odd shape falls back to
    /// the CPU rather than growing a slow read-modify-write path for it.
    #[must_use]
    pub fn handles(width: u32, height: u32) -> bool {
        width >= 4 && height >= 2 && width.is_multiple_of(4) && height.is_multiple_of(2)
    }

    /// Buffers allocated. A steady loop over one output size must not grow this
    /// past one per buffer, which size alone cannot prove.
    #[must_use]
    pub fn allocations(&self) -> u64 {
        self.allocations
    }

    fn params(color: &SourceColor, width: u32, height: u32) -> Params {
        let (matrix, bias) = encode_matrix(color);
        let scale = (1i64 << FRACTION_BITS) as f32;
        let row = |r: [f32; 3]| {
            [
                (r[0] * scale).round() as i32,
                (r[1] * scale).round() as i32,
                (r[2] * scale).round() as i32,
                0,
            ]
        };
        Params {
            row0: row(matrix[0]),
            row1: row(matrix[1]),
            row2: row(matrix[2]),
            offsets: [
                (bias[0] * 255.0 * scale).round() as i32,
                (bias[1] * 255.0 * scale).round() as i32,
                (bias[2] * 255.0 * scale).round() as i32,
                width as i32,
            ],
            size: [height, 0, 0, 0],
        }
    }

    fn buffer(&mut self, device: &wgpu::Device, needed: u64) -> bool {
        let fresh = !matches!(&self.storage, Some((len, _)) if *len == needed);
        if fresh {
            self.allocations += 1;
            self.storage = Some((
                needed,
                device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("recast-nv12-storage"),
                    size: needed,
                    usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
                    mapped_at_creation: false,
                }),
            ));
            self.download = Some((
                needed,
                device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("recast-nv12-download"),
                    size: needed,
                    usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                    mapped_at_creation: false,
                }),
            ));
        }
        fresh
    }

    /// Converts `texture` into packed NV12, replacing `out`. `false` when the
    /// shape is not one the shader handles, which leaves `out` untouched.
    pub fn convert(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        texture: &wgpu::Texture,
        color: &SourceColor,
        out: &mut Vec<u8>,
    ) -> bool {
        let (width, height) = (texture.width(), texture.height());
        if texture.format() != OUTPUT_FORMAT || !Self::handles(width, height) {
            return false;
        }
        let needed = PlaneLayout::Nv12.packed_len(width, height) as u64;
        self.buffer(device, needed);
        let (Some((_, storage)), Some((_, download))) = (&self.storage, &self.download) else {
            unreachable!("the buffers were just created")
        };

        let params = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("recast-nv12-params"),
            contents: bytemuck::bytes_of(&Self::params(color, width, height)),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("recast-nv12"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: storage.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: params.as_entire_binding(),
                },
            ],
        });

        let mut encoder =
            device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("recast-nv12"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &bind, &[]);
            pass.dispatch_workgroups(
                (width / 4).div_ceil(WORKGROUP),
                (height / 2).div_ceil(WORKGROUP),
                1,
            );
        }
        encoder.copy_buffer_to_buffer(storage, 0, download, 0, needed);
        queue.submit([encoder.finish()]);

        let slice = download.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        let _ = device.poll(wgpu::PollType::wait_indefinitely());
        out.clear();
        let Ok(mapped) = slice.get_mapped_range() else {
            // The buffer is reused, so a mapped one left behind makes the NEXT frame's map_async a validation error rather than a fallback.
            download.unmap();
            return false;
        };
        out.extend_from_slice(&mapped);
        drop(mapped);
        download.unmap();
        true
    }
}

impl std::fmt::Debug for GpuNv12 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GpuNv12")
            .field("allocations", &self.allocations)
            .finish_non_exhaustive()
    }
}
