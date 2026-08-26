use crate::face::FontFace;

/// One glyph placed on the baseline, in pixels. `x`/`y` are the pen position the
/// glyph's outline is drawn from, with y increasing DOWN like the canvas.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PositionedGlyph {
    pub id: u16,
    pub x: f64,
    pub y: f64,
    /// Byte offset of the cluster this glyph came from, so a per-word highlight
    /// can decide which glyphs belong to which word.
    pub cluster: u32,
}

/// A shaped line: its glyphs and the pen advance they consumed.
#[derive(Debug, Clone, PartialEq)]
pub struct ShapedLine {
    pub glyphs: Vec<PositionedGlyph>,
    pub width: f64,
}

/// Shapes `text` at `px`, adding `spacing_px` after each glyph the way libass
/// applies ASS `Spacing`. Empty text shapes to an empty line rather than
/// failing, because a caption chunk can legitimately be blank between words.
pub fn shape_line(face: &FontFace, px: f64, text: &str, spacing_px: f64) -> ShapedLine {
    let Some(shaper) = face.shaper() else {
        return ShapedLine {
            glyphs: Vec::new(),
            width: 0.0,
        };
    };
    let mut buffer = rustybuzz::UnicodeBuffer::new();
    buffer.push_str(text);
    buffer.guess_segment_properties();
    let shaped = rustybuzz::shape(&shaper, &[], buffer);

    let scale = px / face.units_per_em();
    let mut pen = 0.0;
    let mut glyphs = Vec::with_capacity(shaped.len());
    for (info, position) in shaped
        .glyph_infos()
        .iter()
        .zip(shaped.glyph_positions().iter())
    {
        glyphs.push(PositionedGlyph {
            id: info.glyph_id as u16,
            x: pen + position.x_offset as f64 * scale,
            // Font y is up, ours is down.
            y: -(position.y_offset as f64) * scale,
            cluster: info.cluster,
        });
        pen += position.x_advance as f64 * scale + spacing_px;
    }
    ShapedLine { glyphs, width: pen }
}
