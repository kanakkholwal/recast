use std::collections::HashMap;

use bytemuck::{Pod, Zeroable};
use recast_color::Srgba;
use recast_gpu::{GpuContext, GpuError, OUTPUT_FORMAT, WORKING_FORMAT};
use recast_scene::LayerId;

use crate::eval::{BackgroundParams, FrameParams, LayerParams};

const MAX_GRADIENT_STOPS: usize = 8;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct BackgroundUniform {
    header: [f32; 4],
    solid: [f32; 4],
    stops: [[f32; 4]; MAX_GRADIENT_STOPS],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct CardUniform {
    rect: [f32; 4],
    canvas: [f32; 4],
    affine_a: [f32; 4],
    affine_b: [f32; 4],
    flags: [f32; 4],
}

/// Decoded source frames for one instant, addressed by layer.
#[derive(Default)]
pub struct FrameInputs<'a> {
    views: HashMap<LayerId, LayerInput<'a>>,
}

pub struct LayerInput<'a> {
    pub view: &'a wgpu::TextureView,
    /// True when sampling the texture returns sRGB-encoded values the shader
    /// must decode itself, which is every non-`*Srgb` 8-bit format.
    pub needs_srgb_decode: bool,
}

impl<'a> FrameInputs<'a> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(&mut self, id: LayerId, input: LayerInput<'a>) -> &mut Self {
        self.views.insert(id, input);
        self
    }

    pub fn get(&self, id: LayerId) -> Option<&LayerInput<'a>> {
        self.views.get(&id)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RenderStats {
    pub layers_drawn: u32,
    pub layers_skipped: u32,
}

pub struct Compositor {
    device: wgpu::Device,
    queue: wgpu::Queue,
    background: BackgroundPass,
    card: CardPass,
    present: PresentPass,
    working: Option<(u32, u32, wgpu::Texture)>,
}

struct BackgroundPass {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    uniform: wgpu::Buffer,
}

struct CardPass {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
}

struct PresentPass {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
}

impl Compositor {
    pub fn new(ctx: &GpuContext) -> Result<Self, GpuError> {
        let device = ctx.device().clone();
        let queue = ctx.queue().clone();
        Ok(Self {
            background: BackgroundPass::new(&device),
            card: CardPass::new(&device),
            present: PresentPass::new(&device),
            device,
            queue,
            working: None,
        })
    }

    /// Composites into the linear working target and presents it to `target`,
    /// which must be `OUTPUT_FORMAT`.
    pub fn render(
        &mut self,
        params: &FrameParams,
        inputs: &FrameInputs<'_>,
        target: &wgpu::TextureView,
    ) -> RenderStats {
        let width = params.geometry.canvas_w.max(1);
        let height = params.geometry.canvas_h.max(1);
        let working_view = self.working_view(width, height);

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("recast-frame"),
            });

        self.draw_background(&mut encoder, &working_view, params, width, height);
        let stats = self.draw_layers(&mut encoder, &working_view, params, inputs, width, height);
        self.present(&mut encoder, &working_view, target);

        self.queue.submit([encoder.finish()]);
        stats
    }

    fn working_view(&mut self, width: u32, height: u32) -> wgpu::TextureView {
        let reuse = matches!(&self.working, Some((w, h, _)) if *w == width && *h == height);
        if !reuse {
            let texture = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("recast-working"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: WORKING_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            self.working = Some((width, height, texture));
        }
        match &self.working {
            Some((_, _, texture)) => texture.create_view(&Default::default()),
            None => unreachable!("the working texture was just created"),
        }
    }

    fn draw_background(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        working: &wgpu::TextureView,
        params: &FrameParams,
        width: u32,
        height: u32,
    ) {
        let uniform = background_uniform(&params.background, width, height);
        self.queue
            .write_buffer(&self.background.uniform, 0, bytemuck::bytes_of(&uniform));

        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("background"),
            layout: &self.background.layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: self.background.uniform.as_entire_binding(),
            }],
        });

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("background"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: working,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.background.pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.draw(0..3, 0..1);
    }

    fn draw_layers(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        working: &wgpu::TextureView,
        params: &FrameParams,
        inputs: &FrameInputs<'_>,
        width: u32,
        height: u32,
    ) -> RenderStats {
        let mut stats = RenderStats::default();
        let mut uniforms = Vec::new();
        let mut bind_groups = Vec::new();

        for layer in &params.layers {
            let Some(input) = inputs.get(layer.id) else {
                stats.layers_skipped += 1;
                continue;
            };
            if !layer.visible || layer.opacity <= 0.0 {
                stats.layers_skipped += 1;
                continue;
            }

            let uniform = card_uniform(layer, params, width, height, input.needs_srgb_decode);
            let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("card-uniform"),
                size: std::mem::size_of::<CardUniform>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.queue
                .write_buffer(&buffer, 0, bytemuck::bytes_of(&uniform));

            bind_groups.push(self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("card"),
                layout: &self.card.layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(input.view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Sampler(&self.card.sampler),
                    },
                ],
            }));
            uniforms.push(buffer);
            stats.layers_drawn += 1;
        }

        if bind_groups.is_empty() {
            return stats;
        }

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("layers"),
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
        pass.set_pipeline(&self.card.pipeline);
        for bind_group in &bind_groups {
            pass.set_bind_group(0, bind_group, &[]);
            pass.draw(0..6, 0..1);
        }
        stats
    }

    fn present(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        working: &wgpu::TextureView,
        target: &wgpu::TextureView,
    ) {
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("present"),
            layout: &self.present.layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(working),
            }],
        });

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("present"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.present.pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.draw(0..3, 0..1);
    }

    pub fn output_texture(&self, width: u32, height: u32) -> wgpu::Texture {
        self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("recast-output"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: OUTPUT_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        })
    }
}

fn linear_rgba(color: Srgba) -> [f32; 4] {
    [
        color.r as f32 / 255.0,
        color.g as f32 / 255.0,
        color.b as f32 / 255.0,
        color.alpha_f32(),
    ]
}

fn background_uniform(background: &BackgroundParams, width: u32, height: u32) -> BackgroundUniform {
    let mut uniform = BackgroundUniform {
        header: [0.0, 0.0, width as f32, height as f32],
        solid: [0.0, 0.0, 0.0, 1.0],
        stops: [[0.0; 4]; MAX_GRADIENT_STOPS],
    };

    match background {
        BackgroundParams::Solid(color) => {
            uniform.solid = linear_rgba(*color);
        }
        BackgroundParams::Gradient(gradient) => {
            let count = gradient.stops.len().min(MAX_GRADIENT_STOPS);
            uniform.header[0] = count as f32;
            uniform.header[1] = (gradient.angle.to_radians()) as f32;
            for (slot, stop) in uniform.stops.iter_mut().zip(&gradient.stops).take(count) {
                *slot = [
                    stop.color.r as f32 / 255.0,
                    stop.color.g as f32 / 255.0,
                    stop.color.b as f32 / 255.0,
                    (stop.pos / 100.0) as f32,
                ];
            }
        }
        // An asset background needs its image decoded and uploaded, which the
        // media layer owns. Until that lands it renders as the fallback grey
        // rather than as an undefined surface.
        BackgroundParams::Asset { .. } => {
            uniform.solid = linear_rgba(Srgba::opaque(0x11, 0x11, 0x11));
        }
    }
    uniform
}

fn card_uniform(
    layer: &LayerParams,
    params: &FrameParams,
    width: u32,
    height: u32,
    needs_srgb_decode: bool,
) -> CardUniform {
    let g = params.geometry;
    let t = layer.transform;
    CardUniform {
        rect: [
            g.video_x as f32,
            g.video_y as f32,
            g.video_w as f32,
            g.video_h as f32,
        ],
        canvas: [width as f32, height as f32, 0.0, 0.0],
        affine_a: [t.sx, t.shx, t.tx, t.shy],
        affine_b: [t.sy, t.ty, layer.opacity, layer.corner_radius],
        flags: [if needs_srgb_decode { 1.0 } else { 0.0 }, 0.0, 0.0, 0.0],
    }
}

fn fullscreen_pipeline(
    device: &wgpu::Device,
    label: &str,
    source: &str,
    layout: &wgpu::BindGroupLayout,
    format: wgpu::TextureFormat,
    blend: Option<wgpu::BlendState>,
    vertex_entry: &str,
) -> wgpu::RenderPipeline {
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(label),
        source: wgpu::ShaderSource::Wgsl(source.into()),
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some(label),
        bind_group_layouts: &[Some(layout)],
        immediate_size: 0,
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &module,
            entry_point: Some(vertex_entry),
            buffers: &[],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &module,
            entry_point: Some("fs"),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend,
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: Default::default(),
        depth_stencil: None,
        multisample: Default::default(),
        multiview_mask: None,
        cache: None,
    })
}

/// Sources arrive premultiplied from the card pass, so the blend is
/// `ONE, ONE_MINUS_SRC_ALPHA` rather than the usual `SRC_ALPHA` form.
const PREMULTIPLIED: wgpu::BlendState = wgpu::BlendState {
    color: wgpu::BlendComponent {
        src_factor: wgpu::BlendFactor::One,
        dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
        operation: wgpu::BlendOperation::Add,
    },
    alpha: wgpu::BlendComponent {
        src_factor: wgpu::BlendFactor::One,
        dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
        operation: wgpu::BlendOperation::Add,
    },
};

impl BackgroundPass {
    fn new(device: &wgpu::Device) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("background"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let pipeline = fullscreen_pipeline(
            device,
            "background",
            include_str!("shaders/background.wgsl"),
            &layout,
            WORKING_FORMAT,
            None,
            "vs",
        );
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("background-uniform"),
            size: std::mem::size_of::<BackgroundUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self {
            pipeline,
            layout,
            uniform,
        }
    }
}

impl CardPass {
    fn new(device: &wgpu::Device) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("card"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
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
        let pipeline = fullscreen_pipeline(
            device,
            "card",
            include_str!("shaders/card.wgsl"),
            &layout,
            WORKING_FORMAT,
            Some(PREMULTIPLIED),
            "vs",
        );
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("card"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });
        Self {
            pipeline,
            layout,
            sampler,
        }
    }
}

impl PresentPass {
    fn new(device: &wgpu::Device) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("present"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            }],
        });
        let pipeline = fullscreen_pipeline(
            device,
            "present",
            include_str!("shaders/present.wgsl"),
            &layout,
            OUTPUT_FORMAT,
            None,
            "vs",
        );
        Self { pipeline, layout }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use recast_color::{parse_gradient, Srgba};

    #[test]
    fn a_solid_background_carries_its_colour_and_no_stops() {
        let u = background_uniform(
            &BackgroundParams::Solid(Srgba::opaque(255, 128, 0)),
            100,
            50,
        );
        assert_eq!(u.header[0], 0.0);
        assert_eq!(u.header[2], 100.0);
        assert_eq!(u.header[3], 50.0);
        assert!((u.solid[0] - 1.0).abs() < 1e-6);
        assert!((u.solid[1] - 128.0 / 255.0).abs() < 1e-6);
    }

    #[test]
    fn a_gradient_carries_its_stops_and_angle_in_radians() {
        let gradient = parse_gradient("linear-gradient(90deg, #ff0000 0%, #0000ff 100%)");
        let u = background_uniform(&BackgroundParams::Gradient(Box::new(gradient)), 10, 10);
        assert_eq!(u.header[0], 2.0);
        assert!((u.header[1] - std::f32::consts::FRAC_PI_2).abs() < 1e-6);
        assert_eq!(u.stops[0][3], 0.0);
        assert_eq!(u.stops[1][3], 1.0);
        assert!((u.stops[0][0] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn a_gradient_with_more_stops_than_the_uniform_holds_is_truncated_not_overrun() {
        let stops: Vec<String> = (0..12)
            .map(|i| format!("#0000{:02x} {}%", i * 20, i * 9))
            .collect();
        let gradient = parse_gradient(&format!("linear-gradient(0deg, {})", stops.join(", ")));
        let u = background_uniform(&BackgroundParams::Gradient(Box::new(gradient)), 10, 10);
        assert_eq!(u.header[0], MAX_GRADIENT_STOPS as f32);
    }

    #[test]
    fn an_asset_background_renders_the_fallback_rather_than_an_undefined_surface() {
        let u = background_uniform(
            &BackgroundParams::Asset {
                kind: "wallpaper".into(),
                value: "C:/nope.jpg".into(),
            },
            10,
            10,
        );
        assert_eq!(u.header[0], 0.0);
        assert!((u.solid[0] - 17.0 / 255.0).abs() < 1e-6);
    }

    #[test]
    fn the_uniforms_match_the_std140_sizes_the_shaders_declare() {
        assert_eq!(std::mem::size_of::<BackgroundUniform>(), 16 + 16 + 16 * 8);
        assert_eq!(std::mem::size_of::<CardUniform>(), 16 * 5);
    }
}
