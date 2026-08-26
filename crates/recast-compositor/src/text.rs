use bytemuck::{Pod, Zeroable};
use recast_gpu::WORKING_FORMAT;
use recast_text::GlyphAtlas;

use crate::render::{clamped_linear_sampler, PREMULTIPLIED};

/// One glyph quad in canvas pixels. The layout module produces these and the
/// pass only draws them, so a caption, a title or a watermark all share it.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct GlyphQuad {
    /// x, y, width, height in canvas px, y down.
    pub rect: [f32; 4],
    /// u0, v0, u1, v1 in atlas uv.
    pub uv: [f32; 4],
    /// sRGB rgb with the master alpha in `a`.
    pub colour: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct CanvasUniform {
    size: [f32; 4],
}

struct AtlasTexture {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    width: u32,
    height: u32,
    generation: u64,
}

pub(crate) struct TextPass {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    uniform: wgpu::Buffer,
    atlas: Option<AtlasTexture>,
}

impl TextPass {
    pub(crate) fn new(device: &wgpu::Device) -> Self {
        // Its own layout rather than `sampled_texture_layout`: the canvas size
        // is read by the vertex stage, which that one does not make visible.
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("text"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("text"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/text.wgsl").into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("text"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        // An instance buffer rather than a storage buffer: WebGL2 has no storage
        // buffers, and instanced arrays are core ES3.
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("text"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs"),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<GlyphQuad>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &wgpu::vertex_attr_array![
                        0 => Float32x4,
                        1 => Float32x4,
                        2 => Float32x4
                    ],
                })],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: WORKING_FORMAT,
                    blend: Some(PREMULTIPLIED),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        Self {
            pipeline,
            layout,
            sampler: clamped_linear_sampler(device, "text"),
            uniform: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("text-canvas"),
                size: std::mem::size_of::<CanvasUniform>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            atlas: None,
        }
    }

    /// Mirrors the CPU atlas onto the GPU, uploading only the rows that changed
    /// unless the generation moved, which means the buffer itself was replaced.
    pub(crate) fn sync(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        atlas: &mut GlyphAtlas,
    ) {
        let (width, height) = atlas.size();
        let stale = match &self.atlas {
            Some(current) => {
                current.width != width
                    || current.height != height
                    || current.generation != atlas.generation()
            }
            None => true,
        };
        if stale {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("glyph-atlas"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::R8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            let view = texture.create_view(&Default::default());
            self.atlas = Some(AtlasTexture {
                texture,
                view,
                width,
                height,
                generation: atlas.generation(),
            });
            atlas.take_dirty();
            self.upload(queue, atlas.coverage(), 0, height, width);
            return;
        }
        if let Some((from, to)) = atlas.take_dirty() {
            let to = to.min(height);
            if to > from {
                let start = (from * width) as usize;
                let end = (to * width) as usize;
                self.upload(queue, &atlas.coverage()[start..end], from, to - from, width);
            }
        }
    }

    fn upload(&self, queue: &wgpu::Queue, rows: &[u8], y: u32, height: u32, width: u32) {
        let Some(atlas) = &self.atlas else { return };
        if height == 0 || rows.is_empty() {
            return;
        }
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &atlas.texture,
                mip_level: 0,
                origin: wgpu::Origin3d { x: 0, y, z: 0 },
                aspect: wgpu::TextureAspect::All,
            },
            rows,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
    }

    pub(crate) fn draw(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        working: &wgpu::TextureView,
        quads: &[GlyphQuad],
        canvas: (u32, u32),
    ) {
        let Some(atlas) = &self.atlas else { return };
        if quads.is_empty() {
            return;
        }
        queue.write_buffer(
            &self.uniform,
            0,
            bytemuck::bytes_of(&CanvasUniform {
                size: [canvas.0 as f32, canvas.1 as f32, 0.0, 0.0],
            }),
        );
        let instances = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("text-instances"),
            size: std::mem::size_of_val(quads) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&instances, 0, bytemuck::cast_slice(quads));

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("text"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&atlas.view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("text"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: working,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.set_vertex_buffer(0, instances.slice(..));
        pass.draw(0..6, 0..quads.len() as u32);
    }
}
