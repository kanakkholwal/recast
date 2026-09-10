//! Laying out a composition's text items into glyph quads, through the same shaper and atlas the captions use.
//! Line breaking is on explicit newlines only: an authored title says where it breaks, unlike a caption, which is handed a stream of words.

use recast_scene::composition::TextAlign;
use recast_text::{FontFace, GlyphAtlas};

use crate::eval::TextItemDraw;
use crate::text::GlyphQuad;

/// A placed glyph awaiting its uv, for the same reason the caption path defers them: packing can grow the atlas.
struct Pending {
    x: f32,
    y: f32,
    placed: recast_text::AtlasGlyph,
    colour: [f32; 4],
}

/// Shapes every item and packs its glyphs, returning them in the order they were given.
/// The box clips nothing: a title that overflows is the author's to fix, and silently cropping one is worse than showing it.
pub fn layout_items(
    items: &[TextItemDraw],
    face: &FontFace,
    face_id: u32,
    atlas: &mut GlyphAtlas,
) -> Vec<GlyphQuad> {
    let mut pending = Vec::new();
    for item in items {
        layout_one(item, face, face_id, atlas, &mut pending);
    }
    let (aw, ah) = atlas.size();
    pending
        .into_iter()
        .map(|q| GlyphQuad {
            rect: [q.x, q.y, q.placed.width as f32, q.placed.height as f32],
            uv: [
                q.placed.x as f32 / aw as f32,
                q.placed.y as f32 / ah as f32,
                (q.placed.x + q.placed.width) as f32 / aw as f32,
                (q.placed.y + q.placed.height) as f32 / ah as f32,
            ],
            colour: q.colour,
        })
        .collect()
}

fn layout_one(
    item: &TextItemDraw,
    face: &FontFace,
    face_id: u32,
    atlas: &mut GlyphAtlas,
    pending: &mut Vec<Pending>,
) {
    let px = f64::from(item.size_px);
    if !draws(item) {
        return;
    }
    let lines: Vec<&str> = item.content.lines().collect();
    let leading = px * line_height(item);
    let metrics = face.metrics();
    let (ascent, descent) = (metrics.ascender * px, -metrics.descender * px);
    // Centred in the box on the block's own height, so a two-line title sits where a one-line one does.
    let block = leading * (lines.len().saturating_sub(1)) as f64 + ascent + descent;
    let top = f64::from(item.rect[1]) + (f64::from(item.rect[3]) - block).max(0.0) * 0.5;
    let colour = colour_of(item);

    for (index, line) in lines.iter().enumerate() {
        let shaped = recast_text::shape_line(face, px, line, 0.0);
        let baseline = top + ascent + leading * index as f64;
        let pen = start_x(item, shaped.width);
        for glyph in &shaped.glyphs {
            let Some(placed) = atlas.insert(face_id, face, glyph.id, px) else {
                continue;
            };
            pending.push(Pending {
                x: (pen + glyph.x + f64::from(placed.left)) as f32,
                y: (baseline + glyph.y + f64::from(placed.top)) as f32,
                placed,
                colour,
            });
        }
    }
}

/// Whether an item puts anything on screen. Shaping a blank or invisible one would pack glyphs into the atlas for nothing.
fn draws(item: &TextItemDraw) -> bool {
    item.size_px > 0.0 && item.alpha > 0.0 && !item.content.trim().is_empty()
}

/// A zero or missing line height reads as the single-spaced default rather than stacking every line on one baseline.
fn line_height(item: &TextItemDraw) -> f64 {
    match item.line_height > 0.0 {
        true => item.line_height,
        false => 1.2,
    }
}

fn colour_of(item: &TextItemDraw) -> [f32; 4] {
    [
        f32::from(item.color.r) / 255.0,
        f32::from(item.color.g) / 255.0,
        f32::from(item.color.b) / 255.0,
        f32::from(item.color.a) / 255.0 * item.alpha,
    ]
}

fn start_x(item: &TextItemDraw, width: f64) -> f64 {
    let (left, box_w) = (f64::from(item.rect[0]), f64::from(item.rect[2]));
    match item.align {
        TextAlign::Start => left,
        TextAlign::Center => left + (box_w - width) * 0.5,
        TextAlign::End => left + box_w - width,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use recast_color::Srgba;

    fn item(content: &str, align: TextAlign) -> TextItemDraw {
        TextItemDraw {
            rect: [100.0, 50.0, 400.0, 200.0],
            content: content.into(),
            size_px: 32.0,
            color: Srgba::opaque(255, 255, 255),
            align,
            weight: 400.0,
            line_height: 1.2,
            alpha: 1.0,
        }
    }

    #[test]
    fn an_empty_or_invisible_item_shapes_nothing() {
        assert!(
            draws(&item("Hello", TextAlign::Center)),
            "the fixture draws"
        );

        assert!(
            !draws(&item("   ", TextAlign::Center)),
            "whitespace is not a title"
        );
        assert!(!draws(&TextItemDraw {
            alpha: 0.0,
            ..item("Hello", TextAlign::Center)
        }));
        assert!(!draws(&TextItemDraw {
            size_px: 0.0,
            ..item("Hello", TextAlign::Center)
        }));
    }

    #[test]
    fn alignment_places_the_pen_at_the_edge_it_names() {
        let width = 100.0;

        assert!((start_x(&item("x", TextAlign::Start), width) - 100.0).abs() < 1e-9);
        assert!((start_x(&item("x", TextAlign::Center), width) - 250.0).abs() < 1e-9);
        assert!((start_x(&item("x", TextAlign::End), width) - 400.0).abs() < 1e-9);
    }

    #[test]
    fn a_missing_line_height_falls_back_rather_than_collapsing_every_line_onto_one() {
        let flat = TextItemDraw {
            line_height: 0.0,
            ..item("a\nb", TextAlign::Center)
        };

        assert!((line_height(&flat) - 1.2).abs() < 1e-9);
        assert!((line_height(&item("a", TextAlign::Center)) - 1.2).abs() < 1e-9);
    }

    #[test]
    fn the_items_alpha_multiplies_its_colour_so_a_dissolve_reaches_the_glyphs() {
        let half = TextItemDraw {
            alpha: 0.5,
            ..item("Hello", TextAlign::Center)
        };

        assert!((colour_of(&half)[3] - 0.5).abs() < 1e-6);
    }
}
