//! Subtitle serialization from a transcript: SRT / WebVTT sidecars, plus ASS
//! for the FFmpeg burn-in path (libass renders the styled overlay into pixels).

use super::{CaptionAnimation, CaptionStyle, Transcript, TranscriptWord};

pub fn to_srt(t: &Transcript) -> String {
    let mut out = String::new();
    for (i, seg) in t.segments.iter().enumerate() {
        out.push_str(&format!("{}\n", i + 1));
        out.push_str(&format!(
            "{} --> {}\n",
            ts(seg.start, ','),
            ts(seg.end, ',')
        ));
        out.push_str(seg.text.trim());
        out.push_str("\n\n");
    }
    out
}

pub fn to_vtt(t: &Transcript) -> String {
    let mut out = String::from("WEBVTT\n\n");
    for seg in &t.segments {
        out.push_str(&format!(
            "{} --> {}\n",
            ts(seg.start, '.'),
            ts(seg.end, '.')
        ));
        out.push_str(seg.text.trim());
        out.push_str("\n\n");
    }
    out
}

/// The video rectangle inside the output canvas (source pixels). Captions are
/// placed relative to it so they sit in the padding, not over the video.
#[derive(Debug, Clone, Copy)]
pub struct VideoRectPx {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

const MAX_CAP_FRAC: f64 = 0.7;

/// Estimated caption block height as a fraction of frame height. Mirror of
/// `captionHeightFrac` in $lib/captions/layout.ts — keep them in sync.
fn caption_height_frac(font_size_pct: f64, max_lines: u32) -> f64 {
    (font_size_pct / 100.0 * max_lines.max(1) as f64 * 1.35).min(MAX_CAP_FRAC)
}

/// Fraction-from-top of the caption block's TOP edge (grows down). `None` =
/// centre. Mirror of `captionTopFrac` in $lib/captions/layout.ts.
fn caption_top_frac(
    position: &str,
    offset_pct: f64,
    cap: f64,
    v_top: f64,
    v_bottom: f64,
) -> Option<f64> {
    if position == "center" {
        return None;
    }
    // Signed: + pushes outward into the padding, - pulls back onto the video.
    let offset = offset_pct / 100.0;
    let cap = cap.clamp(0.0, MAX_CAP_FRAC);
    let max_top = (1.0 - cap).max(0.0);
    if position == "bottom" {
        Some((v_bottom + offset).min(max_top))
    } else {
        Some((v_top - offset - cap).clamp(0.0, max_top))
    }
}

/// Render a transcript to an ASS subtitle script for FFmpeg's `ass`/`subtitles`
/// burn-in filter, styled from `CaptionStyle`. `play_w`/`play_h` are the canvas
/// dimensions captions are laid out against (the composite size, pre-downscale),
/// so font size / margins resolve in the same pixel space as the preview.
/// `video` is the source-video rect inside that canvas — captions are placed in
/// the padding relative to it. `offset` is the trim start (seconds): burn-in is
/// injected before the cut/speed stage, so times are on the trimmed-but-uncut
/// axis and the later select/setpts re-times the burned pixels. `clip_len` caps
/// the output.
#[allow(clippy::too_many_arguments)]
pub fn to_ass(
    t: &Transcript,
    style: &CaptionStyle,
    play_w: u32,
    play_h: u32,
    video: VideoRectPx,
    offset: f64,
    clip_len: f64,
    embedded_font: bool,
) -> String {
    let font = ass_font_name(&style.font_family);
    let font_size = (style.font_size_pct / 100.0 * play_h as f64).max(8.0);
    // An embedded font ships at its exact weight, so don't let libass synthesize
    // bold on top of it (double-bolding). For a fallback system face, only bold
    // from 700+ (600 = semibold should read as regular, not heavy).
    let bold = if !embedded_font && style.font_weight >= 700 {
        -1
    } else {
        0
    };
    let spacing = style.letter_spacing * font_size;
    let outline_px = (style.outline_width / 100.0 * font_size).max(0.0);

    let primary = ass_color(&style.color, 0.0);
    let outline_col = ass_color(&style.outline_color, 0.0);
    // ASS BackColour alpha: 00 = opaque, FF = transparent (inverse of our %).
    let back_col = ass_color(&style.background_color, 100.0 - style.background_opacity);

    let (border_style, outline, shadow) = match style.background.as_str() {
        "box" => (3, outline_px.max(font_size * 0.08), 0.0),
        "soft" => (1, outline_px, (font_size * 0.04).max(1.5)),
        _ => (1, outline_px, 0.0),
    };

    // Place captions relative to the VIDEO rect, not the frame: top/bottom sit
    // in the padding outside the video (mirrors `captionTopFrac` in
    // $lib/captions/layout.ts). We anchor the caption's TOP and let it grow down,
    // so both use the ASS top band (7-9); centre uses the middle band (4-6),
    // which libass auto-centres on the frame == the (centred) video.
    let v_top = video.y as f64 / play_h.max(1) as f64;
    let v_bottom = (video.y + video.h) as f64 / play_h.max(1) as f64;
    let cap = caption_height_frac(style.font_size_pct, style.max_lines);
    let h_offset = match style.align.as_str() {
        "left" => 0,
        "right" => 2,
        _ => 1,
    };
    let (band, margin_v) =
        match caption_top_frac(&style.position, style.offset_pct, cap, v_top, v_bottom) {
            None => (4, 0),
            Some(top_frac) => (7, (top_frac * play_h as f64).round() as i32),
        };
    let alignment = band + h_offset;
    // Constrain the text box to the video's horizontal extent (+ a small inset)
    // so captions line up with the video content, not the letterbox bars.
    let inset = (video.w as f64 * 0.04).round() as i32;
    let margin_l = video.x as i32 + inset;
    let margin_r = (play_w as i32 - (video.x + video.w) as i32).max(0) + inset;

    let mut out = String::new();
    out.push_str("[Script Info]\n");
    out.push_str("ScriptType: v4.00+\n");
    out.push_str("WrapStyle: 0\n");
    out.push_str("ScaledBorderAndShadow: yes\n");
    out.push_str(&format!("PlayResX: {play_w}\nPlayResY: {play_h}\n\n"));

    out.push_str("[V4+ Styles]\n");
    out.push_str(
        "Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, \
BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, \
Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding\n",
    );
    out.push_str(&format!(
        "Style: Default,{font},{size:.0},{primary},{primary},{outline_col},{back_col},{bold},0,0,0,\
100,100,{spacing:.1},0,{border_style},{outline:.1},{shadow:.1},{alignment},{margin_l},{margin_r},{margin_v},1\n\n",
        size = font_size,
    ));

    out.push_str("[Events]\n");
    out.push_str(
        "Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\n",
    );

    let anim = style.animation.clone().unwrap_or_default();
    if anim.is_static() {
        for seg in &t.segments {
            push_dialogue(
                &mut out,
                seg.start,
                seg.end,
                offset,
                clip_len,
                "",
                &ass_text(&seg.text, style.uppercase),
            );
        }
    } else {
        for seg in &t.segments {
            emit_animated_segment(&mut out, seg, style, &anim, offset, clip_len);
        }
    }
    out
}

/// Append one Dialogue line, mapping source times onto the trimmed-but-uncut
/// axis (subtract `offset`, clamp to `clip_len`). Skips fully out-of-range
/// events. `prefix` is an optional leading ASS override block (entrance tags).
#[allow(clippy::too_many_arguments)]
fn push_dialogue(
    out: &mut String,
    src_start: f64,
    src_end: f64,
    offset: f64,
    clip_len: f64,
    prefix: &str,
    text: &str,
) {
    let start = (src_start - offset).max(0.0);
    let mut end = src_end - offset;
    if end <= 0.0 || start >= clip_len {
        return;
    }
    if clip_len > 0.0 {
        end = end.min(clip_len);
    }
    if end <= start {
        return;
    }
    out.push_str(&format!(
        "Dialogue: 0,{},{},Default,,0,0,0,,{}{}\n",
        ass_time(start),
        ass_time(end),
        prefix,
        text,
    ));
}

/// Group a line's words into display chunks — mirrors `chunkWords` in
/// `$lib/captions/animation.ts`. Keep the two in sync.
fn chunk_words<'a>(
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

/// Emit the ASS events for one segment under an animation spec. Each display
/// chunk is held until the next chunk starts (so single-word styles never blink
/// to empty); with active-word emphasis, the chunk is split into one sub-event
/// per word-active window, the first carrying the entrance.
fn emit_animated_segment(
    out: &mut String,
    seg: &super::TranscriptSegment,
    style: &CaptionStyle,
    anim: &CaptionAnimation,
    offset: f64,
    clip_len: f64,
) {
    if seg.words.is_empty() {
        // No per-word timing: animate the whole line as one chunk.
        push_dialogue(
            out,
            seg.start,
            seg.end,
            offset,
            clip_len,
            &entrance_tag(anim),
            &ass_text(&seg.text, style.uppercase),
        );
        return;
    }

    let runs = chunk_words(&seg.words, anim);
    let base_col = ass_primary(&style.color);
    let emph_col = ass_primary(&anim.emphasis_color);
    let emphasized = anim.emphasis == "color" || anim.emphasis == "scale";

    for (i, run) in runs.iter().enumerate() {
        let ds = run[0].start;
        // Hold the chunk until the next chunk starts (last chunk → segment end).
        let de = if i + 1 < runs.len() {
            runs[i + 1][0].start
        } else {
            seg.end
        };

        if !emphasized {
            push_dialogue(
                out,
                ds,
                de,
                offset,
                clip_len,
                &entrance_tag(anim),
                &run_text(run, None, anim, style, &base_col, &emph_col),
            );
            continue;
        }

        for j in 0..run.len() {
            let ws = if j == 0 { ds } else { run[j].start };
            let we = if j + 1 < run.len() {
                run[j + 1].start
            } else {
                de
            };
            let prefix = if j == 0 {
                entrance_tag(anim)
            } else {
                String::new()
            };
            push_dialogue(
                out,
                ws,
                we,
                offset,
                clip_len,
                &prefix,
                &run_text(run, Some(j), anim, style, &base_col, &emph_col),
            );
        }
    }
}

/// The run's words joined with spaces, wrapping the active word in emphasis
/// override tags (colour swap or inline scale).
fn run_text(
    run: &[TranscriptWord],
    active: Option<usize>,
    anim: &CaptionAnimation,
    style: &CaptionStyle,
    base_col: &str,
    emph_col: &str,
) -> String {
    run.iter()
        .enumerate()
        .map(|(j, w)| {
            let txt = ass_text(&w.text, style.uppercase);
            if Some(j) == active {
                match anim.emphasis.as_str() {
                    "color" => format!("{{\\c{emph_col}}}{txt}{{\\c{base_col}}}"),
                    // Scaling a single-word chunk fights the pop entrance, so only
                    // emphasize size when the word sits among others (parity with
                    // the preview's `view.words.length > 1` guard).
                    "scale" if run.len() > 1 => {
                        format!("{{\\fscx114\\fscy114}}{txt}{{\\fscx100\\fscy100}}")
                    }
                    _ => txt,
                }
            } else {
                txt
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Leading override block for a chunk's entrance, or empty for `none`. `slide`
/// falls back to a fade (true slide needs `\pos`/`\move` math against the
/// alignment anchor; none of the shipped presets use it).
fn entrance_tag(anim: &CaptionAnimation) -> String {
    let ms = anim.entrance_ms.max(0.0).round() as i64;
    if ms == 0 {
        return String::new();
    }
    match anim.entrance.as_str() {
        "fade" | "slide" => format!("{{\\fad({ms},0)}}"),
        "pop" => format!("{{\\fad({ms},0)\\fscx60\\fscy60\\t(0,{ms},\\fscx100\\fscy100)}}"),
        _ => String::new(),
    }
}

/// `#RRGGBB` → inline ASS colour literal `&HBBGGRR&` (no alpha).
fn ass_primary(hex: &str) -> String {
    let h = hex.trim_start_matches('#');
    let r = u8::from_str_radix(h.get(0..2).unwrap_or("ff"), 16).unwrap_or(255);
    let g = u8::from_str_radix(h.get(2..4).unwrap_or("ff"), 16).unwrap_or(255);
    let b = u8::from_str_radix(h.get(4..6).unwrap_or("ff"), 16).unwrap_or(255);
    format!("&H{b:02X}{g:02X}{r:02X}&")
}

/// First family of a CSS stack, unquoted (e.g. `'Anton', sans-serif` → `Anton`).
pub(crate) fn first_family(stack: &str) -> String {
    stack
        .split(',')
        .next()
        .unwrap_or(stack)
        .trim()
        .trim_matches(|c| c == '\'' || c == '"')
        .trim()
        .to_string()
}

/// True for a generic / built-in face we never fetch from Google (so the export
/// path skips font embedding and lets libass use its own fallback).
pub(crate) fn is_system_family(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "" | "system-ui"
            | "sans-serif"
            | "serif"
            | "monospace"
            | "arial"
            | "georgia"
            | "impact"
            | "courier new"
            | "times new roman"
            | "arial narrow bold"
    )
}

/// First family of a CSS stack, with web generics mapped to a font libass can
/// resolve on the host (`system-ui` → Arial, etc.).
fn ass_font_name(stack: &str) -> String {
    match first_family(stack).as_str() {
        "system-ui" | "sans-serif" | "" => "Arial".to_string(),
        "serif" => "Times New Roman".to_string(),
        "monospace" => "Courier New".to_string(),
        other => other.to_string(),
    }
}

/// `#RRGGBB` + 0–100 transparency → ASS `&HAABBGGRR` (AA: 00 opaque, FF clear).
fn ass_color(hex: &str, transparency_pct: f64) -> String {
    let h = hex.trim_start_matches('#');
    let r = u8::from_str_radix(h.get(0..2).unwrap_or("ff"), 16).unwrap_or(255);
    let g = u8::from_str_radix(h.get(2..4).unwrap_or("ff"), 16).unwrap_or(255);
    let b = u8::from_str_radix(h.get(4..6).unwrap_or("ff"), 16).unwrap_or(255);
    let a = (transparency_pct.clamp(0.0, 100.0) / 100.0 * 255.0).round() as u8;
    format!("&H{a:02X}{b:02X}{g:02X}{r:02X}")
}

/// `H:MM:SS.cc` (centiseconds) — the ASS time format.
fn ass_time(seconds: f64) -> String {
    let total_cs = (seconds.max(0.0) * 100.0).round() as u64;
    let cs = total_cs % 100;
    let total_s = total_cs / 100;
    format!(
        "{}:{:02}:{:02}.{:02}",
        total_s / 3600,
        (total_s / 60) % 60,
        total_s % 60,
        cs
    )
}

/// Sanitize segment text for an ASS Dialogue line: strip override braces, fold
/// hard newlines to `\N`, optionally uppercase.
fn ass_text(text: &str, uppercase: bool) -> String {
    let cleaned = text.trim().replace(['{', '}'], "");
    let joined = cleaned
        .split(['\n', '\r'])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("\\N");
    if uppercase {
        joined.to_uppercase()
    } else {
        joined
    }
}

/// `HH:MM:SS<sep>mmm` — SRT uses `,` before milliseconds, WebVTT uses `.`.
fn ts(seconds: f64, sep: char) -> String {
    let total_ms = (seconds.max(0.0) * 1000.0).round() as u64;
    let ms = total_ms % 1000;
    let total_s = total_ms / 1000;
    format!(
        "{:02}:{:02}:{:02}{}{:03}",
        total_s / 3600,
        (total_s / 60) % 60,
        total_s % 60,
        sep,
        ms
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transcription::{CaptionAnimation, CaptionStyle, TranscriptSegment};

    /// Full-frame video rect (no padding) for tests that don't exercise layout.
    fn full_vr() -> VideoRectPx {
        VideoRectPx {
            x: 0,
            y: 0,
            w: 1920,
            h: 1080,
        }
    }

    fn words(spec: &[(f64, f64, &str)]) -> Vec<TranscriptWord> {
        spec.iter()
            .map(|(s, e, t)| TranscriptWord {
                start: *s,
                end: *e,
                text: t.to_string(),
            })
            .collect()
    }

    fn transcript(ws: Vec<TranscriptWord>) -> Transcript {
        let start = ws.first().map(|w| w.start).unwrap_or(0.0);
        let end = ws.last().map(|w| w.end).unwrap_or(0.0);
        let text = ws
            .iter()
            .map(|w| w.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        Transcript {
            engine: "t".into(),
            model_id: "m".into(),
            language: None,
            segments: vec![TranscriptSegment {
                id: "seg-0".into(),
                start,
                end,
                text,
                words: ws,
            }],
        }
    }

    fn dialogues(ass: &str) -> Vec<&str> {
        ass.lines().filter(|l| l.starts_with("Dialogue:")).collect()
    }

    fn anim(chunk: &str, emphasis: &str, entrance: &str) -> CaptionAnimation {
        CaptionAnimation {
            chunk: chunk.into(),
            emphasis: emphasis.into(),
            entrance: entrance.into(),
            ..Default::default()
        }
    }

    #[test]
    fn static_animation_emits_one_event_per_segment() {
        let style = CaptionStyle {
            animation: None,
            ..Default::default()
        };
        let t = transcript(words(&[(0.0, 0.5, "hello"), (0.5, 1.0, "world")]));
        let ass = to_ass(&t, &style, 1920, 1080, full_vr(), 0.0, 10.0, false);
        assert_eq!(dialogues(&ass).len(), 1);
        assert!(ass.contains("hello world"));
    }

    #[test]
    fn word_chunk_pop_emits_event_per_word_with_entrance() {
        let style = CaptionStyle {
            animation: Some(anim("word", "none", "pop")),
            ..Default::default()
        };
        let t = transcript(words(&[(0.0, 0.5, "a"), (0.5, 1.0, "b"), (1.0, 1.5, "c")]));
        let ass = to_ass(&t, &style, 1920, 1080, full_vr(), 0.0, 10.0, false);
        assert_eq!(dialogues(&ass).len(), 3);
        assert!(ass.contains("\\fad("));
        assert!(ass.contains("\\t(0,"));
    }

    #[test]
    fn color_emphasis_wraps_active_word_in_accent() {
        let style = CaptionStyle {
            color: "#ffffff".into(),
            animation: Some(CaptionAnimation {
                emphasis_color: "#facc15".into(),
                ..anim("line", "color", "none")
            }),
            ..Default::default()
        };
        let t = transcript(words(&[(0.0, 0.5, "one"), (0.5, 1.0, "two")]));
        let ass = to_ass(&t, &style, 1920, 1080, full_vr(), 0.0, 10.0, false);
        // One line chunk, but colour emphasis splits it per word → 2 sub-events.
        assert_eq!(dialogues(&ass).len(), 2);
        // #facc15 → BGR &H15CCFA& accent, resetting to white base.
        assert!(ass.to_lowercase().contains("&h15ccfa&"));
        assert!(ass.to_lowercase().contains("&hffffff&"));
    }

    #[test]
    fn phrase_chunks_group_words() {
        let style = CaptionStyle {
            animation: Some(CaptionAnimation {
                chunk_size: 2,
                ..anim("phrase", "none", "fade")
            }),
            ..Default::default()
        };
        let t = transcript(words(&[
            (0.0, 0.5, "a"),
            (0.5, 1.0, "b"),
            (1.0, 1.5, "c"),
            (1.5, 2.0, "d"),
            (2.0, 2.5, "e"),
        ]));
        let ass = to_ass(&t, &style, 1920, 1080, full_vr(), 0.0, 10.0, false);
        // 5 words / 2 per chunk = 3 chunks, no emphasis → 3 events.
        assert_eq!(dialogues(&ass).len(), 3);
    }

    #[test]
    fn events_respect_offset_and_clip() {
        let style = CaptionStyle {
            animation: Some(anim("word", "none", "none")),
            ..Default::default()
        };
        let t = transcript(words(&[(2.0, 2.5, "a"), (2.5, 3.0, "b")]));
        // offset 1 shifts to [1,1.5]/[1.5,2]; clip 1.4 drops the second entirely.
        let ass = to_ass(&t, &style, 1920, 1080, full_vr(), 1.0, 1.4, false);
        assert_eq!(dialogues(&ass).len(), 1);
    }

    fn style_bold(ass: &str) -> i32 {
        // Style line: ...,BackColour,Bold,Italic,... → Bold is the field after the
        // 7th comma of the "Style: Default,..." line.
        let line = ass
            .lines()
            .find(|l| l.starts_with("Style: Default,"))
            .unwrap();
        line.split(',').nth(7).unwrap().trim().parse().unwrap()
    }

    #[test]
    fn embedded_font_never_synthesizes_bold() {
        let t = transcript(words(&[(0.0, 0.5, "hi")]));
        // Embedded at heavy weight → Bold 0 (the TTF already carries the weight).
        let heavy = CaptionStyle {
            font_weight: 800,
            animation: None,
            ..Default::default()
        };
        assert_eq!(
            style_bold(&to_ass(&t, &heavy, 1920, 1080, full_vr(), 0.0, 10.0, true)),
            0
        );
        // Fallback face: bold only from 700+, so 600 (semibold) stays regular.
        let semibold = CaptionStyle {
            font_weight: 600,
            animation: None,
            ..Default::default()
        };
        assert_eq!(
            style_bold(&to_ass(
                &t,
                &semibold,
                1920,
                1080,
                full_vr(),
                0.0,
                10.0,
                false
            )),
            0
        );
        let bold = CaptionStyle {
            font_weight: 700,
            animation: None,
            ..Default::default()
        };
        assert_eq!(
            style_bold(&to_ass(&t, &bold, 1920, 1080, full_vr(), 0.0, 10.0, false)),
            -1
        );
    }

    fn style_alignment(ass: &str) -> i32 {
        let line = ass
            .lines()
            .find(|l| l.starts_with("Style: Default,"))
            .unwrap();
        line.split(',').nth(18).unwrap().trim().parse().unwrap()
    }

    #[test]
    fn caption_top_frac_places_bottom_in_padding_and_clamps() {
        let cap = 0.12;
        // 15% padding top/bottom → block sits at/below the video's bottom edge.
        let top = caption_top_frac("bottom", 0.0, cap, 0.15, 0.85).unwrap();
        assert!(top >= 0.85 - 1e-9);
        assert!(top + cap <= 1.0 + 1e-9);
        // No padding → clamped so the caption stays on-frame (over the video).
        let full = caption_top_frac("bottom", 8.0, cap, 0.0, 1.0).unwrap();
        assert!((full - (1.0 - cap)).abs() < 1e-9);
        // Centre is handled by the middle band.
        assert!(caption_top_frac("center", 8.0, cap, 0.15, 0.85).is_none());
    }

    #[test]
    fn bottom_caption_uses_top_band_and_center_uses_middle() {
        let t = transcript(words(&[(0.0, 0.5, "hi")]));
        // 200px bottom padding: video is [0, 880] in a 1080 canvas.
        let video = VideoRectPx {
            x: 0,
            y: 0,
            w: 1920,
            h: 880,
        };
        let bottom = CaptionStyle {
            position: "bottom".into(),
            animation: None,
            ..Default::default()
        };
        // Bottom now anchors from the top band (7-9) so it grows down into padding.
        assert!((7..=9).contains(&style_alignment(&to_ass(
            &t, &bottom, 1920, 1080, video, 0.0, 10.0, false
        ))));
        let center = CaptionStyle {
            position: "center".into(),
            animation: None,
            ..Default::default()
        };
        assert!((4..=6).contains(&style_alignment(&to_ass(
            &t, &center, 1920, 1080, video, 0.0, 10.0, false
        ))));
    }
}
