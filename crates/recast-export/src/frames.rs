use recast_compositor::{
    BackgroundImage, FrameInputs, LayerInput, MissingInput, RenderSource, Renderable, SourceColor,
    SourcePlanes, YuvError,
};
use recast_gpu::Readback;

use crate::nv12_gpu::GpuNv12;
use crate::walk::FrameWalk;

/// What a sink expects, and what a frame is. Not `PlaneLayout`, which only
/// names YUV plane arrangements and has no RGBA member.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PixelLayout {
    #[default]
    Rgba,
    Nv12,
}

/// One finished frame, in whichever layout the loop produced.
///
/// A typed hand-off rather than bare bytes plus a flag: a sink told the wrong
/// layout writes a corrupt file that every test still calls green.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Frame<'a> {
    Rgba(&'a [u8]),
    /// Packed, converted on the GPU where the frame already was.
    Nv12(&'a [u8]),
}

impl Frame<'_> {
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        match self {
            Self::Rgba(bytes) | Self::Nv12(bytes) => bytes,
        }
    }

    #[must_use]
    pub fn layout(&self) -> PixelLayout {
        match self {
            Self::Rgba(_) => PixelLayout::Rgba,
            Self::Nv12(_) => PixelLayout::Nv12,
        }
    }
}

/// Why a render loop stopped. Generic over the picture and sink errors rather
/// than stringifying them: a caller has to tell end-of-file from a dead driver.
#[derive(Debug, thiserror::Error)]
pub enum RenderError<P, S> {
    #[error("decoding the source picture at {source_time}s: {error:?}")]
    Decode { source_time: f64, error: YuvError },
    #[error("reading the picture at {source_time}s")]
    Picture {
        source_time: f64,
        #[source]
        error: P,
    },
    #[error("writing frame {index}")]
    Sink {
        index: u64,
        #[source]
        error: S,
    },
    /// The compositor draws what it is given and skips the rest, so an input
    /// the host forgot to bind would leave the layer out of the file silently.
    #[error("nothing was bound for {} at {output_time}s", crate::frames::name_all(.missing))]
    MissingInputs {
        output_time: f64,
        missing: Vec<MissingInput>,
    },
}

/// Joins the missing inputs for the error text.
pub(crate) fn name_all(missing: &[MissingInput]) -> String {
    missing
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}

/// Decoded pictures on the SOURCE axis, which cuts and speed ramps pull away
/// from the output axis.
pub trait PictureSource {
    type Error: std::error::Error;

    /// The picture covering `source_time`, or `None` past the end. Repeats are
    /// correct when the output frame rate is higher than the source's.
    fn picture_at(&mut self, source_time: f64) -> Result<Option<SourcePlanes<'_>>, Self::Error>;
}

/// A [`PictureSource`] for documents that draw no source video at all.
pub struct NoPictures;

impl PictureSource for NoPictures {
    type Error = std::convert::Infallible;

    fn picture_at(&mut self, _source_time: f64) -> Result<Option<SourcePlanes<'_>>, Self::Error> {
        Ok(None)
    }
}

/// Everything the loop composites besides the screen recording. Grouped because
/// they are all optional and all uploaded by the caller.
pub struct Extras<'a, P> {
    /// A wallpaper or image background, uploaded once and bound every frame.
    pub background: Option<BackgroundImage<'a>>,
    /// The camera recording and how far it lags the screen, in seconds.
    pub camera: Option<(&'a mut P, f64)>,
    /// Decoded image annotations, keyed by the path the scene names them by.
    pub annotations: &'a [(String, wgpu::TextureView)],
}

impl<P> Default for Extras<'_, P> {
    fn default() -> Self {
        Self {
            background: None,
            camera: None,
            annotations: &[],
        }
    }
}

/// Drives a [`Renderable`] over a frame walk, handing each finished frame to a
/// sink as packed RGBA. Holds its buffers so a whole export allocates once.
#[derive(Default)]
pub struct FrameLoop {
    readback: Readback,
    source: Option<(u32, u32, wgpu::Texture)>,
    /// The camera's own slot: a bubble is a different size from the screen, so
    /// sharing one texture would reallocate on every frame.
    camera: Option<(u32, u32, wgpu::Texture)>,
    rgba: Vec<u8>,
    source_allocations: u64,
    /// Set when the caller wants NV12; the pass is built on first use, which is
    /// the first point a device is in hand.
    nv12: Option<(SourceColor, Option<GpuNv12>)>,
}

impl FrameLoop {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A loop that converts to NV12 on the GPU, handing the sink [`Frame::Nv12`].
    ///
    /// Nine times faster than converting the readback on the CPU at 1080p, and
    /// it reads back 1.5 bytes a pixel instead of 4. Shapes the shader cannot
    /// pack fall back to [`Frame::Rgba`], so a sink still has to handle both.
    #[must_use]
    pub fn with_nv12(color: SourceColor) -> Self {
        Self {
            nv12: Some((color, None)),
            ..Self::default()
        }
    }

    /// Renders every frame in `walk` into `sink`. The pixels are borrowed and
    /// reused, so a sink that keeps a frame must copy it.
    /// `background` is uploaded by the caller and bound every frame: it is a
    /// still, so re-uploading it per frame would cost the whole image each time.
    ///
    /// `camera` is the camera recording and how far it lags the screen, sampled
    /// on the same source clock so the bubble stays with the take.
    ///
    /// `annotations` are the decoded image annotations, keyed by the path the
    /// scene names them by, uploaded once for the same reason as `background`.
    pub fn run<R, P, S, E>(
        &mut self,
        source: &mut R,
        pictures: &mut P,
        walk: FrameWalk,
        gpu: &recast_gpu::GpuContext,
        extras: Extras<'_, P>,
        mut sink: S,
    ) -> Result<u64, RenderError<P::Error, E>>
    where
        R: Renderable,
        P: PictureSource,
        S: FnMut(u64, Frame<'_>) -> Result<(), E>,
        E: std::error::Error,
    {
        let Extras {
            background,
            camera,
            annotations,
        } = extras;
        let layer = RenderSource::screen_layer(source);
        let camera_layer = RenderSource::camera_layer(source);
        // Taken apart so the loop can borrow the source without the offset.
        let (mut camera_pictures, camera_offset) = match camera {
            Some((pictures, offset)) => (Some(pictures), offset),
            None => (None, 0.0),
        };
        for (index, output_time) in walk.iter() {
            let params = source.frame_at(output_time);
            let source_time = params.source_time;
            let picture = pictures
                .picture_at(source_time)
                .map_err(|error| RenderError::Picture { source_time, error })?;

            let mut inputs = FrameInputs::new();
            // A decoder past the last frame is not a host that forgot to bind one, so those layers are excused from the check below.
            let mut exhausted: Vec<recast_scene::LayerId> = Vec::new();
            if picture.is_none() {
                exhausted.extend(layer);
            }
            let uploaded = match (picture, layer) {
                (Some(planes), Some(_)) => Some(self.upload(source, &planes, source_time)?),
                _ => None,
            };
            // Split from the upload so the texture outlives the borrow the view takes.
            let view = uploaded
                .as_ref()
                .map(|texture| texture.create_view(&wgpu::TextureViewDescriptor::default()));
            if let (Some(view), Some(layer)) = (&view, layer) {
                inputs.set(
                    layer,
                    LayerInput {
                        view,
                        needs_srgb_decode: false,
                    },
                );
            }
            if let Some(image) = &background {
                inputs.set_background(*image);
            }
            for (path, view) in annotations {
                inputs.set_annotation_image(
                    path,
                    LayerInput {
                        view,
                        needs_srgb_decode: true,
                    },
                );
            }
            let camera_uploaded = match (camera_pictures.as_deref_mut(), camera_layer) {
                (Some(pictures), Some(_)) => {
                    let at = (source_time + camera_offset).max(0.0);
                    let planes = pictures
                        .picture_at(at)
                        .map_err(|error| RenderError::Picture {
                            source_time: at,
                            error,
                        })?;
                    match planes {
                        Some(planes) => Some(self.upload_camera(source, &planes, at)?),
                        None => {
                            exhausted.extend(camera_layer);
                            None
                        }
                    }
                }
                _ => None,
            };
            let camera_view = camera_uploaded
                .as_ref()
                .map(|texture| texture.create_view(&wgpu::TextureViewDescriptor::default()));
            if let (Some(view), Some(layer)) = (&camera_view, camera_layer) {
                inputs.set(
                    layer,
                    LayerInput {
                        view,
                        needs_srgb_decode: false,
                    },
                );
            }
            inputs.set_caption(source.caption_frame(output_time));

            let missing: Vec<_> = inputs
                .missing_for(&params)
                .into_iter()
                .filter(|m| !matches!(m, MissingInput::Layer(id) if exhausted.contains(id)))
                .collect();
            if !missing.is_empty() {
                return Err(RenderError::MissingInputs {
                    output_time,
                    missing,
                });
            }
            let (target, _) = source.render_to_texture(output_time, &inputs);
            let frame = match self.convert(gpu.device(), gpu.queue(), &target) {
                true => Frame::Nv12(&self.rgba),
                false => Frame::Rgba(&self.rgba),
            };
            sink(index, frame).map_err(|error| RenderError::Sink { index, error })?;
        }
        Ok(walk.len())
    }

    /// Fills `self.rgba`, returning whether it holds NV12 rather than RGBA.
    fn convert(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target: &wgpu::Texture,
    ) -> bool {
        if let Some((color, pass)) = &mut self.nv12 {
            let pass = pass.get_or_insert_with(|| GpuNv12::new(device));
            if pass.convert(device, queue, target, color, &mut self.rgba) {
                return true;
            }
        }
        self.readback.read(device, queue, target, &mut self.rgba);
        false
    }

    /// The screen's texture, resized only when the picture's size changes.
    fn upload<R: Renderable, P, S>(
        &mut self,
        source: &mut R,
        planes: &SourcePlanes<'_>,
        source_time: f64,
    ) -> Result<wgpu::Texture, RenderError<P, S>> {
        let mut slot = self.source.take();
        let (texture, allocated) = Self::upload_into(&mut slot, source, planes, source_time)?;
        self.source = slot;
        if allocated {
            self.source_allocations += 1;
        }
        Ok(texture)
    }

    /// The camera's texture, kept apart from the screen's so neither resizes
    /// the other every frame.
    fn upload_camera<R: Renderable, P, S>(
        &mut self,
        source: &mut R,
        planes: &SourcePlanes<'_>,
        source_time: f64,
    ) -> Result<wgpu::Texture, RenderError<P, S>> {
        let mut slot = self.camera.take();
        let (texture, _) = Self::upload_into(&mut slot, source, planes, source_time)?;
        self.camera = slot;
        Ok(texture)
    }

    /// Fills `slot`'s texture with `planes`, reallocating only on a size change.
    /// The bool says whether it did, which is what the allocation count means.
    fn upload_into<R: Renderable, P, S>(
        slot: &mut Option<(u32, u32, wgpu::Texture)>,
        source: &mut R,
        planes: &SourcePlanes<'_>,
        source_time: f64,
    ) -> Result<(wgpu::Texture, bool), RenderError<P, S>> {
        let (width, height) = (planes.width.max(1), planes.height.max(1));
        let reuse = matches!(&slot, Some((w, h, _)) if *w == width && *h == height);
        if !reuse {
            *slot = Some((width, height, source.source_texture(width, height)));
        }
        let Some((_, _, texture)) = slot else {
            unreachable!("the texture was just created")
        };
        let texture = texture.clone();
        source
            .decode_source(planes, &texture)
            .map_err(|error| RenderError::Decode { source_time, error })?;
        Ok((texture, !reuse))
    }

    /// Source textures allocated. A steady loop over one recording must not
    /// grow this past one.
    #[must_use]
    pub fn source_allocations(&self) -> u64 {
        self.source_allocations
    }
}
