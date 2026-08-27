use std::collections::HashMap;

use bytemuck::{Pod, Zeroable};
use recast_color::Srgba;
use recast_gpu::{GpuContext, GpuError, OUTPUT_FORMAT, WORKING_FORMAT};
use recast_scene::LayerId;

use crate::annotation::{AnnotationParams, AnnotationShape};
use crate::caption::CaptionFrame;
use crate::eval::{
    BackgroundParams, CursorDraw, CursorSlot, FrameParams, LayerParams, ShadowParams,
};
use crate::text::TextPass;
use crate::yuv::{planes_of, yuv_uniform, PlaneLayout, SourcePlanes, YuvError, YuvUniform};

const MAX_GRADIENT_STOPS: usize = 8;

/// Taps per side of the separable Gaussian. Beyond this the stride grows
/// instead, which trades a little aliasing for a bounded loop.
const MAX_BLUR_TAPS: f32 = 24.0;

/// The authored 0..100 slider in canvas pixels of sigma, matching what the
/// WebGL preview has always shown (100 lands at 24 px).
const BLUR_PX_PER_UNIT: f32 = 0.24;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct BackgroundUniform {
    header: [f32; 4],
    solid: [f32; 4],
    image: [f32; 4],
    stops: [[f32; 4]; MAX_GRADIENT_STOPS],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct SpriteUniform {
    rect: [f32; 4],
    frame: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct BlurUniform {
    params: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ShadowUniform {
    rect: [f32; 4],
    shape: [f32; 4],
    tint: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ShapeUniform {
    geom: [f32; 4],
    params: [f32; 4],
    fill: [f32; 4],
    stroke: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct RegionUniform {
    rect: [f32; 4],
    params: [f32; 4],
    tint: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct CardUniform {
    rect: [f32; 4],
    canvas: [f32; 4],
    affine_a: [f32; 4],
    affine_b: [f32; 4],
    flags: [f32; 4],
    focus: [f32; 4],
}

/// Decoded source frames for one instant, addressed by layer.
#[derive(Default)]
pub struct FrameInputs<'a> {
    views: HashMap<LayerId, LayerInput<'a>>,
    background: Option<BackgroundImage<'a>>,
    /// Indexed by `CursorSlot::index`. A slot with no sprite draws the dot,
    /// which is how the host chooses between the two without another flag.
    cursor_sprites: [Option<CursorSprite<'a>>; 4],
    /// Keyed by the annotation's image path, so two annotations pointing at one
    /// file share a texture. Small and rarely more than a handful, so a linear
    /// scan beats hashing a path per frame.
    annotation_images: Vec<(String, LayerInput<'a>)>,
    /// This frame's caption, already laid out in canvas pixels. Owned rather
    /// than borrowed because the layout is built per frame, not uploaded.
    caption: CaptionFrame,
}

/// A pointer sprite and the point on it that sits on the cursor position.
#[derive(Debug, Clone, Copy)]
pub struct CursorSprite<'a> {
    pub view: &'a wgpu::TextureView,
    /// Normalised 0..1 within the sprite. `[0.5, 0.5]` centres it, which is
    /// right for a dot and wrong for an arrow.
    pub hotspot: [f32; 2],
}

/// The decoded wallpaper or image background. Separate from `LayerInput`
/// because it is a static asset that is cover-fitted, so its own size matters
/// and a decoded frame's never does.
pub struct BackgroundImage<'a> {
    pub view: &'a wgpu::TextureView,
    pub width: u32,
    pub height: u32,
    pub needs_srgb_decode: bool,
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

    pub fn set_background(&mut self, image: BackgroundImage<'a>) -> &mut Self {
        self.background = Some(image);
        self
    }

    pub fn background(&self) -> Option<&BackgroundImage<'a>> {
        self.background.as_ref()
    }

    pub fn set_cursor_sprite(&mut self, slot: CursorSlot, sprite: CursorSprite<'a>) -> &mut Self {
        self.cursor_sprites[slot.index()] = Some(sprite);
        self
    }

    pub fn set_annotation_image(&mut self, path: &str, input: LayerInput<'a>) -> &mut Self {
        match self.annotation_images.iter_mut().find(|(p, _)| p == path) {
            Some(slot) => slot.1 = input,
            None => self.annotation_images.push((path.to_string(), input)),
        }
        self
    }

    pub fn annotation_image(&self, path: &str) -> Option<&LayerInput<'a>> {
        self.annotation_images
            .iter()
            .find(|(p, _)| p == path)
            .map(|(_, input)| input)
    }

    pub fn set_caption(&mut self, caption: CaptionFrame) -> &mut Self {
        self.caption = caption;
        self
    }

    pub fn cursor_sprite(&self, slot: CursorSlot) -> Option<CursorSprite<'a>> {
        self.cursor_sprites[slot.index()]
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
    blur: BlurPass,
    yuv: YuvPass,
    shadow: ShadowPass,
    card: CardPass,
    shape: ShapePass,
    region: RegionPass,
    image: ImagePass,
    sprite: SpritePass,
    text: TextPass,
    present: PresentPass,
    working: Option<(u32, u32, wgpu::Texture)>,
}

struct BackgroundPass {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    uniform: wgpu::Buffer,
    sampler: wgpu::Sampler,
    /// Bound whenever the background is not an image, so the layout stays
    /// satisfied without a second pipeline.
    placeholder: wgpu::TextureView,
}

struct BlurPass {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    horizontal: wgpu::Buffer,
    vertical: wgpu::Buffer,
    /// Zero taps: a plain copy through the blur pipeline, so a blur annotation
    /// with no radius still gets its tint without a second pipeline.
    identity: wgpu::Buffer,
    sampler: wgpu::Sampler,
    scratch: Option<(u32, u32, wgpu::Texture)>,
}

struct ShadowPass {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
}

struct CardPass {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
}

/// Takes a decoded frame's Y'CbCr planes to the linear working space, so every
/// pass after it sees one RGBA texture and knows nothing about subsampling.
struct YuvPass {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    /// Bound as the third plane for NV12, whose chroma is one texture.
    placeholder: wgpu::TextureView,
    /// Reused across frames of one shape, since a source rarely changes size.
    planes: Option<(u32, u32, PlaneLayout, Vec<wgpu::Texture>)>,
}

struct ShapePass {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
}

/// Draws a pre-blurred copy of the composite back inside one rect. Separate
/// from `ShapePass` because it samples the target it draws into, which a render
/// pass cannot do without the copy.
struct RegionPass {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    /// Holds the blurred composite. Only allocated once a blur is on screen.
    blurred: Option<(u32, u32, wgpu::Texture)>,
}

/// Shares `RegionPass`'s bind-group layout and sampler; only the fragment
/// shader differs, so it is a second pipeline rather than a second pass type.
struct ImagePass {
    pipeline: wgpu::RenderPipeline,
}

struct SpritePass {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    uniform: wgpu::Buffer,
    sampler: wgpu::Sampler,
}

struct PresentPass {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
}

impl Compositor {
    pub fn new(ctx: &GpuContext) -> Result<Self, GpuError> {
        let device = ctx.device().clone();
        let queue = ctx.queue().clone();
        Ok(Self {
            background: BackgroundPass::new(&device, &queue),
            blur: BlurPass::new(&device),
            yuv: YuvPass::new(&device),
            shadow: ShadowPass::new(&device),
            card: CardPass::new(&device),
            shape: ShapePass::new(&device),
            region: RegionPass::new(&device),
            image: ImagePass::new(&device),
            sprite: SpritePass::new(&device),
            text: TextPass::new(&device),
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

        self.draw_background(&mut encoder, &working_view, params, inputs, width, height);
        self.blur_background(&mut encoder, &working_view, params, width, height);
        self.draw_shadow(&mut encoder, &working_view, params);
        let stats = self.draw_layers(&mut encoder, &working_view, params, inputs, width, height);
        self.draw_annotations(&mut encoder, &working_view, params, inputs, width, height);
        self.draw_cursor(&mut encoder, &working_view, params, inputs, width, height);
        // Captions last: they sit above the pointer, as the DOM overlay does.
        self.draw_caption_pill(&mut encoder, &working_view, &inputs.caption);
        self.text.draw(
            &self.device,
            &self.queue,
            &mut encoder,
            &working_view,
            &inputs.caption.glyphs,
            (width, height),
        );
        self.present(&mut encoder, &working_view, target);

        self.queue.submit([encoder.finish()]);
        stats
    }

    /// The caption's backing box, through the same rounded-rect pass the
    /// annotations use rather than a pass of its own.
    fn draw_caption_pill(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        working: &wgpu::TextureView,
        caption: &CaptionFrame,
    ) {
        let Some(pill) = caption.pill else { return };
        if pill.w <= 0.0 || pill.h <= 0.0 || pill.color.a == 0 {
            return;
        }
        let params = AnnotationParams {
            shape: AnnotationShape::Rect {
                x: pill.x,
                y: pill.y,
                w: pill.w,
                h: pill.h,
                radius: pill.radius,
            },
            fill: pill.color,
            stroke: recast_color::TRANSPARENT,
            stroke_width: 0.0,
            alpha: 1.0,
        };
        self.draw_shape_run(encoder, working, &[&params]);
    }

    /// A destination for [`Compositor::decode_source`], sized to one frame.
    ///
    /// The caller owns it and should keep it across frames: reallocating a
    /// full-resolution float target every frame is the expensive part.
    pub fn source_texture(&self, width: u32, height: u32) -> wgpu::Texture {
        self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("recast-source"),
            size: wgpu::Extent3d {
                width: width.max(1),
                height: height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: WORKING_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        })
    }

    /// Uploads one decoded frame's planes and converts them to linear light.
    ///
    /// The result goes into the layer pass as a plain `LayerInput` with
    /// `needs_srgb_decode` false, because the decode already happened here.
    /// Doing it as its own pass rather than inside the card shader means the
    /// dolly blur's taps each cost one sample instead of three.
    pub fn decode_source(
        &mut self,
        frame: &SourcePlanes<'_>,
        target: &wgpu::Texture,
    ) -> Result<(), YuvError> {
        let planes = planes_of(frame)?;
        if target.format() != WORKING_FORMAT {
            return Err(YuvError::TargetFormat);
        }
        if target.width() != frame.width || target.height() != frame.height {
            return Err(YuvError::TargetSize {
                need: (frame.width, frame.height),
                got: (target.width(), target.height()),
            });
        }

        let textures =
            self.yuv
                .plane_textures(&self.device, frame.width, frame.height, frame.layout);
        for (index, (texture, plane)) in textures.iter().zip(&planes).enumerate() {
            let (w, h, _) = frame.layout.plane_size(index, frame.width, frame.height);
            self.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                plane.bytes,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(plane.stride),
                    rows_per_image: Some(h),
                },
                wgpu::Extent3d {
                    width: w,
                    height: h,
                    depth_or_array_layers: 1,
                },
            );
        }

        let views: Vec<wgpu::TextureView> = textures
            .iter()
            .map(|t| t.create_view(&Default::default()))
            .collect();
        let uniform = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("yuv-uniform"),
            size: std::mem::size_of::<YuvUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.queue.write_buffer(
            &uniform,
            0,
            bytemuck::bytes_of(&yuv_uniform(&frame.color, frame.layout)),
        );

        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("yuv"),
            layout: &self.yuv.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&views[0]),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&views[1]),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(
                        views.get(2).unwrap_or(&self.yuv.placeholder),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::Sampler(&self.yuv.sampler),
                },
            ],
        });

        let view = target.create_view(&Default::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("recast-yuv"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("yuv"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
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
            pass.set_pipeline(&self.yuv.pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.draw(0..3, 0..1);
        }
        self.queue.submit([encoder.finish()]);
        Ok(())
    }

    /// Uploads whatever the caller has packed since the last frame. Separate
    /// from `render` because the atlas belongs to the layout, not the frame.
    pub fn sync_glyph_atlas(&mut self, atlas: &mut recast_text::GlyphAtlas) {
        self.text.sync(&self.device, &self.queue, atlas);
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
        inputs: &FrameInputs<'_>,
        width: u32,
        height: u32,
    ) {
        let image = match params.background {
            BackgroundParams::Asset { .. } => inputs.background(),
            _ => None,
        };
        let uniform = background_uniform(&params.background, image, width, height);
        self.queue
            .write_buffer(&self.background.uniform, 0, bytemuck::bytes_of(&uniform));

        let view = match image {
            Some(image) => image.view,
            None => &self.background.placeholder,
        };
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("background"),
            layout: &self.background.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.background.uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.background.sampler),
                },
            ],
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

    /// Blurs whatever the background pass just drew. Only an image can show it:
    /// a flat colour is unchanged and a linear gradient blurs to itself, which
    /// is why the FFmpeg path skips gradients explicitly.
    fn blur_background(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        working: &wgpu::TextureView,
        params: &FrameParams,
        width: u32,
        height: u32,
    ) {
        if !matches!(params.background, BackgroundParams::Asset { .. }) {
            return;
        }
        let Some(plan) = blur_plan(params.background_blur, width, height) else {
            return;
        };

        self.queue.write_buffer(
            &self.blur.horizontal,
            0,
            bytemuck::bytes_of(&BlurUniform {
                params: [plan.step_u, 0.0, plan.taps, plan.sigma_in_steps],
            }),
        );
        self.queue.write_buffer(
            &self.blur.vertical,
            0,
            bytemuck::bytes_of(&BlurUniform {
                params: [0.0, plan.step_v, plan.taps, plan.sigma_in_steps],
            }),
        );

        let scratch = self.blur_scratch(width, height);
        self.run_blur_axis(encoder, &self.blur.horizontal, working, &scratch);
        self.run_blur_axis(encoder, &self.blur.vertical, &scratch, working);
    }

    fn scratch_texture(&self, label: &str, width: u32, height: u32) -> wgpu::Texture {
        self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: WORKING_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        })
    }

    fn blur_scratch(&mut self, width: u32, height: u32) -> wgpu::TextureView {
        let reuse = matches!(&self.blur.scratch, Some((w, h, _)) if *w == width && *h == height);
        if !reuse {
            let texture = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("recast-blur-scratch"),
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
            self.blur.scratch = Some((width, height, texture));
        }
        match &self.blur.scratch {
            Some((_, _, texture)) => texture.create_view(&Default::default()),
            None => unreachable!("the scratch texture was just created"),
        }
    }

    fn run_blur_axis(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        uniform: &wgpu::Buffer,
        src: &wgpu::TextureView,
        dst: &wgpu::TextureView,
    ) {
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("blur"),
            layout: &self.blur.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(src),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.blur.sampler),
                },
            ],
        });
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("blur"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: dst,
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
        pass.set_pipeline(&self.blur.pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.draw(0..3, 0..1);
    }

    fn draw_shadow(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        working: &wgpu::TextureView,
        params: &FrameParams,
    ) {
        if params.shadows.is_empty() {
            return;
        }
        let mut buffers = Vec::with_capacity(params.shadows.len());
        let mut bind_groups = Vec::with_capacity(params.shadows.len());
        for shadow in &params.shadows {
            let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("shadow-uniform"),
                size: std::mem::size_of::<ShadowUniform>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.queue
                .write_buffer(&buffer, 0, bytemuck::bytes_of(&shadow_uniform(shadow)));
            bind_groups.push(self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("shadow"),
                layout: &self.shadow.layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buffer.as_entire_binding(),
                }],
            }));
            buffers.push(buffer);
        }

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("shadow"),
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
        pass.set_pipeline(&self.shadow.pipeline);
        for bind_group in &bind_groups {
            pass.set_bind_group(0, bind_group, &[]);
            pass.draw(0..3, 0..1);
        }
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

            let uniform = card_uniform(layer, width, height, input.needs_srgb_decode);
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

    /// Draw order is `(z_index, insertion)`, so a blur has to interrupt the
    /// batch rather than be hoisted: it must see everything painted before it
    /// and nothing painted after.
    fn draw_annotations(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        working: &wgpu::TextureView,
        params: &FrameParams,
        inputs: &FrameInputs<'_>,
        width: u32,
        height: u32,
    ) {
        let mut run: Vec<&AnnotationParams> = Vec::new();
        for annotation in &params.annotations {
            match &annotation.shape {
                AnnotationShape::Blur { .. } => {
                    self.draw_shape_run(encoder, working, &run);
                    run.clear();
                    self.draw_blur_region(encoder, working, annotation, width, height);
                }
                AnnotationShape::Image { .. } => {
                    self.draw_shape_run(encoder, working, &run);
                    run.clear();
                    self.draw_annotation_image(encoder, working, annotation, inputs);
                }
                _ => run.push(annotation),
            }
        }
        self.draw_shape_run(encoder, working, &run);
    }

    /// Skipped, not drawn as a placeholder, while the host is still decoding the
    /// asset: an editor-only dashed box baked into an export would be worse than
    /// a late image.
    fn draw_annotation_image(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        working: &wgpu::TextureView,
        annotation: &AnnotationParams,
        inputs: &FrameInputs<'_>,
    ) {
        let AnnotationShape::Image {
            x,
            y,
            w,
            h,
            radius,
            opacity,
            path,
        } = &annotation.shape
        else {
            return;
        };
        if *w <= 0.0 || *h <= 0.0 {
            return;
        }
        let Some(input) = inputs.annotation_image(path) else {
            return;
        };

        let uniform = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("annotation-image-uniform"),
            size: std::mem::size_of::<RegionUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.queue.write_buffer(
            &uniform,
            0,
            bytemuck::bytes_of(&RegionUniform {
                rect: [*x, *y, *w, *h],
                params: [radius.max(0.0), opacity * annotation.alpha, 0.0, 0.0],
                tint: [0.0; 4],
            }),
        );
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("annotation-image"),
            layout: &self.region.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(input.view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.region.sampler),
                },
            ],
        });
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("annotation-image"),
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
        pass.set_pipeline(&self.image.pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.draw(0..3, 0..1);
    }

    /// Blurs the whole composite into a scratch, then paints the rect back from
    /// it. Full-canvas because the blur must not run out of source at the rect's
    /// edge; one pair of passes per blur annotation, which is the price of
    /// keeping draw order.
    fn draw_blur_region(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        working: &wgpu::TextureView,
        annotation: &AnnotationParams,
        width: u32,
        height: u32,
    ) {
        let AnnotationShape::Blur {
            x,
            y,
            w,
            h,
            radius,
            sigma_px,
            tint,
        } = annotation.shape
        else {
            return;
        };
        if w <= 0.0 || h <= 0.0 {
            return;
        }

        let blurred = self.blurred_target(width, height);
        match plan_from_sigma(sigma_px, width, height) {
            Some(plan) => {
                self.queue.write_buffer(
                    &self.blur.horizontal,
                    0,
                    bytemuck::bytes_of(&BlurUniform {
                        params: [plan.step_u, 0.0, plan.taps, plan.sigma_in_steps],
                    }),
                );
                self.queue.write_buffer(
                    &self.blur.vertical,
                    0,
                    bytemuck::bytes_of(&BlurUniform {
                        params: [0.0, plan.step_v, plan.taps, plan.sigma_in_steps],
                    }),
                );
                let scratch = self.blur_scratch(width, height);
                self.run_blur_axis(encoder, &self.blur.horizontal, working, &scratch);
                self.run_blur_axis(encoder, &self.blur.vertical, &scratch, &blurred);
            }
            // Strength 0 still draws, because the tint alone is a valid redaction.
            None => self.run_blur_axis(encoder, &self.blur.identity, working, &blurred),
        }

        let uniform = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("region-uniform"),
            size: std::mem::size_of::<RegionUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.queue.write_buffer(
            &uniform,
            0,
            bytemuck::bytes_of(&RegionUniform {
                rect: [x, y, w, h],
                params: [radius.max(0.0), 0.0, 0.0, 0.0],
                tint: srgba_parts(tint),
            }),
        );
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("region"),
            layout: &self.region.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&blurred),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.region.sampler),
                },
            ],
        });
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("annotation-blur"),
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
        pass.set_pipeline(&self.region.pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.draw(0..3, 0..1);
    }

    fn blurred_target(&mut self, width: u32, height: u32) -> wgpu::TextureView {
        let reuse = matches!(&self.region.blurred, Some((w, h, _)) if *w == width && *h == height);
        if !reuse {
            let texture = self.scratch_texture("recast-blur-region", width, height);
            self.region.blurred = Some((width, height, texture));
        }
        match &self.region.blurred {
            Some((_, _, texture)) => texture.create_view(&Default::default()),
            None => unreachable!("the blurred target was just created"),
        }
    }

    fn draw_shape_run(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        working: &wgpu::TextureView,
        annotations: &[&AnnotationParams],
    ) {
        if annotations.is_empty() {
            return;
        }
        let mut buffers = Vec::with_capacity(annotations.len());
        let mut bind_groups = Vec::with_capacity(annotations.len());
        for annotation in annotations {
            let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("shape-uniform"),
                size: std::mem::size_of::<ShapeUniform>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.queue
                .write_buffer(&buffer, 0, bytemuck::bytes_of(&shape_uniform(annotation)));
            bind_groups.push(self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("shape"),
                layout: &self.shape.layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buffer.as_entire_binding(),
                }],
            }));
            buffers.push(buffer);
        }

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("annotations"),
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
        pass.set_pipeline(&self.shape.pipeline);
        for bind_group in &bind_groups {
            pass.set_bind_group(0, bind_group, &[]);
            pass.draw(0..3, 0..1);
        }
    }

    /// The pointer, drawn last so it sits above the annotations. The highlight
    /// ring is a filled ellipse through the shape pass; the pointer itself is a
    /// sprite when the host uploaded one and the dot otherwise.
    fn draw_cursor(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        working: &wgpu::TextureView,
        params: &FrameParams,
        inputs: &FrameInputs<'_>,
        width: u32,
        height: u32,
    ) {
        let Some(cursor) = params.cursor_draw else {
            return;
        };

        if let Some(highlight) = cursor.highlight {
            if highlight.alpha > 0.0 {
                self.draw_ellipse(
                    encoder,
                    working,
                    [
                        highlight.x,
                        highlight.y,
                        highlight.radius_px,
                        highlight.radius_px,
                    ],
                    highlight.color,
                    highlight.alpha,
                );
            }
        }

        if cursor.alpha <= 0.0 {
            return;
        }
        match inputs.cursor_sprite(cursor.slot) {
            Some(sprite) => self.draw_sprite(encoder, working, &cursor, sprite, width, height),
            None => self.draw_ellipse(
                encoder,
                working,
                [
                    cursor.x,
                    cursor.y,
                    cursor.dot_radius_px,
                    cursor.dot_radius_px,
                ],
                Srgba::opaque(255, 255, 255),
                cursor.alpha,
            ),
        }
    }

    fn draw_ellipse(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        working: &wgpu::TextureView,
        geom: [f32; 4],
        color: Srgba,
        alpha: f32,
    ) {
        let uniform = ShapeUniform {
            geom,
            params: [1.0, 0.0, 0.0, alpha.clamp(0.0, 1.0)],
            fill: srgba_parts(color),
            stroke: [0.0; 4],
        };
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("cursor-shape"),
            size: std::mem::size_of::<ShapeUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.queue
            .write_buffer(&buffer, 0, bytemuck::bytes_of(&uniform));
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("cursor-shape"),
            layout: &self.shape.layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        });
        let mut pass = self.overlay_pass(encoder, working, "cursor-shape");
        pass.set_pipeline(&self.shape.pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.draw(0..3, 0..1);
    }

    fn draw_sprite(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        working: &wgpu::TextureView,
        cursor: &CursorDraw,
        sprite: CursorSprite<'_>,
        width: u32,
        height: u32,
    ) {
        // The hotspot, not the centre, lands on the cursor position: an arrow
        // points from its tip and a centred one would sit half a sprite low.
        let uniform = SpriteUniform {
            rect: [
                cursor.x - sprite.hotspot[0] * cursor.sprite_px,
                cursor.y - sprite.hotspot[1] * cursor.sprite_px,
                cursor.sprite_px,
                cursor.sprite_px,
            ],
            frame: [
                width as f32,
                height as f32,
                cursor.alpha.clamp(0.0, 1.0),
                1.0,
            ],
        };
        self.queue
            .write_buffer(&self.sprite.uniform, 0, bytemuck::bytes_of(&uniform));
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("cursor-sprite"),
            layout: &self.sprite.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.sprite.uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(sprite.view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.sprite.sampler),
                },
            ],
        });
        let mut pass = self.overlay_pass(encoder, working, "cursor-sprite");
        pass.set_pipeline(&self.sprite.pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.draw(0..3, 0..1);
    }

    fn overlay_pass<'a>(
        &self,
        encoder: &'a mut wgpu::CommandEncoder,
        working: &'a wgpu::TextureView,
        label: &'static str,
    ) -> wgpu::RenderPass<'a> {
        encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some(label),
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
        })
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
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(working),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.present.sampler),
                },
            ],
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

/// Centred cover fit: the axis that overflows is scaled below 1 so the sampled
/// UV window is the visible crop. Stretching instead, which the WebGL preview
/// does today, distorts every wallpaper that is not exactly the canvas aspect.
fn cover_fit_scale(canvas: (u32, u32), image: (u32, u32)) -> [f32; 2] {
    let canvas_aspect = canvas.0.max(1) as f32 / canvas.1.max(1) as f32;
    let image_aspect = image.0.max(1) as f32 / image.1.max(1) as f32;
    match image_aspect > canvas_aspect {
        true => [canvas_aspect / image_aspect, 1.0],
        false => [1.0, image_aspect / canvas_aspect],
    }
}

fn background_uniform(
    background: &BackgroundParams,
    image: Option<&BackgroundImage<'_>>,
    width: u32,
    height: u32,
) -> BackgroundUniform {
    let mut uniform = BackgroundUniform {
        header: [0.0, 0.0, width as f32, height as f32],
        solid: [0.0, 0.0, 0.0, 1.0],
        image: [0.0; 4],
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
        // An image that has not arrived yet renders as the fallback grey rather
        // than as an undefined surface; the editor loads it asynchronously.
        BackgroundParams::Asset { .. } => match image {
            Some(image) => {
                let scale = cover_fit_scale((width, height), (image.width, image.height));
                uniform.image = [
                    1.0,
                    scale[0],
                    scale[1],
                    match image.needs_srgb_decode {
                        true => 1.0,
                        false => 0.0,
                    },
                ];
            }
            None => uniform.solid = linear_rgba(Srgba::opaque(0x11, 0x11, 0x11)),
        },
    }
    uniform
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct BlurPlan {
    step_u: f32,
    step_v: f32,
    taps: f32,
    sigma_in_steps: f32,
}

fn blur_plan(amount: f32, width: u32, height: u32) -> Option<BlurPlan> {
    plan_from_sigma(amount * BLUR_PX_PER_UNIT, width, height)
}

/// `None` when the radius is below half a pixel, where the two extra full-canvas
/// passes would cost more than they change.
fn plan_from_sigma(sigma: f32, width: u32, height: u32) -> Option<BlurPlan> {
    let sigma = sigma.max(0.0);
    if sigma < 0.5 {
        return None;
    }
    // Widening the stride past MAX_BLUR_TAPS keeps the loop bounded; bilinear
    // filtering hides the gap at these radii.
    let stride = (sigma * 3.0 / MAX_BLUR_TAPS).max(1.0);
    let sigma_in_steps = sigma / stride;
    let taps = (sigma_in_steps * 3.0).ceil().clamp(1.0, MAX_BLUR_TAPS);
    Some(BlurPlan {
        step_u: stride / width.max(1) as f32,
        step_v: stride / height.max(1) as f32,
        taps,
        sigma_in_steps,
    })
}

fn shadow_uniform(shadow: &ShadowParams) -> ShadowUniform {
    ShadowUniform {
        rect: [
            shadow.center_x,
            shadow.center_y,
            shadow.half_w,
            shadow.half_h,
        ],
        shape: [
            shadow.blur_px,
            shadow.spread_px,
            shadow.offset_y_px,
            shadow.radius_px,
        ],
        tint: [
            shadow.color.r as f32 / 255.0,
            shadow.color.g as f32 / 255.0,
            shadow.color.b as f32 / 255.0,
            shadow.opacity,
        ],
    }
}

fn srgba_parts(color: recast_color::Srgba) -> [f32; 4] {
    [
        color.r as f32 / 255.0,
        color.g as f32 / 255.0,
        color.b as f32 / 255.0,
        color.alpha_f32(),
    ]
}

fn shape_uniform(annotation: &AnnotationParams) -> ShapeUniform {
    let (geom, kind, detail) = match annotation.shape {
        AnnotationShape::Rect { x, y, w, h, radius } => ([x, y, w, h], 0.0, radius),
        AnnotationShape::Ellipse { cx, cy, rx, ry } => ([cx, cy, rx, ry], 1.0, 0.0),
        // Blur and image have their own passes, which never call this.
        AnnotationShape::Blur { .. } | AnnotationShape::Image { .. } => ([0.0; 4], 0.0, 0.0),
        AnnotationShape::Arrow {
            x1,
            y1,
            x2,
            y2,
            head,
        } => ([x1, y1, x2, y2], 2.0, head),
    };
    ShapeUniform {
        geom,
        params: [kind, detail, annotation.stroke_width, annotation.alpha],
        fill: srgba_parts(annotation.fill),
        stroke: srgba_parts(annotation.stroke),
    }
}

fn card_uniform(
    layer: &LayerParams,
    width: u32,
    height: u32,
    needs_srgb_decode: bool,
) -> CardUniform {
    let t = layer.transform;
    let d = layer.dest;
    CardUniform {
        rect: [d.x, d.y, d.w, d.h],
        canvas: [width as f32, height as f32, 0.0, 0.0],
        affine_a: [t.sx, t.shx, t.tx, t.shy],
        affine_b: [t.sy, t.ty, layer.opacity, layer.corner_radius],
        flags: [
            if needs_srgb_decode { 1.0 } else { 0.0 },
            layer.rotate,
            streak_length(layer),
            0.0,
        ],
        focus: [
            layer.zoom_center[0],
            layer.zoom_center[1],
            match layer.cover_fit {
                true => 1.0,
                false => 0.0,
            },
            0.0,
        ],
    }
}

/// Streak length in source UV. Velocity-driven, so the blur fires during a ramp
/// and vanishes on the hold; `MAX_STREAK` keeps a fast ramp from smearing the
/// whole frame.
fn streak_length(layer: &LayerParams) -> f32 {
    const MAX_STREAK: f32 = 0.35;
    const VELOCITY_SCALE: f32 = 0.08;
    if layer.motion_blur <= 0.0 {
        return 0.0;
    }
    (layer.motion_blur * layer.zoom_velocity.abs() * VELOCITY_SCALE).min(MAX_STREAK)
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
pub(crate) const PREMULTIPLIED: wgpu::BlendState = wgpu::BlendState {
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

fn sampled_texture_layout(device: &wgpu::Device, label: &str) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some(label),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
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
    })
}

pub(crate) fn clamped_linear_sampler(device: &wgpu::Device, label: &str) -> wgpu::Sampler {
    device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some(label),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::MipmapFilterMode::Nearest,
        ..Default::default()
    })
}

impl BlurPass {
    fn new(device: &wgpu::Device) -> Self {
        let layout = sampled_texture_layout(device, "blur");
        let pipeline = fullscreen_pipeline(
            device,
            "blur",
            include_str!("shaders/blur.wgsl"),
            &layout,
            WORKING_FORMAT,
            None,
            "vs",
        );
        let uniform = |label| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: std::mem::size_of::<BlurUniform>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        };
        Self {
            pipeline,
            layout,
            horizontal: uniform("blur-horizontal"),
            vertical: uniform("blur-vertical"),
            identity: uniform("blur-identity"),
            sampler: clamped_linear_sampler(device, "blur"),
            scratch: None,
        }
    }
}

impl BackgroundPass {
    fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let layout = sampled_texture_layout(device, "background");
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
        let size = wgpu::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        };
        let placeholder = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("background-placeholder"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            placeholder.as_image_copy(),
            &[0, 0, 0, 255],
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: Some(1),
            },
            size,
        );
        Self {
            pipeline,
            layout,
            uniform,
            sampler: clamped_linear_sampler(device, "background"),
            placeholder: placeholder.create_view(&Default::default()),
        }
    }
}

impl ShadowPass {
    fn new(device: &wgpu::Device) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("shadow"),
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
            "shadow",
            include_str!("shaders/shadow.wgsl"),
            &layout,
            WORKING_FORMAT,
            Some(PREMULTIPLIED),
            "vs",
        );
        Self { pipeline, layout }
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

impl SpritePass {
    fn new(device: &wgpu::Device) -> Self {
        let layout = sampled_texture_layout(device, "sprite");
        let pipeline = fullscreen_pipeline(
            device,
            "sprite",
            include_str!("shaders/sprite.wgsl"),
            &layout,
            WORKING_FORMAT,
            Some(PREMULTIPLIED),
            "vs",
        );
        Self {
            pipeline,
            layout,
            uniform: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("sprite-uniform"),
                size: std::mem::size_of::<SpriteUniform>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            sampler: clamped_linear_sampler(device, "sprite"),
        }
    }
}

impl ShapePass {
    fn new(device: &wgpu::Device) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("shape"),
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
            "shape",
            include_str!("shaders/shape.wgsl"),
            &layout,
            WORKING_FORMAT,
            Some(PREMULTIPLIED),
            "vs",
        );
        Self { pipeline, layout }
    }
}

impl RegionPass {
    fn new(device: &wgpu::Device) -> Self {
        let layout = sampled_texture_layout(device, "region");
        let pipeline = fullscreen_pipeline(
            device,
            "region",
            include_str!("shaders/region.wgsl"),
            &layout,
            WORKING_FORMAT,
            Some(PREMULTIPLIED),
            "vs",
        );
        Self {
            pipeline,
            layout,
            sampler: clamped_linear_sampler(device, "region"),
            blurred: None,
        }
    }
}

impl YuvPass {
    fn new(device: &wgpu::Device) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("yuv"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                plane_binding(1),
                plane_binding(2),
                plane_binding(3),
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let pipeline = fullscreen_pipeline(
            device,
            "yuv",
            include_str!("shaders/yuv.wgsl"),
            &layout,
            WORKING_FORMAT,
            None,
            "vs",
        );
        let placeholder = device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("yuv-placeholder"),
                size: wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::R8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
            .create_view(&Default::default());
        Self {
            pipeline,
            layout,
            sampler: clamped_linear_sampler(device, "yuv"),
            placeholder,
            planes: None,
        }
    }

    /// Plane textures for this frame's shape, reallocated only when it changes.
    fn plane_textures(
        &mut self,
        device: &wgpu::Device,
        width: u32,
        height: u32,
        layout: PlaneLayout,
    ) -> &[wgpu::Texture] {
        let reuse = matches!(
            &self.planes,
            Some((w, h, l, _)) if *w == width && *h == height && *l == layout
        );
        if !reuse {
            let textures = (0..layout.plane_count())
                .map(|index| {
                    let (w, h, _) = layout.plane_size(index, width, height);
                    device.create_texture(&wgpu::TextureDescriptor {
                        label: Some("yuv-plane"),
                        size: wgpu::Extent3d {
                            width: w,
                            height: h,
                            depth_or_array_layers: 1,
                        },
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format: layout.plane_format(index),
                        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                        view_formats: &[],
                    })
                })
                .collect();
            self.planes = Some((width, height, layout, textures));
        }
        match &self.planes {
            Some((_, _, _, textures)) => textures,
            None => &[],
        }
    }
}

fn plane_binding(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}

impl ImagePass {
    fn new(device: &wgpu::Device) -> Self {
        let layout = sampled_texture_layout(device, "annotation-image");
        Self {
            pipeline: fullscreen_pipeline(
                device,
                "annotation-image",
                include_str!("shaders/image.wgsl"),
                &layout,
                WORKING_FORMAT,
                Some(PREMULTIPLIED),
                "vs",
            ),
        }
    }
}

impl PresentPass {
    fn new(device: &wgpu::Device) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("present"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
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
        Self {
            pipeline,
            layout,
            sampler: clamped_linear_sampler(device, "present"),
        }
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
            None,
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
        let u = background_uniform(
            &BackgroundParams::Gradient(Box::new(gradient)),
            None,
            10,
            10,
        );
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
        let u = background_uniform(
            &BackgroundParams::Gradient(Box::new(gradient)),
            None,
            10,
            10,
        );
        assert_eq!(u.header[0], MAX_GRADIENT_STOPS as f32);
    }

    fn asset() -> BackgroundParams {
        BackgroundParams::Asset {
            kind: "wallpaper".into(),
            value: "C:/nope.jpg".into(),
        }
    }

    #[test]
    fn an_asset_background_with_no_image_yet_renders_the_fallback_grey() {
        let u = background_uniform(&asset(), None, 10, 10);
        assert_eq!(u.header[0], 0.0);
        assert_eq!(u.image[0], 0.0);
        assert!((u.solid[0] - 17.0 / 255.0).abs() < 1e-6);
    }

    /// The WebGL preview samples the image at the raw canvas UV, which stretches
    /// any wallpaper whose aspect is not the canvas aspect. The export crops.
    /// This engine crops, so the two agree.
    #[test]
    fn a_wide_image_is_cropped_horizontally_rather_than_squashed() {
        let scale = cover_fit_scale((1000, 1000), (2000, 1000));
        assert!((scale[0] - 0.5).abs() < 1e-6);
        assert_eq!(scale[1], 1.0);
    }

    #[test]
    fn a_tall_image_is_cropped_vertically() {
        let scale = cover_fit_scale((1000, 1000), (1000, 2000));
        assert_eq!(scale[0], 1.0);
        assert!((scale[1] - 0.5).abs() < 1e-6);
    }

    #[test]
    fn an_image_that_already_matches_the_canvas_aspect_is_not_cropped_at_all() {
        assert_eq!(cover_fit_scale((1920, 1080), (3840, 2160)), [1.0, 1.0]);
    }

    #[test]
    fn a_degenerate_size_does_not_divide_by_zero() {
        let scale = cover_fit_scale((0, 0), (0, 0));
        assert!(scale[0].is_finite() && scale[1].is_finite());
    }

    /// Blurring is skipped below half a pixel: two full-canvas passes would cost
    /// more than they change.
    #[test]
    fn a_blur_too_small_to_see_is_not_run_at_all() {
        assert_eq!(blur_plan(0.0, 100, 100), None);
        assert_eq!(blur_plan(2.0, 100, 100), None);
        assert!(blur_plan(4.0, 100, 100).is_some());
    }

    #[test]
    fn the_tap_count_stays_bounded_however_large_the_radius() {
        for amount in [10.0, 100.0, 1_000.0, 100_000.0] {
            let plan = blur_plan(amount, 1920, 1080).expect("a plan");
            assert!(
                plan.taps <= MAX_BLUR_TAPS,
                "{amount} gave {} taps",
                plan.taps
            );
            assert!(plan.taps >= 1.0);
        }
    }

    /// The stride is in canvas pixels, so the UV step has to shrink as the
    /// canvas grows or the blur would widen with resolution.
    #[test]
    fn the_blur_radius_is_in_canvas_pixels_not_uv() {
        let small = blur_plan(100.0, 500, 500).expect("a plan");
        let large = blur_plan(100.0, 1000, 1000).expect("a plan");
        assert!((small.step_u / large.step_u - 2.0).abs() < 1e-4);
        assert_eq!(small.taps, large.taps);
    }

    fn layer(motion_blur: f32, zoom_velocity: f32) -> LayerParams {
        LayerParams {
            id: recast_scene::LayerId(0),
            visible: true,
            opacity: 1.0,
            transform: crate::Affine2::IDENTITY,
            dest: crate::eval::DestRect {
                x: 0.0,
                y: 0.0,
                w: 100.0,
                h: 100.0,
            },
            rotate: 0.0,
            corner_radius: 0.0,
            blur: 0.0,
            motion_blur,
            zoom_center: [0.5, 0.5],
            zoom_velocity,
            cover_fit: false,
        }
    }

    #[test]
    fn no_authored_motion_blur_means_no_streak() {
        assert_eq!(streak_length(&layer(0.0, 5.0)), 0.0);
    }

    /// The whole point of driving this from velocity: a held zoom is not moving,
    /// so it must not smear. The old FFmpeg path had no equivalent at all and
    /// the field was documented as preview-only.
    #[test]
    fn a_held_zoom_does_not_streak_however_strong_the_setting() {
        assert_eq!(streak_length(&layer(1.0, 0.0)), 0.0);
    }

    #[test]
    fn a_ramp_streaks_in_both_directions() {
        let up = streak_length(&layer(1.0, 4.0));
        let down = streak_length(&layer(1.0, -4.0));
        assert!(up > 0.0);
        assert_eq!(up, down);
    }

    #[test]
    fn a_violent_ramp_is_capped_rather_than_smearing_the_whole_frame() {
        assert!(streak_length(&layer(1.0, 10_000.0)) <= 0.35);
    }

    #[test]
    fn the_uniforms_match_the_std140_sizes_the_shaders_declare() {
        assert_eq!(std::mem::size_of::<BackgroundUniform>(), 16 * 3 + 16 * 8);
        assert_eq!(std::mem::size_of::<BlurUniform>(), 16);
        assert_eq!(std::mem::size_of::<CardUniform>(), 16 * 6);
    }
}
