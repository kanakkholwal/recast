use crate::model::{CaptionAnimation, CaptionStyle, TranscriptWord};

/// Words considered spoken at source-time `t`. A word counts from the moment
/// `t` reaches its `start`, matching how ASS `\k` flips a syllable at its
/// boundary, and it stays counted, so a gap never un-highlights an earlier word.
pub fn spoken_word_count(words: &[TranscriptWord], t: f64) -> usize {
    let mut n = 0;
    for (i, w) in words.iter().enumerate() {
        if t >= w.start {
            n = i + 1;
        } else {
            break;
        }
    }
    n
}

/// Per-word centisecond durations for ASS `\k`, in chunk order.
///
/// Rounds the CUMULATIVE boundary and diffs consecutive boundaries, so the sum
/// equals the rounded total exactly; rounding each word on its own accumulates
/// drift across a line. Each word holds until the next one starts.
pub fn karaoke_centiseconds(words: &[TranscriptWord], chunk_start: f64) -> Vec<i64> {
    let cs = |s: f64| ((s - chunk_start) * 100.0).round() as i64;
    let mut durations = Vec::with_capacity(words.len());
    let mut previous = 0i64;
    for (i, w) in words.iter().enumerate() {
        let next_start = words.get(i + 1).map(|n| n.start).unwrap_or(w.end);
        let boundary = previous.max(cs(next_start));
        durations.push(boundary - previous);
        previous = boundary;
    }
    durations
}

/// Hex colour for a word: the active word wins the accent under `color`
/// emphasis, otherwise progressive highlight paints spoken words base and
/// unspoken muted, and `none`/`active` paint every word the base colour.
pub fn word_color<'a>(
    index: usize,
    active: Option<usize>,
    spoken: usize,
    anim: &'a CaptionAnimation,
    style: &'a CaptionStyle,
) -> &'a str {
    if Some(index) == active && anim.emphasis == "color" {
        return &anim.emphasis_color;
    }
    if anim.highlight() == "progressive" {
        return if index < spoken {
            &style.color
        } else {
            &style.muted_color
        };
    }
    &style.color
}

/// Whether word `index` scales up. Suppressed for a lone word, whose pop
/// entrance already carries the emphasis.
pub fn word_scaled(
    index: usize,
    active: Option<usize>,
    word_count: usize,
    anim: &CaptionAnimation,
) -> bool {
    anim.emphasis == "scale" && Some(index) == active && word_count > 1
}
