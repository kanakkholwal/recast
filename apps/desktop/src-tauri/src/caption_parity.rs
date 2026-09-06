//! Diffing the ASS burn-in against the compositor's caption pass. Reads a
//! generated script back so both sides can be sampled on one clock.

/// One `Dialogue` line recovered from a generated script.
#[derive(Debug, Clone, PartialEq)]
pub struct AssEvent {
    pub layer: u32,
    pub start: f64,
    pub end: f64,
    pub body: String,
}

impl AssEvent {
    /// Whether this event draws the pill rather than the text.
    #[must_use]
    pub fn is_drawing(&self) -> bool {
        self.body.contains("\\p1")
    }

    #[must_use]
    pub fn covers(&self, t: f64) -> bool {
        t >= self.start && t < self.end
    }
}

/// Every `Dialogue` line in `ass`, in file order.
#[must_use]
pub fn parse_events(ass: &str) -> Vec<AssEvent> {
    ass.lines()
        .filter_map(|line| line.strip_prefix("Dialogue: "))
        .filter_map(|rest| {
            // Text is the tenth field and carries commas of its own, so it must not be split.
            let mut fields = rest.splitn(10, ',');
            let layer = fields.next()?.trim().parse().ok()?;
            let start = parse_ass_time(fields.next()?)?;
            let end = parse_ass_time(fields.next()?)?;
            let body = fields.nth(6)?.to_string();
            Some(AssEvent {
                layer,
                start,
                end,
                body,
            })
        })
        .collect()
}

fn parse_ass_time(text: &str) -> Option<f64> {
    let mut parts = text.trim().split(':');
    let hours: f64 = parts.next()?.parse().ok()?;
    let minutes: f64 = parts.next()?.parse().ok()?;
    let seconds: f64 = parts.next()?.parse().ok()?;
    Some(hours * 3600.0 + minutes * 60.0 + seconds)
}

/// The text event on screen at `t`, or `None` when the script shows nothing.
#[must_use]
pub fn text_at(events: &[AssEvent], t: f64) -> Option<&AssEvent> {
    events.iter().find(|e| !e.is_drawing() && e.covers(t))
}

/// The pill event on screen at `t`.
#[must_use]
pub fn pill_at(events: &[AssEvent], t: f64) -> Option<&AssEvent> {
    events.iter().find(|e| e.is_drawing() && e.covers(t))
}

/// `&HBBGGRR&` back to `[r, g, b]`.
#[must_use]
pub fn ass_rgb(literal: &str) -> Option<[u8; 3]> {
    let hex = literal.trim_start_matches("&H").trim_end_matches('&');
    if hex.len() < 6 {
        return None;
    }
    let b = u8::from_str_radix(hex.get(0..2)?, 16).ok()?;
    let g = u8::from_str_radix(hex.get(2..4)?, 16).ok()?;
    let r = u8::from_str_radix(hex.get(4..6)?, 16).ok()?;
    Some([r, g, b])
}

/// The words of a text event with the colour each was written in, in order.
#[must_use]
pub fn coloured_words(body: &str) -> Vec<(String, [u8; 3])> {
    let mut out: Vec<(String, [u8; 3])> = Vec::new();
    let mut colour = [255u8, 255, 255];
    let mut rest = body;
    while let Some(open) = rest.find('{') {
        push_words(&mut out, &rest[..open], colour);
        let Some(close) = rest[open..].find('}') else {
            break;
        };
        let block = &rest[open + 1..open + close];
        if let Some(found) = colour_override(block) {
            colour = found;
        }
        rest = &rest[open + close + 1..];
    }
    push_words(&mut out, rest, colour);
    out
}

fn push_words(out: &mut Vec<(String, [u8; 3])>, text: &str, colour: [u8; 3]) {
    for word in text.split_whitespace() {
        out.push((word.to_string(), colour));
    }
}

/// The `\c` override in one block, ignoring the `\1a`/`\3a` alpha tags.
fn colour_override(block: &str) -> Option<[u8; 3]> {
    let at = block.rfind("\\c&H")?;
    let tail = &block[at + 2..];
    let end = tail.find('&').and_then(|first| {
        tail.get(first + 1..)
            .and_then(|r| r.find('&'))
            .map(|second| first + second + 2)
    })?;
    ass_rgb(&tail[..end])
}

/// One colour per glyph the burn-in will draw, so the sequence can be lined up
/// against the compositor's quads. Spaces shape to no glyph in either path.
#[must_use]
pub fn per_glyph_colours(body: &str) -> Vec<[u8; 3]> {
    coloured_words(body)
        .into_iter()
        .flat_map(|(word, colour)| std::iter::repeat_n(colour, word.chars().count()))
        .collect()
}

/// `\pos(x,y)` from an override block.
#[must_use]
pub fn pos(body: &str) -> Option<(f64, f64)> {
    let at = body.find("\\pos(")? + 5;
    let end = at + body[at..].find(')')?;
    let mut parts = body[at..end].split(',');
    let x = parts.next()?.trim().parse().ok()?;
    let y = parts.next()?.trim().parse().ok()?;
    Some((x, y))
}

/// Width and height of a `\p1` drawing, read as the extent of its coordinates.
#[must_use]
pub fn drawing_extent(body: &str) -> Option<(f64, f64)> {
    let numbers: Vec<f64> = body
        .split(|c: char| c.is_whitespace() || c == '{' || c == '}')
        .filter_map(|token| token.parse::<f64>().ok())
        .collect();
    if numbers.len() < 4 {
        return None;
    }
    // Every path command takes coordinate pairs, so from the leading `m` the numbers alternate x then y.
    let extent = |skip: usize| {
        numbers
            .iter()
            .skip(skip)
            .step_by(2)
            .copied()
            .fold(0.0, f64::max)
    };
    Some((extent(0), extent(1)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use recast_captions::{
        CaptionAnimation, CaptionCue, CaptionStyle, CaptionTrack, TranscriptWord,
    };
    use recast_compositor::caption::{layout_caption, CaptionClock, CaptionFrame, VideoRect};
    use recast_text::{FontFace, GlyphAtlas};

    use crate::transcription::subtitles::{to_ass, RenderFont, VideoRectPx};
    use crate::transcription::{Transcript, TranscriptSegment};

    const CANVAS: (u32, u32) = (1920, 1080);

    /// One face for both paths, so a shaping difference cannot be mistaken for a
    /// layout one. `None` when the machine has no such font, which skips.
    fn shared_face() -> Option<(FontFace, RenderFont)> {
        let resolved = recast_text::resolve_face("Arial", 400, None)?;
        // Both resolvers query one fontdb for the same family and weight, so this is the same file twice.
        let matched = crate::transcription::text_measure::resolve_font("Arial", 400, None)?;
        let font = RenderFont {
            ass_name: matched.ass_name,
            embedded: false,
            // Only the Style header uses it, and the compositor has no libass size correction to match.
            ass_scale: 1.0,
            measure: Some(matched.measure),
        };
        Some((resolved.face, font))
    }

    fn word(text: &str, start: f64, end: f64) -> TranscriptWord {
        TranscriptWord {
            start,
            end,
            text: text.into(),
        }
    }

    /// Six words on a half-second grid, all plain ASCII so one glyph is one char.
    fn words() -> Vec<TranscriptWord> {
        vec![
            word("the", 1.0, 1.4),
            word("quick", 1.5, 1.9),
            word("brown", 2.0, 2.4),
            word("dog", 2.5, 2.9),
            word("runs", 3.0, 3.4),
            word("home", 3.5, 3.9),
        ]
    }

    fn transcript(words: Vec<TranscriptWord>) -> Transcript {
        let start = words.first().map_or(0.0, |w| w.start);
        let end = words.last().map_or(0.0, |w| w.end);
        let text = words
            .iter()
            .map(|w| w.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        Transcript {
            engine: "test".into(),
            model_id: "test".into(),
            language: None,
            segments: vec![TranscriptSegment {
                id: "s0".into(),
                start,
                end,
                text,
                words,
            }],
        }
    }

    /// No entrance, so a sample is never caught mid-fade with the two paths
    /// transforming the block differently.
    fn styled(emphasis: &str, highlight: &str) -> CaptionStyle {
        CaptionStyle {
            animation: Some(CaptionAnimation {
                chunk: "phrase".into(),
                chunk_size: 3,
                emphasis: emphasis.into(),
                emphasis_color: "#facc15".into(),
                highlight: Some(highlight.into()),
                entrance: "none".into(),
                entrance_ms: 0.0,
                hold_gaps: true,
            }),
            ..CaptionStyle::default()
        }
    }

    fn style() -> CaptionStyle {
        styled("none", "progressive")
    }

    /// Every way a word can be coloured: by how far the line has been spoken,
    /// by being the one word now spoken, and by both at once.
    fn colour_styles() -> Vec<(&'static str, CaptionStyle)> {
        vec![
            ("progressive", styled("none", "progressive")),
            ("active word accent", styled("color", "none")),
            ("both", styled("color", "progressive")),
        ]
    }

    fn video_rect() -> (VideoRect, VideoRectPx) {
        let px = VideoRectPx {
            x: 96,
            y: 54,
            w: 1728,
            h: 972,
        };
        let rect = VideoRect {
            x: f64::from(px.x),
            y: f64::from(px.y),
            w: f64::from(px.w),
            h: f64::from(px.h),
        };
        (rect, px)
    }

    fn burn_in(t: &Transcript, style: &CaptionStyle, font: &RenderFont) -> Vec<AssEvent> {
        let (_, px) = video_rect();
        parse_events(&to_ass(t, style, CANVAS.0, CANVAS.1, px, 0.0, 10.0, font))
    }

    /// The track as the host builds it from a transcript.
    fn track(t: &Transcript) -> CaptionTrack {
        CaptionTrack {
            segments: t
                .segments
                .iter()
                .map(|s| CaptionCue {
                    start: s.start,
                    end: s.end,
                    words: s.words.clone(),
                })
                .collect(),
        }
    }

    fn engine_frame(
        style: &CaptionStyle,
        track: &CaptionTrack,
        face: &FontFace,
        t: f64,
    ) -> CaptionFrame {
        let (rect, _) = video_rect();
        let mut atlas = GlyphAtlas::new(1024, 4096);
        layout_caption(
            style,
            track,
            CaptionClock {
                source: t,
                output: t,
                time_map: None,
            },
            rect,
            CANVAS,
            face,
            0,
            &mut atlas,
        )
    }

    fn glyph_colours(frame: &CaptionFrame) -> Vec<[u8; 3]> {
        frame
            .glyphs
            .iter()
            .map(|q| {
                [
                    (q.colour[0] * 255.0).round() as u8,
                    (q.colour[1] * 255.0).round() as u8,
                    (q.colour[2] * 255.0).round() as u8,
                ]
            })
            .collect()
    }

    /// Away from every word boundary, where a centisecond of ASS rounding decides
    /// which side of a highlight flip a sample lands on.
    fn samples() -> Vec<f64> {
        let mut out = Vec::new();
        let mut t: f64 = 1.05;
        while t < 3.9 {
            out.push((t * 1000.0).round() / 1000.0);
            t += 0.1;
        }
        out
    }

    #[test]
    fn both_paths_light_the_same_words_at_every_sample() {
        let Some((face, font)) = shared_face() else {
            return;
        };
        let source = transcript(words());
        let track = track(&source);
        for (name, style) in colour_styles() {
            let events = burn_in(&source, &style, &font);
            for t in samples() {
                let burned = text_at(&events, t).map(|e| per_glyph_colours(&e.body));
                let rendered = glyph_colours(&engine_frame(&style, &track, &face, t));
                assert_eq!(
                    burned,
                    Some(rendered),
                    "the two caption paths disagree on {name} at t={t}"
                );
            }
        }
    }

    /// The samples must include a gap between two words, or `hold_gaps` and the
    /// active-word rule never differ between the paths.
    #[test]
    fn the_samples_land_between_words_as_well_as_inside_them() {
        let words = words();
        let in_gap = samples().into_iter().filter(|&t| {
            words
                .iter()
                .any(|w| t >= w.end && words.iter().any(|n| n.start > t))
        });
        assert!(in_gap.count() >= 3);
    }

    /// A word glyph must be a word char, or the colour sequences above are lined
    /// up wrongly and would agree by accident.
    #[test]
    fn the_fixture_shapes_one_glyph_per_character() {
        let Some((face, _)) = shared_face() else {
            return;
        };
        let style = style();
        let frame = engine_frame(&style, &track(&transcript(words())), &face, 2.05);
        assert_eq!(frame.glyphs.len(), "thequickbrown".len());
    }

    /// Two segments of four words under a six-word chunk. A track with no
    /// segment structure chunks straight across the boundary and shows two
    /// words of the second segment while the first is still on screen.
    fn two_segments() -> Transcript {
        let mut t = transcript(vec![
            word("one", 1.0, 1.4),
            word("two", 1.5, 1.9),
            word("three", 2.0, 2.4),
            word("four", 2.5, 2.9),
        ]);
        let second = transcript(vec![
            word("five", 4.0, 4.4),
            word("six", 4.5, 4.9),
            word("seven", 5.0, 5.4),
            word("eight", 5.5, 5.9),
        ]);
        t.segments.extend(second.segments);
        t
    }

    #[test]
    fn both_paths_break_a_chunk_at_the_segment_boundary() {
        let Some((face, font)) = shared_face() else {
            return;
        };
        let transcript = two_segments();
        let style = styled("none", "progressive");
        let events = burn_in(&transcript, &style, &font);
        let track = track(&transcript);

        // Across both segments and the silence between, where an unsegmented track keeps drawing.
        for step in 0..70 {
            let t = 0.95 + f64::from(step) * 0.1;
            let burned = text_at(&events, t)
                .map(|e| per_glyph_colours(&e.body))
                .unwrap_or_default();
            let rendered = glyph_colours(&engine_frame(&style, &track, &face, t));
            assert_eq!(burned, rendered, "the two paths disagree at t={t}");
        }
    }

    #[test]
    fn neither_path_shows_a_caption_after_the_last_word() {
        let Some((face, font)) = shared_face() else {
            return;
        };
        let transcript = two_segments();
        let style = styled("none", "progressive");
        let events = burn_in(&transcript, &style, &font);
        let track = track(&transcript);

        assert!(
            text_at(&events, 7.0).is_none(),
            "the burn-in held the caption"
        );
        assert!(
            engine_frame(&style, &track, &face, 7.0).is_empty(),
            "the compositor held the caption past the transcript"
        );
    }

    #[test]
    fn a_single_line_pill_lands_in_the_same_place_in_both_paths() {
        let Some((face, font)) = shared_face() else {
            return;
        };
        let source = transcript(words());
        let style = style();
        let events = burn_in(&source, &style, &font);

        let t = 2.05;
        let event = pill_at(&events, t).expect("the burn-in drew no pill");
        let (bx, by) = pos(&event.body).expect("the pill event has no position");
        let (bw, bh) = drawing_extent(&event.body).expect("the pill event has no path");
        let pill = engine_frame(&style, &track(&source), &face, t)
            .pill
            .expect("the compositor drew no pill");

        for (burned, rendered, axis) in [
            (bx, f64::from(pill.x), "x"),
            (by, f64::from(pill.y), "y"),
            (bw, f64::from(pill.w), "width"),
            (bh, f64::from(pill.h), "height"),
        ] {
            assert!(
                (burned - rendered).abs() <= 1.0,
                "pill {axis}: burn-in {burned}, compositor {rendered}"
            );
        }
    }
}
