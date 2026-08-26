use ttf_parser::OutlineBuilder;
use zeno::{Fill, Mask};

use crate::face::FontFace;

/// An 8-bit coverage bitmap for one glyph, plus where it sits relative to the
/// pen. `left`/`top` are in pixels, y DOWN, so the canvas position is
/// `(pen.x + left, baseline.y + top)`.
#[derive(Debug, Clone, PartialEq)]
pub struct GlyphMask {
    pub width: u32,
    pub height: u32,
    pub left: i32,
    pub top: i32,
    pub coverage: Vec<u8>,
}

impl GlyphMask {
    pub fn is_empty(&self) -> bool {
        self.width == 0 || self.height == 0
    }
}

#[derive(Default)]
struct PathSink {
    commands: Vec<zeno::Command>,
}

/// ttf-parser walks the outline in FONT units with y UP; zeno fills in the
/// space we hand it, so the scale to pixels and the y flip happen here rather
/// than in a second pass over the points.
struct Scaled {
    sink: PathSink,
    scale: f32,
}

impl OutlineBuilder for Scaled {
    fn move_to(&mut self, x: f32, y: f32) {
        self.sink.commands.push(zeno::Command::MoveTo(
            [x * self.scale, -y * self.scale].into(),
        ));
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.sink.commands.push(zeno::Command::LineTo(
            [x * self.scale, -y * self.scale].into(),
        ));
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.sink.commands.push(zeno::Command::QuadTo(
            [x1 * self.scale, -y1 * self.scale].into(),
            [x * self.scale, -y * self.scale].into(),
        ));
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.sink.commands.push(zeno::Command::CurveTo(
            [x1 * self.scale, -y1 * self.scale].into(),
            [x2 * self.scale, -y2 * self.scale].into(),
            [x * self.scale, -y * self.scale].into(),
        ));
    }

    fn close(&mut self) {
        self.sink.commands.push(zeno::Command::Close);
    }
}

/// Rasterises one glyph at `px`. `None` when the face has no outline for it,
/// which is the normal answer for a space.
pub fn rasterize(face: &FontFace, glyph: u16, px: f64) -> Option<GlyphMask> {
    let parsed = face.parse()?;
    let mut builder = Scaled {
        sink: PathSink::default(),
        scale: (px / face.units_per_em()) as f32,
    };
    parsed.outline_glyph(ttf_parser::GlyphId(glyph), &mut builder)?;
    if builder.sink.commands.is_empty() {
        return None;
    }

    let path = builder.sink.commands.as_slice();
    let bounds = zeno::bounds(path, Fill::NonZero, None);
    // Whole pixels, so the mask covers every partially-touched one.
    let left = bounds.min.x.floor() as i32;
    let top = bounds.min.y.floor() as i32;
    let width = (bounds.max.x.ceil() - left as f32).max(0.0).ceil() as u32;
    let height = (bounds.max.y.ceil() - top as f32).max(0.0).ceil() as u32;
    if width == 0 || height == 0 {
        return None;
    }

    let mut coverage = vec![0u8; (width * height) as usize];
    Mask::new(path)
        .style(Fill::NonZero)
        .origin(zeno::Origin::TopLeft)
        .offset([-left as f32, -top as f32])
        .size(width, height)
        .render_into(&mut coverage, None);

    Some(GlyphMask {
        width,
        height,
        left,
        top,
        coverage,
    })
}
