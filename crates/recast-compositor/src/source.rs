use recast_scene::AudioGraph;

use crate::caption::CaptionFrame;
use crate::eval::FrameParams;
use crate::session::{OutputSize, Session};

/// What a render reads FROM: anything that can answer "what does the frame at
/// output time T contain".
///
/// `Session` implements it over a captured `Scene`. An authored composition
/// will implement it too. Export, capture and the CLI are written against this
/// trait rather than against `Scene`, so a second document kind does not fork
/// the render path — which is the one thing the engine rewrite forbids.
///
/// `FrameParams` is the compositor's input language, not the scene's: it is
/// geometry, background, cursor, shadows, layers and annotations. Producing it
/// is the whole contract.
pub trait RenderSource {
    fn output_size(&self) -> OutputSize;

    /// Output-axis seconds. Zero means nothing to render.
    fn output_duration(&self) -> f64;

    fn frame_at(&self, output_time: f64) -> FrameParams;

    /// `&mut` because laying out captions grows the glyph atlas.
    fn caption_frame(&mut self, output_time: f64) -> CaptionFrame;

    fn audio(&self) -> &AudioGraph;
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

    fn caption_frame(&mut self, output_time: f64) -> CaptionFrame {
        Self::caption_frame(self, output_time)
    }

    fn audio(&self) -> &AudioGraph {
        &self.scene().audio
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
    /// nothing in the trait requires a `Scene`.
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
        fn caption_frame(&mut self, _output_time: f64) -> CaptionFrame {
            unimplemented!("the stub only exercises the non-GPU half")
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
}
