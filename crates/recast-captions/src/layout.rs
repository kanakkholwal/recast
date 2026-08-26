use crate::model::{CaptionAnimation, CaptionStyle, TranscriptWord};

/// Largest caption block height ever reserved, as a fraction of the frame, so
/// the clamp cannot push a caption past the frame centre.
pub const MAX_CAP_FRAC: f64 = 0.7;

/// Line height plus breathing room, for the block-height estimate only.
const LINE_FACTOR: f64 = 1.35;

/// Estimated caption block height as a fraction of frame height. Uses
/// `max_lines` as an upper bound so the clamp reserves room for the tallest case.
pub fn caption_height_frac(font_size_pct: f64, max_lines: u32) -> f64 {
    (font_size_pct / 100.0 * max_lines.max(1) as f64 * LINE_FACTOR).min(MAX_CAP_FRAC)
}

/// Fraction-from-top of the caption block's top edge, the block growing
/// downward. `None` means centre, which the caller resolves against the video.
///
/// The baseline is the clamped ON-FRAME edge rather than the raw video edge:
/// anchoring on the raw edge left the whole positive offset range dead-clamped
/// whenever the video reached the frame edge.
pub fn caption_top_frac(
    position: &str,
    offset_pct: f64,
    cap: f64,
    v_top: f64,
    v_bottom: f64,
) -> Option<f64> {
    if position == "center" {
        return None;
    }
    // Signed: positive moves the caption inward over the video, negative tucks
    // it outward into the padding.
    let offset = offset_pct / 100.0;
    let cap = cap.clamp(0.0, MAX_CAP_FRAC);
    let max_top = (1.0 - cap).max(0.0);
    if position == "bottom" {
        let base = v_bottom.min(max_top);
        Some((base - offset).clamp(0.0, max_top))
    } else {
        let base = (v_top - cap).max(0.0);
        Some((base + offset).clamp(0.0, max_top))
    }
}

/// Groups `words` into the runs shown on screen at once.
pub fn chunk_words<'a>(
    words: &'a [TranscriptWord],
    anim: &CaptionAnimation,
) -> Vec<&'a [TranscriptWord]> {
    if words.is_empty() {
        return Vec::new();
    }
    let size = match anim.chunk.as_str() {
        "line" => words.len(),
        "word" => 1,
        _ => (anim.chunk_size as usize).max(1),
    };
    words.chunks(size).collect()
}

/// Greedy line break by character count, never splitting inside a word and
/// capped at `max_lines`. Returns groups of indices into `words`.
///
/// Measurement-free on purpose: the DOM and libass shape text differently, so a
/// break derived from either would drift. Both honour the break decided here.
pub fn break_into_lines(words: &[TranscriptWord], max_chars: u32, max_lines: u32) -> Vec<Vec<usize>> {
    let limit = max_chars.max(1) as usize;
    let cap = max_lines.max(1) as usize;
    let mut lines: Vec<Vec<usize>> = Vec::new();
    let mut current: Vec<usize> = Vec::new();
    let mut current_len = 0usize;
    for (i, w) in words.iter().enumerate() {
        let word_len = w.text.chars().count();
        let added = if current.is_empty() {
            word_len
        } else {
            current_len + 1 + word_len
        };
        if !current.is_empty() && added > limit {
            lines.push(std::mem::take(&mut current));
            current.push(i);
            current_len = word_len;
        } else {
            current.push(i);
            current_len = added;
        }
        if lines.len() == cap {
            break;
        }
    }
    if !current.is_empty() && lines.len() < cap {
        lines.push(current);
    }
    lines.truncate(cap);
    lines
}

/// Pill geometry from the style plus a measured text width. No measurement of
/// its own: the caller supplies `text_width_px`, so the DOM and the compositor
/// derive the same box from their own shapers.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PillBox {
    pub width: f64,
    pub height: f64,
    pub radius: f64,
    pub pad_x: f64,
    pub pad_y: f64,
}

pub fn pill_box(style: &CaptionStyle, font_px: f64, text_width_px: f64, line_count: usize) -> PillBox {
    let pad_x = style.box_padding_x_em * font_px;
    let pad_y = style.box_padding_y_em * font_px;
    let lines = line_count.max(1) as f64;
    let height = lines * style.line_height * font_px + 2.0 * pad_y;
    PillBox {
        width: text_width_px + 2.0 * pad_x,
        height,
        radius: (style.box_radius_em * font_px).min(height / 2.0).max(0.0),
        pad_x,
        pad_y,
    }
}
