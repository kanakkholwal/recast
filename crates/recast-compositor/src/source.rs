use recast_scene::{AudioGraph, LayerId};

use crate::caption::CaptionFrame;
use crate::eval::FrameParams;
use crate::render::{FrameInputs, RenderStats};
use crate::session::{OutputSize, Session};
use crate::yuv::{SourcePlanes, YuvError};

/// What a render reads FROM: what the frame at output time T contains.
/// GPU-free on purpose, so analysis and the agent plane stay headless.
pub trait RenderSource {
    fn output_size(&self) -> OutputSize;

    /// Output-axis seconds. Zero means nothing to render.
    fn output_duration(&self) -> f64;

    fn frame_at(&self, output_time: f64) -> FrameParams;

    /// Which layer a decoded screen picture belongs to. `None` means the
    /// document has no screen track, which an authored composition may not.
    fn screen_layer(&self) -> Option<LayerId>;

    /// Which layer the camera recording belongs to, where the scene has one.
    fn camera_layer(&self) -> Option<LayerId> {
        None
    }

    fn audio(&self) -> &AudioGraph;
}

/// A [`RenderSource`] that can also draw itself. Layout and draw stay behind
/// one implementor because a caption's glyph cache is 1:1 with its compositor.
pub trait Renderable: RenderSource {
    /// `&mut` because laying out captions rasterises into the glyph cache.
    fn caption_frame(&mut self, output_time: f64) -> CaptionFrame;

    /// A texture sized for one decoded picture, to be filled by
    /// [`Self::decode_source`] and handed back through `FrameInputs`.
    fn source_texture(&self, width: u32, height: u32) -> wgpu::Texture;

    /// Uploads one decoded picture and converts it to linear light.
    fn decode_source(
        &mut self,
        planes: &SourcePlanes<'_>,
        target: &wgpu::Texture,
    ) -> Result<(), YuvError>;

    /// Renders into a scene-sized texture the implementor reuses across calls.
    fn render_to_texture(
        &mut self,
        output_time: f64,
        inputs: &FrameInputs<'_>,
    ) -> (wgpu::Texture, RenderStats);
}

impl RenderSource for Session {
    fn output_size(&self) -> OutputSize {
        Self::output_size(self)
    }

    fn output_duration(&self) -> f64 {
        Self::output_duration(self)
    }

    fn frame_at(&self, output_time: f64) -> FrameParams {
        self.evaluate(output_time)
    }

    fn camera_layer(&self) -> Option<LayerId> {
        Self::camera_layer(self)
    }

    fn screen_layer(&self) -> Option<LayerId> {
        Self::screen_layer(self)
    }

    fn audio(&self) -> &AudioGraph {
        &self.scene().audio
    }
}

impl Renderable for Session {
    fn caption_frame(&mut self, output_time: f64) -> CaptionFrame {
        Self::caption_frame(self, output_time)
    }

    fn source_texture(&self, width: u32, height: u32) -> wgpu::Texture {
        Self::source_texture(self, width, height)
    }

    fn decode_source(
        &mut self,
        planes: &SourcePlanes<'_>,
        target: &wgpu::Texture,
    ) -> Result<(), YuvError> {
        Self::decode_source(self, planes, target)
    }

    fn render_to_texture(
        &mut self,
        output_time: f64,
        inputs: &FrameInputs<'_>,
    ) -> (wgpu::Texture, RenderStats) {
        Self::render_to_texture(self, output_time, inputs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The point of the trait is that a caller can be written without naming
    /// `Session` or `Scene`. If this stops compiling, the seam has leaked.
    fn duration_of(source: &dyn RenderSource) -> f64 {
        source.output_duration()
    }

    fn size_of(source: &impl RenderSource) -> OutputSize {
        source.output_size()
    }

    /// A second implementor standing in for an authored composition, proving
    /// nothing in the read half requires a `Scene` or a GPU.
    struct Stub {
        audio: AudioGraph,
    }

    impl RenderSource for Stub {
        fn output_size(&self) -> OutputSize {
            OutputSize {
                width: 1920,
                height: 1080,
            }
        }
        fn output_duration(&self) -> f64 {
            4.5
        }
        fn frame_at(&self, _output_time: f64) -> FrameParams {
            unimplemented!("the stub only exercises the non-GPU half")
        }
        fn screen_layer(&self) -> Option<LayerId> {
            None
        }
        fn audio(&self) -> &AudioGraph {
            &self.audio
        }
    }

    #[test]
    fn a_non_scene_source_satisfies_the_contract() {
        let stub = Stub {
            audio: AudioGraph::default(),
        };
        assert_eq!(duration_of(&stub), 4.5);
        assert_eq!(size_of(&stub).width, 1920);
    }

    /// `Stub` implements `RenderSource` and NOT `Renderable`, so this
    /// compiling is the assertion: the read half needs no adapter.
    #[test]
    fn the_read_half_is_usable_without_a_renderer() {
        fn read_only(source: &impl RenderSource) -> f64 {
            source.output_duration()
        }
        assert_eq!(
            read_only(&Stub {
                audio: AudioGraph::default()
            }),
            4.5
        );
    }
}
