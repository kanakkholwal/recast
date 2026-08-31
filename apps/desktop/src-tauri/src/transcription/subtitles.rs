//! Subtitle serialization from a transcript: SRT / WebVTT sidecars, plus ASS
//! for the FFmpeg burn-in path (libass renders the styled overlay into pixels).

use recast_captions::{
    break_into_lines, caption_height_frac, caption_top_frac, chunk_words, word_scaled,
};

use super::{CaptionAnimation, CaptionStyle, Transcript, TranscriptSegment, TranscriptWord};

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

/// WebVTT with word-level inline cue timestamps when the segment carries word
/// timing: the body leads with each word's `<HH:MM:SS.mmm>` tag so the web
/// player can drive progressive highlight. A player that ignores the tags still
/// shows the whole cue text, so this stays compatible with older recasts (and
/// segments without word timing fall back to plain text). Mirrors
/// `serializeKaraokeVtt` in @recast/captions.
pub fn to_vtt(t: &Transcript) -> String {
    let mut out = String::from("WEBVTT\n\n");
    for seg in &t.segments {
        out.push_str(&format!(
            "{} --> {}\n",
            ts(seg.start, '.'),
            ts(seg.end, '.')
        ));
        if seg.words.is_empty() {
            out.push_str(seg.text.trim());
        } else {
            for (i, w) in seg.words.iter().enumerate() {
                if i > 0 {
                    out.push(' ');
                }
                out.push_str(&format!("<{}>{}", ts(w.start, '.'), w.text.trim()));
            }
        }
        out.push_str("\n\n");
    }
    out
}

/// Convert an SRT subtitle string to WebVTT. VTT is SRT plus a `WEBVTT` header
/// and `.` (not `,`) before the milliseconds; cue index lines are valid VTT cue
/// identifiers, so they pass through untouched. Used to feed a sibling `.srt`
/// file into the `<track>` element, which only accepts WebVTT.
pub fn srt_to_vtt(srt: &str) -> String {
    let mut out = String::from("WEBVTT\n\n");
    for line in srt.lines() {
        if line.contains("-->") {
            out.push_str(&line.replace(',', "."));
        } else {
            out.push_str(line);
        }
        out.push('\n');
    }
    out
}

/// Read a caption sidecar sitting next to `media_path` (e.g. `foo.mp4` →
/// `foo.vtt` or `foo.srt`) and return it as WebVTT, or `None` when neither
/// exists. Prefers `.vtt` (already WebVTT); converts `.srt`. The export queue
/// writes these sidecars next to an export, so a shared/previewed file can carry
/// captions with no loaded project.
pub(crate) fn read_caption_sidecar(media_path: &std::path::Path) -> Option<String> {
    let vtt = media_path.with_extension("vtt");
    if let Ok(text) = std::fs::read_to_string(&vtt) {
        return Some(text);
    }
    let srt = media_path.with_extension("srt");
    if let Ok(text) = std::fs::read_to_string(&srt) {
        return Some(srt_to_vtt(&text));
    }
    None
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

/// The face `to_ass` should target, resolved by `text_measure` from the actual
/// font file (see that module for why the CSS family name and size are wrong for
/// libass). For a system fallback with no resolution, use `RenderFont::fallback`.
#[derive(Debug, Clone)]
pub struct RenderFont {
    /// ASS `Fontname` — the name libass matches on (legacy family / name ID 1).
    pub ass_name: String,
    /// True when the face is embedded via `fontsdir` (don't synthesize bold).
    pub embedded: bool,
    /// `Fontsize = css_px * ass_scale` so libass renders at the preview's pixel
    /// height (libass divides Fontsize by winAscent+winDescent, not upem).
    pub ass_scale: f64,
    /// The resolved face for measuring caption-line widths. `Some` enables the
    /// exact rounded pill for single-line box captions; `None` (fallback / tests)
    /// keeps the square `BorderStyle:3` auto box.
    pub measure: Option<super::text_measure::MeasureFace>,
}

impl RenderFont {
    /// Fallback when `text_measure` can't resolve the face (offline, no match):
    /// map the CSS stack to a libass-resolvable name, no size correction, no
    /// measurement (so the square auto box is used).
    pub fn fallback(style: &CaptionStyle) -> Self {
        Self {
            ass_name: ass_font_name(&style.font_family),
            embedded: false,
            ass_scale: 1.0,
            measure: None,
        }
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
#[expect(
    clippy::too_many_arguments,
    reason = "the ASS header needs every style and geometry input at once"
)]
pub fn to_ass(
    t: &Transcript,
    style: &CaptionStyle,
    play_w: u32,
    play_h: u32,
    video: VideoRectPx,
    offset: f64,
    clip_len: f64,
    font: &RenderFont,
) -> String {
    let font_name = &font.ass_name;
    // Rendered pixel height (the preview's `fontSizePct`cqh); all pixel geometry lives in this PlayRes space.
    let css_px = (style.font_size_pct / 100.0 * play_h as f64).max(8.0);
    // Corrected so libass's winAscent plus winDescent scaling lands the glyphs at `css_px` (see text_measure).
    let font_size = css_px * font.ass_scale;
    // An embedded font ships at its exact weight, so only synthesize bold for a fallback face at 700 or above.
    let bold = if !font.embedded && style.font_weight >= 700 {
        -1
    } else {
        0
    };
    let spacing = style.letter_spacing * css_px;
    let outline_px = (style.outline_width / 100.0 * css_px).max(0.0);

    let anim = style.animation.clone().unwrap_or_default();
    let primary = ass_color(&style.color, 0.0);
    // Words are coloured with inline `\c`, not karaoke, so SecondaryColour is only a sensible default.
    let outline_col = ass_color(&style.outline_color, 0.0);
    // ASS BackColour alpha: 00 = opaque, FF = transparent (inverse of our %).
    let back_col = ass_color(&style.background_color, 100.0 - style.background_opacity);

    let (border_style, outline, shadow) = match style.background.as_str() {
        "box" => (3, outline_px.max(css_px * 0.08), 0.0),
        "soft" => (1, outline_px, (css_px * 0.04).max(1.5)),
        _ => (1, outline_px, 0.0),
    };

    // Anchor the caption's TOP against the VIDEO rect (ASS band 7-9); centre uses 4-6, which libass centres on the frame.
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
    // Constrain to the video's horizontal extent so captions line up with content, not the letterbox bars.
    let inset = (video.w as f64 * 0.04).round() as i32;
    let margin_l = video.x as i32 + inset;
    let margin_r = (play_w as i32 - (video.x + video.w) as i32).max(0) + inset;

    let mut out = String::new();
    out.push_str("[Script Info]\n");
    out.push_str("ScriptType: v4.00+\n");
    out.push_str("WrapStyle: 0\n");
    out.push_str("ScaledBorderAndShadow: yes\n");
    // libass disables kerning unless asked, so without this the burn-in is subtly wider than the preview.
    out.push_str("Kerning: yes\n");
    out.push_str(&format!("PlayResX: {play_w}\nPlayResY: {play_h}\n\n"));

    out.push_str("[V4+ Styles]\n");
    out.push_str(
        "Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, \
BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, \
Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding\n",
    );
    out.push_str(&format!(
        "Style: Default,{font_name},{font_size:.0},{primary},{primary},{outline_col},{back_col},{bold},0,0,0,\
100,100,{spacing:.1},0,{border_style},{outline:.1},{shadow:.1},{alignment},{margin_l},{margin_r},{margin_v},1\n\n",
    ));

    out.push_str("[Events]\n");
    out.push_str(
        "Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\n",
    );

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
        let ctx = LayoutCtx {
            style,
            font,
            css_px,
            play_h,
            video,
        };
        for seg in &t.segments {
            emit_animated_segment(&mut out, &ctx, seg, &anim, offset, clip_len);
        }
    }
    out
}

/// Kept source-time spans of `[trim_start, trim_end]` with `cuts` removed.
/// `cuts` must be source-time, sorted and non-overlapping (what
/// `collect_export_cuts` produces, shifted back by `trim_start`).
pub fn kept_spans(trim_start: f64, trim_end: f64, cuts: &[(f64, f64)]) -> Vec<(f64, f64)> {
    let mut spans = Vec::new();
    let mut cursor = trim_start;
    for &(cut_start, cut_end) in cuts {
        if cut_end <= trim_start || cut_start >= trim_end {
            continue;
        }
        let lo = cut_start.max(trim_start);
        if lo - cursor > SPAN_EPS {
            spans.push((cursor, lo));
        }
        cursor = cursor.max(cut_end.min(trim_end));
    }
    if trim_end - cursor > SPAN_EPS {
        spans.push((cursor, trim_end));
    }
    spans
}

/// Matches the frontend's cut EPS so both sides agree on a boundary.
const SPAN_EPS: f64 = 1e-4;

/// Splits caption segments across kept spans, dropping the parts inside a cut. Mirrors `splitSegmentAcrossSpans`; keep the two in sync.
/// The cut stage alone stops a caption outlasting a cut but not chunking across it, which showed text for removed audio and broke chunks unlike the preview.
pub fn split_transcript_by_spans(t: &Transcript, spans: &[(f64, f64)]) -> Transcript {
    let mut segments = Vec::with_capacity(t.segments.len());
    for seg in &t.segments {
        let pieces: Vec<(f64, f64)> = spans
            .iter()
            .filter_map(|&(span_start, span_end)| {
                let start = seg.start.max(span_start);
                let end = seg.end.min(span_end);
                (end > start).then_some((start, end))
            })
            .collect();
        let split = pieces.len() > 1;
        for (i, (start, end)) in pieces.into_iter().enumerate() {
            let words: Vec<TranscriptWord> = seg
                .words
                .iter()
                .filter_map(|w| {
                    let ws = w.start.clamp(start, end);
                    let we = w.end.clamp(start, end);
                    (we > ws).then(|| TranscriptWord {
                        start: ws,
                        end: we,
                        text: w.text.clone(),
                    })
                })
                .collect();
            // A split piece is its own cue carrying the half actually spoken here; an unsplit segment keeps its identity.
            let text = if split && !words.is_empty() {
                words
                    .iter()
                    .map(|w| w.text.as_str())
                    .collect::<Vec<_>>()
                    .join(" ")
                    .trim()
                    .to_string()
            } else {
                seg.text.clone()
            };
            segments.push(TranscriptSegment {
                id: if split {
                    format!("{}:{i}", seg.id)
                } else {
                    seg.id.clone()
                },
                start,
                end,
                text,
                words,
            });
        }
    }
    Transcript {
        segments,
        ..t.clone()
    }
}

/// Append one Dialogue line on ASS layer 0, mapping source times onto the
/// trimmed-but-uncut axis. `prefix` is an optional leading ASS override block.
fn push_dialogue(
    out: &mut String,
    src_start: f64,
    src_end: f64,
    offset: f64,
    clip_len: f64,
    prefix: &str,
    text: &str,
) {
    push_dialogue_layer(out, 0, src_start, src_end, offset, clip_len, prefix, text);
}

/// Append one Dialogue line on `layer` (higher = drawn on top), mapping source
/// times onto the trimmed-but-uncut axis (subtract `offset`, clamp to
/// `clip_len`). Skips fully out-of-range events. The pill draws on layer 0, the
/// text on layer 1 so it sits on top.
#[expect(
    clippy::too_many_arguments,
    reason = "one dialogue line: layer, timing, style and text are independent"
)]
fn push_dialogue_layer(
    out: &mut String,
    layer: u32,
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
        "Dialogue: {layer},{},{},Default,,0,0,0,,{}{}\n",
        ass_time(start),
        ass_time(end),
        prefix,
        text,
    ));
}

/// Group a line's words into display chunks — mirrors `chunkWords` in
/// `$lib/captions/animation.ts`. Keep the two in sync.
/// Everything the pill/positioning math needs, built once in `to_ass`.
struct LayoutCtx<'a> {
    style: &'a CaptionStyle,
    font: &'a RenderFont,
    /// Rendered caption pixel height (= the preview's `fontSizePct`cqh).
    css_px: f64,
    play_h: u32,
    video: VideoRectPx,
}

/// Pixel geometry for a single-line rounded pill and the text inside it.
struct PillLayout {
    pill_x: f64,
    pill_y: f64,
    pill_w: f64,
    pill_h: f64,
    radius: f64,
    text_x: f64,
    text_y: f64,
}

impl LayoutCtx<'_> {
    /// Geometry for a rounded pill hugging a SINGLE line `text`, or `None` when
    /// the exact pill doesn't apply: not a box background, no measurable face
    /// (fallback / tests → keep the square auto box), or a zero-width measure.
    fn pill_for_line(&self, text: &str) -> Option<PillLayout> {
        if self.style.background != "box" {
            return None;
        }
        let face = self.font.measure.as_ref()?;
        let spacing_px = self.style.letter_spacing * self.css_px;
        let w = super::text_measure::measure_line_width(face, self.css_px, text, spacing_px);
        if w <= 0.0 {
            return None;
        }
        let pad_x = self.style.box_padding_x_em * self.css_px;
        let pad_y = self.style.box_padding_y_em * self.css_px;
        let pill_w = w + 2.0 * pad_x;
        let pill_h = self.style.line_height * self.css_px + 2.0 * pad_y;
        let radius = (self.style.box_radius_em * self.css_px)
            .min(pill_h / 2.0)
            .max(0.0);

        let ph = self.play_h.max(1) as f64;
        let v_top = self.video.y as f64 / ph;
        let v_bottom = (self.video.y + self.video.h) as f64 / ph;
        // The ACTUAL pill height is more precise than the max-lines estimate the auto-box path relies on.
        let cap = pill_h / ph;
        let pill_y = match caption_top_frac(
            &self.style.position,
            self.style.offset_pct,
            cap,
            v_top,
            v_bottom,
        ) {
            Some(tf) => tf * ph,
            None => (v_top + v_bottom) / 2.0 * ph - pill_h / 2.0,
        };
        let vx = self.video.x as f64;
        let vw = self.video.w as f64;
        let inset = vw * 0.04;
        let pill_x = match self.style.align.as_str() {
            "left" => vx + inset,
            "right" => vx + vw - inset - pill_w,
            _ => vx + (vw - pill_w) / 2.0,
        }
        .max(vx.max(0.0));
        Some(PillLayout {
            pill_x,
            pill_y,
            pill_w,
            pill_h,
            radius,
            text_x: pill_x + pad_x,
            text_y: pill_y + pad_y,
        })
    }
}

/// ASS `\p1` drawing commands for a rounded rectangle `w`×`h`, corner radius `r`,
/// origin at (0,0). Corners are cubic-bezier quarter-arcs (kappa ≈ 0.5523), so a
/// radius of h/2 yields a clean stadium.
fn pill_path(w: f64, h: f64, r: f64) -> String {
    let r = r.min(w / 2.0).min(h / 2.0).max(0.0);
    let k = 0.5523 * r;
    let n = |v: f64| format!("{v:.1}");
    format!(
        "m {} 0 l {} 0 b {} 0 {} {} {} {} l {} {} b {} {} {} {} {} {} l {} {} b {} {} 0 {} 0 {} l 0 {} b 0 {} {} 0 {} 0",
        n(r),           // m r 0
        n(w - r),       // l (w-r) 0
        n(w - r + k), n(w), n(r - k), n(w), n(r), // TR corner -> (w,r)
        n(w), n(h - r), // l w (h-r)
        n(w), n(h - r + k), n(w - r + k), n(h), n(w - r), n(h), // BR -> (w-r,h)
        n(r), n(h),     // l r h
        n(r - k), n(h), n(h - r + k), n(h - r), // BL -> (0,h-r)
        n(r),           // l 0 r
        n(r - k), n(r - k), n(r), // TL -> (r,0)
    )
}

/// Emit the pill Dialogue (layer 0) for a chunk window.
fn emit_pill(
    out: &mut String,
    style: &CaptionStyle,
    p: &PillLayout,
    ds: f64,
    de: f64,
    offset: f64,
    clip_len: f64,
) {
    let fill = ass_primary(&style.background_color);
    let alpha =
        ((100.0 - style.background_opacity).clamp(0.0, 100.0) / 100.0 * 255.0).round() as u8;
    let prefix = format!(
        "{{\\an7\\pos({:.0},{:.0})\\bord0\\shad0\\1c{fill}\\1a&H{alpha:02X}&\\p1}}",
        p.pill_x, p.pill_y
    );
    let body = format!("{}{{\\p0}}", pill_path(p.pill_w, p.pill_h, p.radius));
    push_dialogue_layer(out, 0, ds, de, offset, clip_len, &prefix, &body);
}

/// Emit the ASS events for one segment under an animation spec. Each display
/// chunk is held until the next chunk starts (so single-word styles never blink
/// to empty). When words need per-word colour (progressive highlight or active-
/// word emphasis) the chunk is split into one sub-event per word window, each
/// colouring every word by the same rule the preview uses; the first sub-event
/// carries the entrance. When the chunk fits ONE line and the style is a box with
/// a measurable font, an exact rounded pill (`\p1`, layer 0) is drawn behind the
/// `\pos`-anchored text and the square auto box is suppressed; otherwise the
/// Style's `BorderStyle:3` auto box handles the background.
fn emit_animated_segment(
    out: &mut String,
    ctx: &LayoutCtx,
    seg: &super::TranscriptSegment,
    anim: &CaptionAnimation,
    offset: f64,
    clip_len: f64,
) {
    let style = ctx.style;
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
    // Per-word events only when a word's colour depends on progress or the active word is emphasised.
    let per_word = anim.highlight() == "progressive" || anim.emphasis != "none";

    for (i, run) in runs.iter().enumerate() {
        let ds = run[0].start;
        // Hold the chunk until the next chunk starts (last chunk → segment end).
        let de = if i + 1 < runs.len() {
            runs[i + 1][0].start
        } else {
            seg.end
        };

        // Exact rounded pill only for a single line; multi-line keeps the square auto box libass sizes correctly.
        let single_line =
            break_into_lines(run, style.max_chars_per_line, style.max_lines).len() == 1;
        let line_text = run
            .iter()
            .map(|w| ass_text(&w.text, style.uppercase))
            .collect::<Vec<_>>()
            .join(" ");
        let pill = if single_line {
            ctx.pill_for_line(&line_text)
        } else {
            None
        };
        // In pill mode the text is `\pos`-anchored inside the pill and the auto box is made transparent.
        let pos_prefix = pill.as_ref().map_or(String::new(), |p| {
            format!(
                "{{\\an7\\pos({:.0},{:.0})\\3a&HFF&\\4a&HFF&}}",
                p.text_x, p.text_y
            )
        });
        if let Some(p) = &pill {
            emit_pill(out, style, p, ds, de, offset, clip_len);
        }
        let text_layer = if pill.is_some() { 1 } else { 0 };

        if !per_word {
            push_dialogue_layer(
                out,
                text_layer,
                ds,
                de,
                offset,
                clip_len,
                &format!("{pos_prefix}{}", entrance_tag(anim)),
                &run_text(run, None, run.len(), anim, style),
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
            let entrance = if j == 0 {
                entrance_tag(anim)
            } else {
                String::new()
            };
            // At word j's window, words 0..=j are spoken and j is active, matching the preview at that time.
            push_dialogue_layer(
                out,
                text_layer,
                ws,
                we,
                offset,
                clip_len,
                &format!("{pos_prefix}{entrance}"),
                &run_text(run, Some(j), j + 1, anim, style),
            );
        }
    }
}

/// The run's words joined with spaces. Each word is wrapped in a `\c` colour
/// override (from {@link word_color}) so progressive highlight and active-word
/// accent compose; the active word additionally scales for `scale` emphasis.
/// `active` is the currently-spoken word (None = none), `spoken` how many words
/// are spoken. Mirrors `wordColor`/`wordScaled` in @recast/captions.
fn run_text(
    run: &[TranscriptWord],
    active: Option<usize>,
    spoken: usize,
    anim: &CaptionAnimation,
    style: &CaptionStyle,
) -> String {
    run.iter()
        .enumerate()
        .map(|(j, w)| {
            let txt = ass_text(&w.text, style.uppercase);
            let col = word_color(j, active, spoken, anim, style);
            let colored = format!("{{\\c{col}}}{txt}");
            if word_scaled(j, active, run.len(), anim) {
                format!("{{\\fscx114\\fscy114}}{colored}{{\\fscx100\\fscy100}}")
            } else {
                colored
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Inline ASS colour literal for word `index`, wrapping the shared rule.
fn word_color(
    index: usize,
    active: Option<usize>,
    spoken: usize,
    anim: &CaptionAnimation,
    style: &CaptionStyle,
) -> String {
    ass_primary(recast_captions::word_color(
        index, active, spoken, anim, style,
    ))
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
pub(crate) fn ass_font_name(stack: &str) -> String {
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

    /// A RenderFont with no size correction and no measure face (tests assert
    /// markup, not glyph size; `measure: None` keeps the square auto box).
    fn rf(embedded: bool) -> RenderFont {
        RenderFont {
            ass_name: "Arial".into(),
            embedded,
            ass_scale: 1.0,
            measure: None,
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

    /// A cut landing inside a caption must split it, not stretch one cue across
    /// the seam carrying words the export removed. Mirrors the TS regression in
    /// `apps/desktop/src/lib/captions/output-time.test.ts`.
    #[test]
    fn split_transcript_by_spans_splits_a_straddling_segment() {
        let t = transcript(words(&[
            (10.0, 11.0, "before"),
            (11.0, 12.0, "seam"),
            (16.0, 17.0, "after"),
            (17.0, 18.0, "seam"),
        ]));
        // Cut removes [12, 16) out of a [0, 24] clip.
        let spans = kept_spans(0.0, 24.0, &[(12.0, 16.0)]);
        assert_eq!(spans, vec![(0.0, 12.0), (16.0, 24.0)]);

        let out = split_transcript_by_spans(&t, &spans);

        assert_eq!(out.segments.len(), 2);
        assert_eq!((out.segments[0].start, out.segments[0].end), (10.0, 12.0));
        assert_eq!((out.segments[1].start, out.segments[1].end), (16.0, 18.0));
        assert_eq!(out.segments[0].text, "before seam");
        assert_eq!(out.segments[1].text, "after seam");
        assert_ne!(out.segments[0].id, out.segments[1].id);
    }

    #[test]
    fn split_transcript_by_spans_drops_words_inside_the_cut() {
        let t = transcript(words(&[
            (10.0, 11.0, "kept"),
            (13.0, 15.0, "removed"),
            (16.0, 17.0, "kept2"),
        ]));
        let out = split_transcript_by_spans(&t, &kept_spans(0.0, 24.0, &[(12.0, 16.0)]));
        let texts: Vec<&str> = out
            .segments
            .iter()
            .flat_map(|s| s.words.iter().map(|w| w.text.as_str()))
            .collect();
        assert_eq!(texts, vec!["kept", "kept2"]);
    }

    #[test]
    fn split_transcript_by_spans_drops_a_segment_wholly_inside_a_cut() {
        let t = transcript(words(&[(13.0, 15.0, "gone")]));
        let out = split_transcript_by_spans(&t, &kept_spans(0.0, 24.0, &[(12.0, 16.0)]));
        assert!(out.segments.is_empty());
    }

    #[test]
    fn split_transcript_by_spans_leaves_an_uncut_segment_untouched() {
        let t = transcript(words(&[(2.0, 3.0, "hello"), (3.0, 5.0, "there")]));
        let out = split_transcript_by_spans(&t, &kept_spans(0.0, 24.0, &[(12.0, 16.0)]));
        assert_eq!(out.segments.len(), 1);
        assert_eq!(out.segments[0].id, "seg-0");
        assert_eq!(out.segments[0].text, "hello there");
    }

    #[test]
    fn kept_spans_honours_trim_and_merges_edge_cuts() {
        // A cut overlapping the trim head just moves the first span's start.
        assert_eq!(kept_spans(5.0, 20.0, &[(3.0, 8.0)]), vec![(8.0, 20.0)]);
        // A cut running past the trim tail truncates the last span.
        assert_eq!(kept_spans(0.0, 20.0, &[(18.0, 25.0)]), vec![(0.0, 18.0)]);
        // Cuts entirely outside the clip are ignored.
        assert_eq!(kept_spans(5.0, 10.0, &[(0.0, 2.0)]), vec![(5.0, 10.0)]);
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
        let ass = to_ass(&t, &style, 1920, 1080, full_vr(), 0.0, 10.0, &rf(false));
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
        let ass = to_ass(&t, &style, 1920, 1080, full_vr(), 0.0, 10.0, &rf(false));
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
        let ass = to_ass(&t, &style, 1920, 1080, full_vr(), 0.0, 10.0, &rf(false));
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
        let ass = to_ass(&t, &style, 1920, 1080, full_vr(), 0.0, 10.0, &rf(false));
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
        let ass = to_ass(&t, &style, 1920, 1080, full_vr(), 1.0, 1.4, &rf(false));
        assert_eq!(dialogues(&ass).len(), 1);
    }

    fn style_bold(ass: &str) -> i32 {
        // Bold is the field after the 7th comma of the `Style: Default,...` line.
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
            style_bold(&to_ass(
                &t,
                &heavy,
                1920,
                1080,
                full_vr(),
                0.0,
                10.0,
                &rf(true)
            )),
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
                &rf(false)
            )),
            0
        );
        let bold = CaptionStyle {
            font_weight: 700,
            animation: None,
            ..Default::default()
        };
        assert_eq!(
            style_bold(&to_ass(
                &t,
                &bold,
                1920,
                1080,
                full_vr(),
                0.0,
                10.0,
                &rf(false)
            )),
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
    fn caption_top_frac_places_bottom_and_offset_moves_inward() {
        let cap = 0.12;
        // 15% padding top/bottom → at Offset 0 the block sits at the video's edge.
        let top = caption_top_frac("bottom", 0.0, cap, 0.15, 0.85).unwrap();
        assert!(top >= 0.85 - 1e-9);
        assert!(top + cap <= 1.0 + 1e-9);
        // Full-bleed video: baseline at the frame edge, and a positive Offset still lifts the caption inward.
        let base = caption_top_frac("bottom", 0.0, cap, 0.0, 1.0).unwrap();
        let lifted = caption_top_frac("bottom", 8.0, cap, 0.0, 1.0).unwrap();
        assert!((base - (1.0 - cap)).abs() < 1e-9);
        assert!((lifted - (1.0 - cap - 0.08)).abs() < 1e-9);
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
            &t,
            &bottom,
            1920,
            1080,
            video,
            0.0,
            10.0,
            &rf(false)
        ))));
        let center = CaptionStyle {
            position: "center".into(),
            animation: None,
            ..Default::default()
        };
        assert!((4..=6).contains(&style_alignment(&to_ass(
            &t,
            &center,
            1920,
            1080,
            video,
            0.0,
            10.0,
            &rf(false)
        ))));
    }

    // Manual render check (ignored in CI): writes an ASS a human renders via ffmpeg to confirm the pill hugs the text.
    #[test]
    #[ignore]
    fn render_pill_ass_for_manual_inspection() {
        let m =
            super::super::text_measure::resolve_font("Arial", 600, None).expect("resolve Arial");
        let font = RenderFont {
            ass_name: m.ass_name,
            embedded: false,
            ass_scale: m.ass_scale,
            measure: Some(m.measure),
        };
        let style = CaptionStyle {
            background: "box".into(),
            background_color: "#0b0b12".into(),
            background_opacity: 78.0,
            box_radius_em: 0.6,
            animation: Some(CaptionAnimation {
                highlight: Some("progressive".into()),
                ..anim("phrase", "none", "slide")
            }),
            ..Default::default()
        };
        let t = transcript(words(&[
            (0.2, 0.5, "but"),
            (0.5, 0.7, "it's"),
            (0.7, 0.9, "a"),
            (0.9, 1.3, "text"),
            (1.3, 1.8, "editor"),
        ]));
        let ass = to_ass(&t, &style, 1920, 1080, full_vr(), 0.0, 10.0, &font);
        let path = std::env::temp_dir().join("recast-pill-check.ass");
        std::fs::write(&path, &ass).unwrap();
        eprintln!("wrote {}", path.display());
        assert!(ass.contains("\\p1"), "pill drawn");
    }

    #[test]
    fn srt_to_vtt_adds_header_and_dots_timecodes() {
        let srt =
            "1\n00:00:01,000 --> 00:00:02,500\nHello\n\n2\n00:00:02,500 --> 00:00:03,000\nworld\n";
        let vtt = srt_to_vtt(srt);
        assert!(vtt.starts_with("WEBVTT\n\n"));
        assert!(vtt.contains("00:00:01.000 --> 00:00:02.500"));
        assert!(vtt.contains("Hello"));
        // Only timecode commas flip to dots; text is untouched.
        assert!(!vtt.contains(",500"));
    }

    #[test]
    fn pill_path_is_a_closed_rounded_rect() {
        let p = pill_path(200.0, 60.0, 20.0);
        assert!(p.starts_with("m "), "starts with a move: {p}");
        // One bezier per rounded corner.
        assert_eq!(p.matches("b ").count(), 4, "four corner beziers: {p}");
        // Radius clamps to half the shorter side (stadium) without overflowing.
        assert!(
            pill_path(200.0, 60.0, 999.0).contains("30.0"),
            "radius clamps to h/2"
        );
    }

    #[test]
    fn no_measure_face_keeps_the_square_auto_box_no_pill() {
        // Tests build RenderFont with measure: None, so even a box style must keep the square auto box, not a pill.
        let style = CaptionStyle {
            background: "box".into(),
            animation: Some(CaptionAnimation {
                highlight: Some("progressive".into()),
                ..anim("line", "none", "none")
            }),
            ..Default::default()
        };
        let t = transcript(words(&[(0.0, 0.5, "one"), (0.5, 1.0, "two")]));
        let ass = to_ass(&t, &style, 1920, 1080, full_vr(), 0.0, 10.0, &rf(false));
        assert!(
            !ass.contains("\\p1"),
            "no vector pill without a measure face"
        );
        // Style still declares the opaque box (BorderStyle 3 = field index 15).
        let style_line = ass
            .lines()
            .find(|l| l.starts_with("Style: Default,"))
            .unwrap();
        assert_eq!(style_line.split(',').nth(15).unwrap().trim(), "3");
    }

    #[test]
    fn vtt_emits_word_timestamps_and_stays_compatible() {
        let t = transcript(words(&[(4.12, 4.38, "but"), (4.38, 4.6, "it's")]));
        let vtt = to_vtt(&t);
        assert!(vtt.starts_with("WEBVTT"));
        // Each word carries a leading inline timestamp; a tag-blind player still sees the plain words.
        assert!(vtt.contains("<00:00:04.120>but <00:00:04.380>it's"));

        // A segment without word timing falls back to plain cue text.
        let mut plain = transcript(words(&[(0.0, 1.0, "x")]));
        plain.segments[0].words.clear();
        plain.segments[0].text = "hello world".into();
        assert!(to_vtt(&plain).contains("hello world"));
    }

    #[test]
    fn progressive_highlight_mutes_unspoken_words() {
        let style = CaptionStyle {
            color: "#ffffff".into(),
            muted_color: "#a1a1aa".into(),
            animation: Some(CaptionAnimation {
                highlight: Some("progressive".into()),
                ..anim("line", "none", "none")
            }),
            ..Default::default()
        };
        let t = transcript(words(&[
            (0.0, 0.5, "one"),
            (0.5, 1.0, "two"),
            (1.0, 1.5, "three"),
        ]));
        let ass = to_ass(&t, &style, 1920, 1080, full_vr(), 0.0, 10.0, &rf(false));
        // The first of 3 word-window events must paint the later words with the muted colour.
        assert_eq!(dialogues(&ass).len(), 3);
        let first = dialogues(&ass)[0].to_lowercase();
        assert!(first.contains("&haaa1a1&"), "unspoken words muted: {first}");
        assert!(first.contains("&hffffff&"), "spoken word base: {first}");
    }

    #[test]
    fn word_color_mirrors_package_rule() {
        let style = CaptionStyle {
            color: "#ffffff".into(),
            muted_color: "#a1a1aa".into(),
            ..Default::default()
        };
        let prog = CaptionAnimation {
            highlight: Some("progressive".into()),
            emphasis: "none".into(),
            ..Default::default()
        };
        // spoken (index < spoken) -> base; unspoken -> muted.
        assert_eq!(
            word_color(0, None, 2, &prog, &style),
            ass_primary("#ffffff")
        );
        assert_eq!(
            word_color(2, None, 2, &prog, &style),
            ass_primary("#a1a1aa")
        );
        // colour emphasis on the active word wins over progressive muting.
        let prog_color = CaptionAnimation {
            emphasis: "color".into(),
            emphasis_color: "#4ade80".into(),
            ..prog
        };
        assert_eq!(
            word_color(2, Some(2), 2, &prog_color, &style),
            ass_primary("#4ade80")
        );
    }
}
