use recast_compositor::{BackgroundImage, FrameInputs, LayerInput, Session, SourceGeometry};
use recast_gpu::{GpuContext, GpuOptions, OUTPUT_FORMAT};
use recast_scene::LayerId;
use wasm_bindgen::prelude::*;

use crate::backend::{backend_name, backends_for};
use crate::cursor_io::parse_track;
use crate::scene_io::parse_scene;

/// A decoded frame parked for the next `render`. Held as a texture rather than a
/// `VideoFrame`: retaining a `VideoFrame` past the decoder's expectations
/// silently stops the decoder, which is the failure this project has already hit
/// once on the TypeScript side.
struct LayerTexture {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    width: u32,
    height: u32,
}

#[wasm_bindgen]
pub struct PreviewEngine {
    ctx: GpuContext,
    session: Session,
    surface: wgpu::Surface<'static>,
    surface_size: (u32, u32),
    frames: Vec<(LayerId, LayerTexture)>,
    background: Option<LayerTexture>,
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

        Ok(Self {
            ctx,
            session,
            surface,
            surface_size: (0, 0),
            frames: Vec::new(),
            background: None,
        })
    }

    #[wasm_bindgen(js_name = setScene)]
    pub fn set_scene(&mut self, json: &str) -> Result<(), JsValue> {
        let scene = parse_scene(json).map_err(|e| JsValue::from_str(&e.to_string()))?;
        self.session.set_scene(scene);
        Ok(())
    }

    /// The recorded pointer path, as the track file is written. Held on the
    /// scene, so it survives `setScene` only if that scene carries one; the
    /// editor calls this again after replacing the scene.
    #[wasm_bindgen(js_name = setCursorTrack)]
    pub fn set_cursor_track(&mut self, json: &str) -> Result<(), JsValue> {
        let track = parse_track(json).map_err(|e| JsValue::from_str(&e.to_string()))?;
        let mut scene = self.session.scene().clone();
        scene.cursor_track = Some(track);
        self.session.set_scene(scene);
        Ok(())
    }

    /// Where the pointer sits at `output_time`, as
    /// `[x, y, alpha, scale, pressed, right, dragging, hlX, hlY, hlAlpha]`, or
    /// an empty array when there is nothing to draw. A flat array rather than an
    /// object because this is read every frame.
    #[wasm_bindgen(js_name = cursorAt)]
    pub fn cursor_at(&self, output_time: f64) -> Vec<f64> {
        let Some(cursor) = self.session.evaluate(output_time).cursor else {
            return Vec::new();
        };
        let highlight = cursor.highlight.unwrap_or(recast_compositor::Highlight {
            x: 0.0,
            y: 0.0,
            alpha: 0.0,
        });
        vec![
            cursor.x,
            cursor.y,
            cursor.alpha,
            cursor.scale,
            f64::from(u8::from(cursor.pressed)),
            f64::from(u8::from(cursor.right)),
            f64::from(u8::from(cursor.dragging)),
            highlight.x,
            highlight.y,
            highlight.alpha,
        ]
    }

    #[wasm_bindgen(js_name = setSourceSize)]
    pub fn set_source_size(&mut self, width: u32, height: u32) {
        self.session.set_source(SourceGeometry {
            width: width.max(1),
            height: height.max(1),
        });
    }

    #[wasm_bindgen(js_name = screenLayerId)]
    pub fn screen_layer_id(&self) -> Option<u32> {
        self.session.screen_layer().map(|id| id.0)
    }

    #[wasm_bindgen(js_name = cameraLayerId)]
    pub fn camera_layer_id(&self) -> Option<u32> {
        self.session.camera_layer().map(|id| id.0)
    }

    /// Uploads a decoded `VideoFrame` for `layer_id`. The frame is copied into a
    /// GPU texture here and is NOT retained, so the caller must still close it.
    #[wasm_bindgen(js_name = setLayerFrame)]
    pub fn set_layer_frame(
        &mut self,
        layer_id: u32,
        frame: &web_sys::VideoFrame,
    ) -> Result<(), JsValue> {
        let width = frame.display_width();
        let height = frame.display_height();
        if width == 0 || height == 0 {
            return Err(JsValue::from_str("VideoFrame has a zero dimension"));
        }

        let id = LayerId(layer_id);
        let index = self.slot_for(id, width, height);
        let slot = &self.frames[index].1;
        self.ctx.queue().copy_external_image_to_texture(
            &wgpu::CopyExternalImageSourceInfo {
                // `VideoFrame::clone` is the JS method, which returns a Result;
                // this is the Rust handle clone, which does not copy the frame.
                source: wgpu::ExternalImageSource::VideoFrame(Clone::clone(frame)),
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
        Ok(())
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
        Ok(())
    }

    #[wasm_bindgen(js_name = clearBackgroundImage)]
    pub fn clear_background_image(&mut self) {
        self.background = None;
    }

    /// Renders `output_time` (gapless output-timeline seconds) to the canvas.
    /// Returns the layers drawn.
    #[wasm_bindgen]
    pub fn render(&mut self, output_time: f64) -> Result<u32, JsValue> {
        let size = self.session.output_size();
        self.configure_surface(size.width, size.height);

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
        for (id, slot) in &self.frames {
            inputs.set(
                *id,
                LayerInput {
                    view: &slot.view,
                    needs_srgb_decode: true,
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

    /// The backend actually in use, for the preview to report. `"auto"` resolves
    /// here, so this is the only honest answer to "did WebGPU work".
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
    /// Index into `frames`, not a borrow: returning a reference here would
    /// hold `self` and block the queue access at the call site.
    fn slot_for(&mut self, id: LayerId, width: u32, height: u32) -> usize {
        if let Some(index) = self.frames.iter().position(|(slot_id, _)| *slot_id == id) {
            let slot = &self.frames[index].1;
            if slot.width == width && slot.height == height {
                return index;
            }
            self.frames.remove(index);
        }

        let slot = self.new_texture(width, height);
        self.frames.push((id, slot));
        self.frames.len() - 1
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
