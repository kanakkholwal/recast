use recast_compositor::{
    FrameInputs, LayerInput, RenderSource, Renderable, SourcePlanes, YuvError,
};
use recast_gpu::Readback;

use crate::walk::FrameWalk;

/// Why a render loop stopped.
#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    #[error("decoding the source picture at {source_time}s: {error:?}")]
    Decode { source_time: f64, error: YuvError },
    #[error("reading the picture at {source_time}s: {message}")]
    Picture { source_time: f64, message: String },
    #[error("writing frame {index}: {message}")]
    Sink { index: u64, message: String },
}

/// Decoded pictures on the SOURCE axis, which cuts and speed ramps pull away
/// from the output axis.
pub trait PictureSource {
    /// The picture covering `source_time`, or `None` past the end. Repeats are
    /// correct when the output frame rate is higher than the source's.
    fn picture_at(&mut self, source_time: f64) -> Result<Option<SourcePlanes<'_>>, String>;
}

/// A [`PictureSource`] for documents that draw no source video at all.
pub struct NoPictures;

impl PictureSource for NoPictures {
    fn picture_at(&mut self, _source_time: f64) -> Result<Option<SourcePlanes<'_>>, String> {
        Ok(None)
    }
}

/// Drives a [`Renderable`] over a frame walk, handing each finished frame to a
/// sink as packed RGBA. Holds its buffers so a whole export allocates once.
#[derive(Default)]
pub struct FrameLoop {
    readback: Readback,
    source: Option<(u32, u32, wgpu::Texture)>,
    rgba: Vec<u8>,
    source_allocations: u64,
}

impl FrameLoop {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Renders every frame in `walk` into `sink`. The pixels are borrowed and
    /// reused, so a sink that keeps a frame must copy it.
    pub fn run<R, P, S>(
        &mut self,
        source: &mut R,
        pictures: &mut P,
        walk: FrameWalk,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        mut sink: S,
    ) -> Result<u64, RenderError>
    where
        R: Renderable,
        P: PictureSource,
        S: FnMut(u64, &[u8]) -> Result<(), String>,
    {
        let layer = RenderSource::screen_layer(source);
        for (index, output_time) in walk.iter() {
            let params = source.frame_at(output_time);
            let source_time = params.source_time;
            let picture =
                pictures
                    .picture_at(source_time)
                    .map_err(|message| RenderError::Picture {
                        source_time,
                        message,
                    })?;

            let mut inputs = FrameInputs::new();
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
            inputs.set_caption(source.caption_frame(output_time));

            let (target, _) = source.render_to_texture(output_time, &inputs);
            self.readback.read(device, queue, &target, &mut self.rgba);
            sink(index, &self.rgba).map_err(|message| RenderError::Sink { index, message })?;
        }
        Ok(walk.len())
    }

    /// The source texture, resized only when the picture's size changes.
    fn upload<R: Renderable>(
        &mut self,
        source: &mut R,
        planes: &SourcePlanes<'_>,
        source_time: f64,
    ) -> Result<wgpu::Texture, RenderError> {
        let (width, height) = (planes.width.max(1), planes.height.max(1));
        let reuse = matches!(&self.source, Some((w, h, _)) if *w == width && *h == height);
        if !reuse {
            self.source_allocations += 1;
            self.source = Some((width, height, source.source_texture(width, height)));
        }
        let Some((_, _, texture)) = &self.source else {
            unreachable!("the source texture was just created")
        };
        let texture = texture.clone();
        source
            .decode_source(planes, &texture)
            .map_err(|error| RenderError::Decode { source_time, error })?;
        Ok(texture)
    }

    /// Source textures allocated. A steady loop over one recording must not
    /// grow this past one.
    #[must_use]
    pub fn source_allocations(&self) -> u64 {
        self.source_allocations
    }
}
