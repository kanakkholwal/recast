use recast_captions::{
    active_chunk_index, active_cue, active_word_index, break_into_lines, caption_top_frac,
    chunk_words, pill_box, spoken_word_count, word_color, word_scaled, CaptionAnimation,
    CaptionStyle, CaptionTrack,
};
use recast_color::{parse_css_color, Srgba};
use recast_text::{shape_line, FontFace, GlyphAtlas};
use recast_time::{original_to_output, TimeMap};

use crate::text::GlyphQuad;

/// Emphasis scale for the punched word, matching the export's `\fscx114`.
const EMPHASIS_SCALE: f64 = 1.14;

/// Slide distance as a fraction of the font size, and the scale a pop starts
/// from. Both mirror the DOM painter, which is what the export matches.
const SLIDE_EM: f64 = 0.25;
const POP_FROM: f64 = 0.97;

/// The two clocks a caption needs. The chunk is resolved on the ORIGINAL axis
/// because words carry source times; the entrance is clocked on the OUTPUT axis
/// so it plays at viewer rate even across a speed change.
#[derive(Debug, Clone, Copy)]
pub struct CaptionClock<'a> {
    pub source: f64,
    pub output: f64,
    /// Projects the chunk's original start onto the output axis. `None` leaves
    /// the two axes identical, which is right when nothing retimes the clip.
    pub time_map: Option<&'a TimeMap>,
}

/// Opacity and the transform an entrance applies about the pill centre.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Entrance {
    alpha: f32,
    scale: f64,
    dy: f64,
}

impl Entrance {
    const NONE: Self = Self {
        alpha: 1.0,
        scale: 1.0,
        dy: 0.0,
    };
}

fn ease_out_quad(p: f64) -> f64 {
    1.0 - (1.0 - p) * (1.0 - p)
}

fn ease_out_cubic(p: f64) -> f64 {
    1.0 - (1.0 - p).powi(3)
}

fn entrance_at(anim: &CaptionAnimation, elapsed: f64, font_px: f64) -> Entrance {
    let duration = anim.entrance_ms.max(0.0) / 1000.0;
    if anim.entrance == "none" || duration <= 0.0 {
        return Entrance::NONE;
    }
    let p = (elapsed / duration).clamp(0.0, 1.0);
    if anim.entrance == "fade" {
        return Entrance {
            alpha: ease_out_quad(p) as f32,
            ..Entrance::NONE
        };
    }
    let e = ease_out_cubic(p);
    Entrance {
        alpha: e as f32,
        scale: POP_FROM + (1.0 - POP_FROM) * e,
        // Only `slide` travels; `pop` scales in place.
        dy: match anim.entrance == "slide" {
            true => SLIDE_EM * font_px * (1.0 - e),
            false => 0.0,
        },
    }
}

/// The video rect inside the canvas, in canvas pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VideoRect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// The backing pill, in canvas pixels. Drawn through the shape pass, so it is a
/// rect plus a radius rather than its own pass.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CaptionPill {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub radius: f32,
    pub color: Srgba,
}

/// One frame's caption: the pill behind it and the glyphs on top.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CaptionFrame {
    pub pill: Option<CaptionPill>,
    pub glyphs: Vec<GlyphQuad>,
}

impl CaptionFrame {
    pub fn is_empty(&self) -> bool {
        self.pill.is_none() && self.glyphs.is_empty()
    }
}

/// Lays out the caption visible at SOURCE time `t`, packing whatever glyphs it needs into `atlas`.
/// Placement, chunking, line breaking and per-word colour all come from `recast-captions`, the same functions the ASS burn-in calls, so this only turns shaped lines into quads.
#[expect(
    clippy::too_many_arguments,
    reason = "the caption layout inputs: style, words, clock, geometry, face and atlas are all independent"
)]
pub fn layout_caption(
    style: &CaptionStyle,
    track: &CaptionTrack,
    clock: CaptionClock<'_>,
    video: VideoRect,
    canvas: (u32, u32),
    face: &FontFace,
    face_id: u32,
    atlas: &mut GlyphAtlas,
) -> CaptionFrame {
    let canvas_h = canvas.1.max(1) as f64;
    let font_px = style.font_size_pct / 100.0 * canvas_h;
    if !style.enabled || track.is_empty() || font_px <= 0.0 {
        return CaptionFrame::default();
    }

    // Cue-scoped: chunking the whole track runs a chunk across a sentence boundary and never ends.
    let Some(cue) = active_cue(&track.segments, clock.source) else {
        return CaptionFrame::default();
    };
    let anim = resolved_animation(style);
    let runs = chunk_words(&cue.words, &anim);
    let Some(index) = active_chunk_index(&runs, clock.source) else {
        return CaptionFrame::default();
    };
    let run = runs[index];
    if run.is_empty() || clock.source < run[0].start {
        return CaptionFrame::default();
    }
    let chunk_start_output = match clock.time_map {
        Some(map) => original_to_output(map, run[0].start),
        None => run[0].start,
    };
    let entrance = entrance_at(&anim, clock.output - chunk_start_output, font_px);
    if entrance.alpha <= 0.0 {
        return CaptionFrame::default();
    }

    let spacing_px = style.letter_spacing * font_px;
    let lines = break_into_lines(run, style.max_chars_per_line, style.max_lines);
    if lines.is_empty() {
        return CaptionFrame::default();
    }

    let active = active_word_index(run, clock.source, anim.hold_gaps);
    let spoken = spoken_word_count(run, clock.source);
    let shaped: Vec<Vec<ShapedWord>> = lines
        .iter()
        .map(|line| {
            line.iter()
                .map(|&i| {
                    let scale = match word_scaled(i, active, run.len(), &anim) {
                        true => EMPHASIS_SCALE,
                        false => 1.0,
                    };
                    ShapedWord {
                        index: i,
                        text: cased(&run[i].text, style.uppercase),
                        px: font_px * scale,
                    }
                })
                .collect()
        })
        .collect();

    let space_px = shape_line(face, font_px, " ", spacing_px).width;
    let line_widths: Vec<f64> = shaped
        .iter()
        .map(|line| line_width(face, line, spacing_px, space_px))
        .collect();
    let widest = line_widths.iter().copied().fold(0.0, f64::max);
    if widest <= 0.0 {
        return CaptionFrame::default();
    }

    let pill = pill_box(style, font_px, widest, shaped.len());
    let (block_x, block_y) = block_origin(style, &pill, video, canvas);

    let mut pending: Vec<PendingQuad> = Vec::new();

    let line_h = style.line_height * font_px;
    for (row, line) in shaped.iter().enumerate() {
        // Each line is centred on the widest one, which is what the pill hugs.
        let indent = (widest - line_widths[row]) * align_fraction(&style.align);
        let mut pen = block_x + pill.pad_x + indent;
        // The em box is line_height tall; the baseline sits on the ascender.
        let baseline = block_y
            + pill.pad_y
            + row as f64 * line_h
            + (line_h - font_px) / 2.0
            + face.metrics().ascender * font_px;

        for word in line {
            let colour = word_color(word.index, active, spoken, &anim, style);
            let rgba = parse_css_color(colour).unwrap_or(Srgba::opaque(255, 255, 255));
            push_word(
                &mut pending,
                face,
                face_id,
                atlas,
                word,
                pen,
                baseline,
                spacing_px,
                rgba,
            );
            pen += shape_line(face, word.px, &word.text, spacing_px).width + space_px;
        }
    }
    let mut frame = CaptionFrame {
        pill: backing_pill(style, block_x, block_y, &pill),
        glyphs: resolve_quads(pending, atlas),
    };
    apply_entrance(
        &mut frame,
        entrance,
        (block_x + pill.width / 2.0, block_y + pill.height / 2.0),
    );
    frame
}

/// Scales about the pill centre, shifts, and fades, the way the DOM painter
/// applies the same three as a canvas transform plus `globalAlpha`.
fn apply_entrance(frame: &mut CaptionFrame, entrance: Entrance, centre: (f64, f64)) {
    if entrance == Entrance::NONE {
        return;
    }
    let (cx, cy) = (centre.0 as f32, centre.1 as f32);
    let (scale, dy) = (entrance.scale as f32, entrance.dy as f32);
    let map = |x: f32, y: f32| ((x - cx) * scale + cx, (y - cy) * scale + cy + dy);
    if let Some(pill) = &mut frame.pill {
        let (x, y) = map(pill.x, pill.y);
        pill.x = x;
        pill.y = y;
        pill.w *= scale;
        pill.h *= scale;
        pill.radius *= scale;
        pill.color.a = (pill.color.a as f32 * entrance.alpha).round() as u8;
    }
    for quad in &mut frame.glyphs {
        let (x, y) = map(quad.rect[0], quad.rect[1]);
        quad.rect = [x, y, quad.rect[2] * scale, quad.rect[3] * scale];
        quad.colour[3] *= entrance.alpha;
    }
}

struct ShapedWord {
    index: usize,
    text: String,
    px: f64,
}

/// An absent animation predates progressive highlight, so it resolves to `active` rather than to the current default. Mirrors `resolveCaptionAnimation`.
fn resolved_animation(style: &CaptionStyle) -> CaptionAnimation {
    match &style.animation {
        Some(anim) => {
            let mut anim = anim.clone();
            anim.highlight = Some(anim.highlight().to_string());
            anim
        }
        None => CaptionAnimation::default(),
    }
}

fn cased(text: &str, uppercase: bool) -> String {
    match uppercase {
        true => text.to_uppercase(),
        false => text.to_string(),
    }
}

fn line_width(face: &FontFace, line: &[ShapedWord], spacing_px: f64, space_px: f64) -> f64 {
    if line.is_empty() {
        return 0.0;
    }
    let words: f64 = line
        .iter()
        .map(|w| shape_line(face, w.px, &w.text, spacing_px).width)
        .sum();
    words + space_px * (line.len() - 1) as f64
}

fn align_fraction(align: &str) -> f64 {
    match align {
        "left" => 0.0,
        "right" => 1.0,
        _ => 0.5,
    }
}

/// Top-left of the pill, from the shared vertical placement plus the horizontal
/// alignment the export uses (a 4% inset from the video edge).
fn block_origin(
    style: &CaptionStyle,
    pill: &recast_captions::PillBox,
    video: VideoRect,
    canvas: (u32, u32),
) -> (f64, f64) {
    let canvas_h = canvas.1.max(1) as f64;
    let v_top = video.y / canvas_h;
    let v_bottom = (video.y + video.h) / canvas_h;
    // The actual pill height, which is tighter than the max-lines estimate the auto-box path has to use.
    let cap = pill.height / canvas_h;
    let y = match caption_top_frac(&style.position, style.offset_pct, cap, v_top, v_bottom) {
        Some(frac) => frac * canvas_h,
        None => (v_top + v_bottom) / 2.0 * canvas_h - pill.height / 2.0,
    };

    let inset = video.w * 0.04;
    let x = match style.align.as_str() {
        "left" => video.x + inset,
        "right" => video.x + video.w - inset - pill.width,
        _ => video.x + (video.w - pill.width) / 2.0,
    };
    (x.max(video.x.max(0.0)), y)
}

fn backing_pill(
    style: &CaptionStyle,
    x: f64,
    y: f64,
    pill: &recast_captions::PillBox,
) -> Option<CaptionPill> {
    if style.background != "box" {
        return None;
    }
    let base = parse_css_color(&style.background_color)?;
    let alpha = (style.background_opacity / 100.0).clamp(0.0, 1.0);
    Some(CaptionPill {
        x: x as f32,
        y: y as f32,
        w: pill.width as f32,
        h: pill.height as f32,
        radius: pill.radius as f32,
        color: Srgba {
            a: (base.a as f64 * alpha).round() as u8,
            ..base
        },
    })
}

#[expect(
    clippy::too_many_arguments,
    reason = "one glyph run: pen state, face, style and atlas each vary independently"
)]
fn push_word(
    pending: &mut Vec<PendingQuad>,
    face: &FontFace,
    face_id: u32,
    atlas: &mut GlyphAtlas,
    word: &ShapedWord,
    pen: f64,
    baseline: f64,
    spacing_px: f64,
    colour: Srgba,
) {
    let shaped = shape_line(face, word.px, &word.text, spacing_px);
    let rgba = [
        colour.r as f32 / 255.0,
        colour.g as f32 / 255.0,
        colour.b as f32 / 255.0,
        colour.a as f32 / 255.0,
    ];
    for glyph in &shaped.glyphs {
        let Some(placed) = atlas.insert(face_id, face, glyph.id, word.px) else {
            continue;
        };
        pending.push(PendingQuad {
            x: (pen + glyph.x + placed.left as f64) as f32,
            y: (baseline + glyph.y + placed.top as f64) as f32,
            placed,
            colour: rgba,
        });
    }
}

/// A placed glyph awaiting its uv. Packing another glyph can grow the atlas, which changes the height every uv is divided by, so they are all resolved once the last insert is done.
struct PendingQuad {
    x: f32,
    y: f32,
    placed: recast_text::AtlasGlyph,
    colour: [f32; 4],
}

fn resolve_quads(pending: Vec<PendingQuad>, atlas: &GlyphAtlas) -> Vec<GlyphQuad> {
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
