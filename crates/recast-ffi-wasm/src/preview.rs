use recast_compositor::{
    BackgroundImage, CursorSprite, FrameInputs, LayerInput, Session, SourceGeometry,
};
use recast_gpu::{GpuContext, GpuOptions, OUTPUT_FORMAT};
use recast_scene::LayerId;
use wasm_bindgen::prelude::*;

use crate::backend::{backend_name, backends_for};
use crate::cursor_io::parse_track;
use crate::ring::pick_slot;
use crate::scene_io::parse_scene;
use crate::slot::{parse_slot, slot_at};

/// Slots per layer when the host has not sized the ring. Matches the WebGL
/// render worker this replaces.
const DEFAULT_RING_CAPACITY: usize = 6;

/// A decoded frame parked for a later `render`. Held as a texture rather than a
/// `VideoFrame`: retaining a `VideoFrame` past the decoder's expectations
/// silently stops the decoder, which is the failure this project has already hit
/// once on the TypeScript side.
struct LayerTexture {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    width: u32,
    height: u32,
    /// Presentation timestamp in microseconds; negative when the slot is empty.
    ts_us: i64,
}

/// Recently decoded frames for one layer. A ring rather than a single texture
/// because frames arrive ahead of the playhead, so the draw loop has to choose
/// which one belongs to this instant.
struct LayerRing {
    slots: Vec<LayerTexture>,
    capacity: usize,
    next: usize,
    bound: Option<usize>,
}

impl LayerRing {
    fn new(capacity: usize) -> Self {
        Self {
            slots: Vec::new(),
            capacity: capacity.max(1),
            next: 0,
            bound: None,
        }
    }

    fn timestamps(&self) -> Vec<i64> {
        self.slots.iter().map(|s| s.ts_us).collect()
    }
}

#[wasm_bindgen]
pub struct PreviewEngine {
    ctx: GpuContext,
    session: Session,
    surface: wgpu::Surface<'static>,
    surface_size: (u32, u32),
    /// What the canvas backing store is, when the host has told us. The preview
    /// draws at DISPLAY resolution and lets the present pass scale the
    /// composition down, so a 4K project in a small pane is not composited at 4K.
    canvas_size: Option<(u32, u32)>,
    /// See `flush_uploads`.
    defers_uploads: bool,
    frames: Vec<(LayerId, LayerRing)>,
    background: Option<LayerTexture>,
    /// Indexed by `CursorSlot::index`. An empty slot draws the dot instead.
    cursor_sprites: [Option<LayerTexture>; 4],
    /// Normalised hotspot per slot, parallel to `cursor_sprites`.
    cursor_hotspots: [[f32; 2]; 4],
    /// Keyed by the annotation's image path, matching the host's own per-path
    /// cache, so two annotations on one file upload once.
    annotation_images: Vec<(String, LayerTexture)>,
}

#[wasm_bindgen]
impl PreviewEngine {
    /// `canvas` is an `HTMLCanvasElement` or an `OffscreenCanvas`. `backend` is
    /// "auto", "webgpu" or "webgl2".
    #[wasm_bindgen]
    pub async fn create(
        canvas: JsValue,
        backend: Option<String>,
    ) -> Result<PreviewEngine, JsValue> {
        console_error_panic_hook::set_once();

        let target = surface_target(canvas)?;
        // The preview accepts a software adapter: a slow preview beats none.
        let options = GpuOptions {
            require_hardware: false,
            label: "recast-preview",
            backends: Some(
                backends_for(
                    backend.as_deref(),
                    cfg!(feature = "webgpu"),
                    cfg!(feature = "webgl2"),
                )
                .map_err(|e| JsValue::from_str(&e))?,
            ),
            ..Default::default()
        };

        // The surface must come from the instance the device is built on, so the
        // instance is created here and handed to the context rather than the
        // context making a second one of its own.
        let instance = GpuContext::instance_for(&options);
        let surface = instance
            .create_surface(target)
            .map_err(|e| JsValue::from_str(&format!("create surface: {e}")))?;
        let ctx = GpuContext::from_instance(instance, options, Some(&surface))
            .await
            .map_err(|e| JsValue::from_str(&e.to_string()))?;

        let session = Session::new(
            &ctx,
            recast_scene::Scene::default(),
            SourceGeometry {
                width: 1,
                height: 1,
            },
        )
        .map_err(|e| JsValue::from_str(&e.to_string()))?;
        let defers_uploads = ctx.adapter().get_info().backend == wgpu::Backend::Gl;

        Ok(Self {
            ctx,
            session,
            surface,
            surface_size: (0, 0),
            canvas_size: None,
            defers_uploads,
            frames: Vec::new(),
            background: None,
            cursor_sprites: [None, None, None, None],
            cursor_hotspots: [[0.5, 0.5]; 4],
            annotation_images: Vec::new(),
        })
    }

    #[wasm_bindgen(js_name = setScene)]
    pub fn set_scene(&mut self, json: &str) -> Result<(), JsValue> {
        let scene = parse_scene(json).map_err(|e| JsValue::from_str(&e.to_string()))?;
        self.session.set_scene(scene);
        Ok(())
    }

    /// The editor's resolved output axis, as `store.timeMap` serialises. The
    /// editor's cut lanes and flags can drop a cut the scene still carries, so
    /// deriving the axis here instead would put every effect at a different
    /// instant from the picture. An empty string goes back to the scene's own.
    #[wasm_bindgen(js_name = setTimeMap)]
    pub fn set_time_map(&mut self, json: &str) -> Result<(), JsValue> {
        let map = match json.is_empty() {
            true => None,
            false => {
                Some(serde_json::from_str(json).map_err(|e| JsValue::from_str(&e.to_string()))?)
            }
        };
        self.session.set_time_map(map);
        Ok(())
    }

    /// The recorded pointer path, as the track file is written. Survives a
    /// later `setScene`, which is what the editor pushes on any store write.
    #[wasm_bindgen(js_name = setCursorTrack)]
    pub fn set_cursor_track(&mut self, json: &str) -> Result<(), JsValue> {
        let track = parse_track(json).map_err(|e| JsValue::from_str(&e.to_string()))?;
        self.session.set_cursor_track(Some(track));
        Ok(())
    }

    /// Where the pointer sits at `output_time`, in CANVAS PIXELS, as
    /// `[x, y, alpha, spritePx, dotRadiusPx, slot, hlX, hlY, hlRadiusPx,
    /// hlAlpha]`, or an empty array when there is nothing to draw.
    ///
    /// The engine draws the pointer itself; this exists so the host can place a
    /// DOM overlay on top (a tooltip, a hit target) without re-deriving the
    /// position from the scene.
    #[wasm_bindgen(js_name = cursorAt)]
    pub fn cursor_at(&self, output_time: f64) -> Vec<f64> {
        let Some(cursor) = self.session.evaluate(output_time).cursor_draw else {
            return Vec::new();
        };
        let (hl_x, hl_y, hl_radius, hl_alpha) = match cursor.highlight {
            Some(h) => (
                f64::from(h.x),
                f64::from(h.y),
                f64::from(h.radius_px),
                f64::from(h.alpha),
            ),
            None => (0.0, 0.0, 0.0, 0.0),
        };
        vec![
            f64::from(cursor.x),
            f64::from(cursor.y),
            f64::from(cursor.alpha),
            f64::from(cursor.sprite_px),
            f64::from(cursor.dot_radius_px),
            cursor.slot.index() as f64,
            hl_x,
            hl_y,
            hl_radius,
            hl_alpha,
        ]
    }

    #[wasm_bindgen(js_name = setSourceSize)]
    pub fn set_source_size(&mut self, width: u32, height: u32) {
        self.session.set_source(SourceGeometry {
            width: width.max(1),
            height: height.max(1),
        });
    }

    /// The canvas backing-store size. Pass the same values written to
    /// `canvas.width` / `canvas.height`; the aspect must match the composition
    /// or the present pass stretches.
    #[wasm_bindgen(js_name = setCanvasSize)]
    pub fn set_canvas_size(&mut self, width: u32, height: u32) {
        self.canvas_size = Some((width.max(1), height.max(1)));
    }

    #[wasm_bindgen(js_name = screenLayerId)]
    pub fn screen_layer_id(&self) -> Option<u32> {
        self.session.screen_layer().map(|id| id.0)
    }

    #[wasm_bindgen(js_name = cameraLayerId)]
    pub fn camera_layer_id(&self) -> Option<u32> {
        self.session.camera_layer().map(|id| id.0)
    }

    /// How many decoded frames this layer buffers. Sizing is the host's call: it
    /// knows the resolution and the memory budget.
    #[wasm_bindgen(js_name = setLayerRingCapacity)]
    pub fn set_layer_ring_capacity(&mut self, layer_id: u32, capacity: u32) {
        let id = LayerId(layer_id);
        self.frames.retain(|(slot_id, _)| *slot_id != id);
        self.frames.push((id, LayerRing::new(capacity as usize)));
    }

    /// Uploads a decoded frame and hands ownership straight back: the pixels are
    /// copied into a texture we own, so the caller must still close the frame.
    #[wasm_bindgen(js_name = putLayerFrame)]
    pub fn put_layer_frame(
        &mut self,
        layer_id: u32,
        frame: &web_sys::VideoFrame,
        timestamp_us: f64,
    ) -> Result<(), JsValue> {
        let width = frame.display_width();
        let height = frame.display_height();
        if width == 0 || height == 0 {
            return Err(JsValue::from_str("VideoFrame has a zero dimension"));
        }

        let index = self.slot_for(LayerId(layer_id), width, height);
        let ring = &mut self.frames[index].1;
        let slot_index = ring.next;
        ring.next = (slot_index + 1) % ring.slots.len().max(1);
        let slot = &mut ring.slots[slot_index];
        slot.ts_us = timestamp_us.max(0.0) as i64;
        let texture = slot.texture.clone();

        self.ctx.queue().copy_external_image_to_texture(
            &wgpu::CopyExternalImageSourceInfo {
                // `VideoFrame::clone` is the JS method, which returns a Result;
                // this is the Rust handle clone, which does not copy the frame.
                source: wgpu::ExternalImageSource::VideoFrame(Clone::clone(frame)),
                origin: wgpu::Origin2d::ZERO,
                flip_y: false,
            },
            wgpu::CopyExternalImageDestInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
                color_space: wgpu::PredefinedColorSpace::Srgb,
                premultiplied_alpha: false,
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        self.flush_uploads();
        Ok(())
    }

    /// Chooses the frame `render` will draw: the newest at or before
    /// `timestamp_us` that is not older than `floor_us`, the start of the current
    /// segment. False when nothing qualifies yet, which leaves the previous
    /// choice standing.
    #[wasm_bindgen(js_name = bindLayerFrame)]
    pub fn bind_layer_frame(&mut self, layer_id: u32, timestamp_us: f64, floor_us: f64) -> bool {
        let Some((_, ring)) = self.frames.iter_mut().find(|(id, _)| id.0 == layer_id) else {
            return false;
        };
        let picked = pick_slot(
            &ring.timestamps(),
            timestamp_us.max(0.0) as i64,
            floor_us.max(0.0) as i64,
        );
        if picked.is_some() {
            ring.bound = picked;
        }
        picked.is_some()
    }

    /// Whether a previously bound frame is still there to hold on to. True right
    /// after a cut, while the post-cut GOP decodes: freezing the last frame beats
    /// flashing the background.
    #[wasm_bindgen(js_name = hasBoundFrame)]
    pub fn has_bound_frame(&self, layer_id: u32) -> bool {
        self.frames
            .iter()
            .any(|(id, ring)| id.0 == layer_id && ring.bound.is_some())
    }

    #[wasm_bindgen(js_name = clearLayerFrame)]
    pub fn clear_layer_frame(&mut self, layer_id: u32) {
        self.frames.retain(|(id, _)| id.0 != layer_id);
    }

    /// Uploads the decoded wallpaper or image background. Cover-fitted against
    /// the canvas in the shader, so the natural size is what must be passed and
    /// the caller must not pre-scale.
    #[wasm_bindgen(js_name = setBackgroundImage)]
    pub fn set_background_image(&mut self, image: &web_sys::ImageBitmap) -> Result<(), JsValue> {
        let width = image.width();
        let height = image.height();
        if width == 0 || height == 0 {
            return Err(JsValue::from_str(
                "the background image has a zero dimension",
            ));
        }

        let stale =
            matches!(&self.background, Some(slot) if slot.width != width || slot.height != height);
        if stale || self.background.is_none() {
            self.background = Some(self.new_texture(width, height));
        }
        let Some(slot) = &self.background else {
            return Err(JsValue::from_str("the background texture was just created"));
        };

        self.ctx.queue().copy_external_image_to_texture(
            &wgpu::CopyExternalImageSourceInfo {
                source: wgpu::ExternalImageSource::ImageBitmap(Clone::clone(image)),
                origin: wgpu::Origin2d::ZERO,
                flip_y: false,
            },
            wgpu::CopyExternalImageDestInfo {
                texture: &slot.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
                color_space: wgpu::PredefinedColorSpace::Srgb,
                premultiplied_alpha: false,
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        self.flush_uploads();
        Ok(())
    }

    /// The WebGL backend records an external-image copy and runs it at the next
    /// submit, by which point the caller has closed the frame or the bitmap.
    /// WebGPU copies on the spot, so it is spared a submit per decoded frame.
    fn flush_uploads(&self) {
        if self.defers_uploads {
            self.ctx.queue().submit(std::iter::empty());
        }
    }

    #[wasm_bindgen(js_name = clearBackgroundImage)]
    pub fn clear_background_image(&mut self) {
        self.background = None;
    }

    /// Uploads one pointer sprite. `slot` is "rest", "press", "rightPress" or
    /// "drag"; a slot with no sprite draws the dot, which is how the host picks
    /// a pointer style without another flag.
    #[wasm_bindgen(js_name = setCursorSprite)]
    pub fn set_cursor_sprite(
        &mut self,
        slot: &str,
        image: &web_sys::ImageBitmap,
        hotspot_x: f32,
        hotspot_y: f32,
    ) -> Result<(), JsValue> {
        let slot = parse_slot(slot).map_err(|e| JsValue::from_str(&e))?;
        let (width, height) = (image.width(), image.height());
        if width == 0 || height == 0 {
            return Err(JsValue::from_str("the cursor sprite has a zero dimension"));
        }

        let index = slot.index();
        self.cursor_hotspots[index] = [hotspot_x.clamp(0.0, 1.0), hotspot_y.clamp(0.0, 1.0)];
        let stale = matches!(&self.cursor_sprites[index], Some(t) if t.width != width || t.height != height);
        if stale || self.cursor_sprites[index].is_none() {
            self.cursor_sprites[index] = Some(self.new_texture(width, height));
        }
        let Some(texture) = &self.cursor_sprites[index] else {
            return Err(JsValue::from_str("the sprite texture was just created"));
        };

        self.ctx.queue().copy_external_image_to_texture(
            &wgpu::CopyExternalImageSourceInfo {
                source: wgpu::ExternalImageSource::ImageBitmap(Clone::clone(image)),
                origin: wgpu::Origin2d::ZERO,
                flip_y: false,
            },
            wgpu::CopyExternalImageDestInfo {
                texture: &texture.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
                color_space: wgpu::PredefinedColorSpace::Srgb,
                premultiplied_alpha: false,
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        self.flush_uploads();
        Ok(())
    }

    /// The decoded asset for an image annotation, addressed by the same path the
    /// scene carries. Copied into a texture, so the caller still owns the bitmap.
    #[wasm_bindgen(js_name = setAnnotationImage)]
    pub fn set_annotation_image(
        &mut self,
        path: &str,
        image: &web_sys::ImageBitmap,
    ) -> Result<(), JsValue> {
        let (width, height) = (image.width(), image.height());
        if path.is_empty() || width == 0 || height == 0 {
            return Err(JsValue::from_str(
                "an annotation image needs a path and a non-zero size",
            ));
        }
        let texture = self.new_texture(width, height);
        self.ctx.queue().copy_external_image_to_texture(
            &wgpu::CopyExternalImageSourceInfo {
                source: wgpu::ExternalImageSource::ImageBitmap(Clone::clone(image)),
                origin: wgpu::Origin2d::ZERO,
                flip_y: false,
            },
            wgpu::CopyExternalImageDestInfo {
                texture: &texture.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
                color_space: wgpu::PredefinedColorSpace::Srgb,
                premultiplied_alpha: false,
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        self.flush_uploads();
        match self.annotation_images.iter_mut().find(|(p, _)| p == path) {
            Some(slot) => slot.1 = texture,
            None => self.annotation_images.push((path.to_string(), texture)),
        }
        Ok(())
    }

    #[wasm_bindgen(js_name = clearAnnotationImages)]
    pub fn clear_annotation_images(&mut self) {
        self.annotation_images.clear();
    }

    #[wasm_bindgen(js_name = clearCursorSprites)]
    pub fn clear_cursor_sprites(&mut self) {
        self.cursor_sprites = [None, None, None, None];
        self.cursor_hotspots = [[0.5, 0.5]; 4];
    }

    /// Renders `output_time` (gapless output-timeline seconds) to the canvas.
    /// Returns the layers drawn.
    #[wasm_bindgen]
    pub fn render(&mut self, output_time: f64) -> Result<u32, JsValue> {
        let (width, height) = match self.canvas_size {
            Some(size) => size,
            None => {
                let size = self.session.output_size();
                (size.width, size.height)
            }
        };
        self.configure_surface(width, height);

        // A dropped frame is a skip, not an error: the browser reports Timeout
        // and Occluded routinely, and turning either into a JS exception would
        // tear the preview down over a minimised window.
        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame)
            | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                return Ok(0)
            }
            other => {
                self.surface_size = (0, 0);
                return Err(JsValue::from_str(&format!(
                    "surface unavailable: {other:?}"
                )));
            }
        };
        let view = frame.texture.create_view(&Default::default());

        let mut inputs = FrameInputs::new();
        if let Some(slot) = &self.background {
            inputs.set_background(BackgroundImage {
                view: &slot.view,
                width: slot.width,
                height: slot.height,
                needs_srgb_decode: true,
            });
        }
        for (id, ring) in &self.frames {
            let Some(slot) = ring.bound.and_then(|index| ring.slots.get(index)) else {
                continue;
            };
            inputs.set(
                *id,
                LayerInput {
                    view: &slot.view,
                    needs_srgb_decode: true,
                },
            );
        }

        for (path, texture) in &self.annotation_images {
            inputs.set_annotation_image(
                path,
                LayerInput {
                    view: &texture.view,
                    needs_srgb_decode: true,
                },
            );
        }

        for (index, sprite) in self.cursor_sprites.iter().enumerate() {
            let (Some(sprite), Some(slot)) = (sprite, slot_at(index)) else {
                continue;
            };
            inputs.set_cursor_sprite(
                slot,
                CursorSprite {
                    view: &sprite.view,
                    hotspot: self.cursor_hotspots[index],
                },
            );
        }

        let stats = self.session.render(output_time, &inputs, &view);
        drop(view);
        self.ctx.queue().present(frame);
        Ok(stats.layers_drawn)
    }

    #[wasm_bindgen(js_name = outputWidth)]
    pub fn output_width(&self) -> u32 {
        self.session.output_size().width
    }

    #[wasm_bindgen(js_name = outputHeight)]
    pub fn output_height(&self) -> u32 {
        self.session.output_size().height
    }

    #[wasm_bindgen(js_name = outputDuration)]
    pub fn output_duration(&self) -> f64 {
        self.session.output_duration()
    }

    /// The backend actually in use. `"auto"` resolves here, so this is the only
    /// honest answer to "did WebGPU work".
    #[wasm_bindgen]
    pub fn backend(&self) -> String {
        backend_name(self.ctx.info().backend).to_string()
    }

    #[wasm_bindgen(js_name = adapterName)]
    pub fn adapter_name(&self) -> String {
        self.ctx.info().name
    }

    #[wasm_bindgen(js_name = isSoftware)]
    pub fn is_software(&self) -> bool {
        self.ctx.is_software()
    }

    #[wasm_bindgen]
    pub fn destroy(self) {
        drop(self);
    }
}

impl PreviewEngine {
    /// Index into `frames`, not a borrow: returning a reference would hold
    /// `self` and block the queue access at the call site. A resolution change
    /// reallocates the whole ring, because every slot is sized to the source.
    fn slot_for(&mut self, id: LayerId, width: u32, height: u32) -> usize {
        let index = match self.frames.iter().position(|(slot_id, _)| *slot_id == id) {
            Some(index) => index,
            None => {
                self.frames
                    .push((id, LayerRing::new(DEFAULT_RING_CAPACITY)));
                self.frames.len() - 1
            }
        };

        let ring = &self.frames[index].1;
        let sized = ring
            .slots
            .first()
            .is_some_and(|slot| slot.width == width && slot.height == height);
        if !sized {
            let slots = (0..ring.capacity)
                .map(|_| self.new_texture(width, height))
                .collect();
            let ring = &mut self.frames[index].1;
            ring.slots = slots;
            ring.next = 0;
            ring.bound = None;
        }
        index
    }

    fn new_texture(&self, width: u32, height: u32) -> LayerTexture {
        let texture = self.ctx.device().create_texture(&wgpu::TextureDescriptor {
            label: Some("recast-layer-frame"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        LayerTexture {
            texture,
            view,
            width,
            height,
            ts_us: -1,
        }
    }

    fn configure_surface(&mut self, width: u32, height: u32) {
        let (width, height) = (width.max(1), height.max(1));
        if self.surface_size == (width, height) {
            return;
        }
        self.surface.configure(
            self.ctx.device(),
            &wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format: OUTPUT_FORMAT,
                width,
                height,
                present_mode: wgpu::PresentMode::Fifo,
                color_space: wgpu::SurfaceColorSpace::Srgb,
                desired_maximum_frame_latency: 2,
                alpha_mode: wgpu::CompositeAlphaMode::Auto,
                view_formats: vec![],
            },
        );
        self.surface_size = (width, height);
    }
}

fn surface_target(canvas: JsValue) -> Result<wgpu::SurfaceTarget<'static>, JsValue> {
    if let Ok(element) = canvas.clone().dyn_into::<web_sys::HtmlCanvasElement>() {
        return Ok(wgpu::SurfaceTarget::Canvas(element));
    }
    if let Ok(offscreen) = canvas.dyn_into::<web_sys::OffscreenCanvas>() {
        return Ok(wgpu::SurfaceTarget::OffscreenCanvas(offscreen));
    }
    Err(JsValue::from_str(
        "expected an HTMLCanvasElement or an OffscreenCanvas",
    ))
}
