use std::path::Path;

use super::frame::Frame;

/// Context available to every render node during processing.
pub struct RenderContext<'a> {
    /// Current timestamp in seconds relative to the video start.
    pub timestamp: f64,
    /// Path to the static assets directory (wallpapers, etc.).
    pub static_root: &'a Path,
    /// Path to the cursor track JSON, if available.
    pub cursor_track_path: Option<&'a Path>,
    /// Original source video dimensions (before any transforms).
    pub source_width: u32,
    pub source_height: u32,
}

/// A single processing step in the render pipeline.
pub trait ProcessingNode: Send + Sync {
    /// Apply this node's effect to the input frame and return the result.
    /// Nodes can resize, composite, overlay, or transform the frame.
    fn process(&self, frame: Frame, ctx: &RenderContext) -> Result<Frame, String>;

    /// Human-readable name for logging and debugging.
    fn name(&self) -> &str;
}

/// An ordered sequence of processing nodes applied to each frame.
pub struct RenderPipeline {
    nodes: Vec<Box<dyn ProcessingNode>>,
}

impl RenderPipeline {
    pub fn new() -> Self {
        Self { nodes: Vec::new() }
    }

    /// Add a processing node to the end of the pipeline.
    pub fn push(&mut self, node: Box<dyn ProcessingNode>) {
        self.nodes.push(node);
    }

    /// Process a single frame through all nodes in order.
    pub fn process_frame(&self, frame: Frame, ctx: &RenderContext) -> Result<Frame, String> {
        let mut current = frame;
        for node in &self.nodes {
            current = node.process(current, ctx)?;
        }
        Ok(current)
    }
}
