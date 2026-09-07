//! Sampled frames become OCR'd elements, then temporally adjacent near-identical frames collapse into spans.
//! Boxes are normalized 0..1 (the OmniParser/UI-TARS convention) so a text-only model reasons independently of resolution.

use serde::Serialize;

use super::engine::OcrEngine;
use super::frames::SampledFrame;

/// One recognized element (a text line), OmniParser-shaped.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenElement {
    /// Stable within a span, for Set-of-Mark style reference ("element 7").
    pub id: u32,
    /// `"text"` today; `"icon"` once a permissive detector is added.
    pub kind: String,
    /// Normalized `[x0, y0, x1, y1]` in 0..1 of the frame.
    pub bbox: [f32; 4],
    pub content: String,
    /// Which engine produced this, e.g. `"ocrs"`.
    pub source: String,
}

/// A stretch of time over which the screen text stayed effectively the same.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenStateSpan {
    /// Seconds on the video clock.
    pub start: f64,
    pub end: f64,
    pub elements: Vec<ScreenElement>,
    /// Optional inline preview of the span's representative frame, as a
    /// `data:image/jpeg;base64,...` URI. Off unless `TimelineOpts::previews` is
    /// set, because it is only for humans reviewing the output; the structured
    /// `elements` are what a model consumes.
    pub preview: Option<String>,
}

/// The full structured read of a video.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoTextTimeline {
    /// Engine that read the frames, e.g. `"ocrs"`.
    pub engine: String,
    pub spans: Vec<ScreenStateSpan>,
    /// What the run actually did. Not part of the model-facing payload; it exists
    /// so a human reviewing a read can see the work behind it (how many frames the
    /// sampler walked, how many survived the gate, where the time went) instead of
    /// being handed spans with no provenance.
    pub stats: OcrStats,
}

/// Counters and per-stage timings for one read.
/// `build_timeline` fills in what it knows (the frames it read, the elements it found); the caller that owns the whole pass fills in the rest.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OcrStats {
    /// Video length in seconds, per ffprobe.
    pub duration_secs: f64,
    /// Coarse frames the decode pass walked.
    pub frames_scanned: u32,
    /// Frames that survived the change gate and were actually OCR'd.
    pub frames_read: u32,
    /// Total recognized elements across every span.
    pub elements: u32,
    /// Decode + change-gate pass.
    pub sample_ms: u64,
    /// One-time model load.
    pub model_load_ms: u64,
    /// The OCR pass itself, which dominates the rest by a wide margin.
    pub ocr_ms: u64,
}

/// Progress of the OCR pass, reported once per frame read.
#[derive(Debug, Clone, Copy)]
pub struct ReadTick {
    /// Frames OCR'd so far.
    pub done: u64,
    /// Frames to OCR in total. Exact: the sampler has already finished.
    pub total: u64,
    /// Distinct screen states found so far.
    pub spans: u64,
}

/// How similar two spans' text must be to be treated as the same screen state.
const MERGE_JACCARD: f32 = 0.9;

/// Long edge of the inline preview image. Small on purpose: it exists so a human
/// can scan the spans in a list, not as a reference image for a vision model.
const PREVIEW_MAX_DIM: u32 = 480;

/// Options for building the timeline.
#[derive(Debug, Clone, Default)]
pub struct TimelineOpts {
    /// Attach a small JPEG preview of each span's frame (for review UIs).
    pub previews: bool,
}

/// OCRs every sampled frame and collapses near-identical neighbours into spans; `total_secs` closes the final span.
/// `on_tick` fires after each recognition, the phase worth reporting: OCR runs at about a third of a second a frame, so it is nearly all the wall clock.
pub fn build_timeline(
    frames: &[SampledFrame],
    total_secs: f64,
    engine: &dyn OcrEngine,
    opts: &TimelineOpts,
    on_tick: &mut dyn FnMut(ReadTick),
) -> Result<VideoTextTimeline, String> {
    let source = engine.source().to_string();

    struct Build {
        start: f64,
        elements: Vec<ScreenElement>,
        texts: Vec<String>,
        preview: Option<String>,
    }
    let mut builds: Vec<Build> = Vec::new();
    let mut element_count: u64 = 0;

    for (i, frame) in frames.iter().enumerate() {
        let lines = engine.recognize(&frame.rgba, frame.width, frame.height)?;
        let texts: Vec<String> = lines
            .iter()
            .map(|l| normalize_text(&l.text))
            .filter(|t| !t.is_empty())
            .collect();

        // Tick even for a frame that merges away: it cost a full OCR pass, and only its output is discarded.
        let tick = |builds: &[Build], on_tick: &mut dyn FnMut(ReadTick)| {
            on_tick(ReadTick {
                done: i as u64 + 1,
                total: frames.len() as u64,
                spans: builds.len() as u64,
            });
        };

        // Same screen state as the previous span? Extend it instead of adding one.
        if let Some(last) = builds.last() {
            if jaccard(&last.texts, &texts) >= MERGE_JACCARD {
                tick(&builds, on_tick);
                continue;
            }
        }

        element_count += lines.len() as u64;

        let elements = lines
            .iter()
            .enumerate()
            .map(|(i, l)| ScreenElement {
                id: i as u32,
                kind: "text".into(),
                bbox: normalize_bbox(l.x, l.y, l.width, l.height, frame.width, frame.height),
                content: l.text.clone(),
                source: source.clone(),
            })
            .collect();

        let preview = if opts.previews {
            encode_preview(&frame.rgba, frame.width, frame.height)
        } else {
            None
        };

        builds.push(Build {
            start: frame.t_secs,
            elements,
            texts,
            preview,
        });

        tick(&builds, on_tick);
    }

    // Close each span at the next one's start; the last runs to the video end.
    let mut spans = Vec::with_capacity(builds.len());
    for i in 0..builds.len() {
        let start = builds[i].start;
        let end = builds
            .get(i + 1)
            .map(|b| b.start)
            .unwrap_or(total_secs.max(start));
        spans.push(ScreenStateSpan {
            start,
            end,
            elements: builds[i].elements.clone(),
            preview: builds[i].preview.clone(),
        });
    }

    Ok(VideoTextTimeline {
        engine: source,
        stats: OcrStats {
            duration_secs: total_secs,
            frames_read: frames.len() as u32,
            elements: element_count as u32,
            ..Default::default()
        },
        spans,
    })
}

/// Encode a frame as a small JPEG `data:` URI for the review UI. Returns `None`
/// rather than failing the whole read: a missing preview is cosmetic, and the
/// structured elements are the real payload.
fn encode_preview(rgba: &[u8], width: u32, height: u32) -> Option<String> {
    use base64::Engine as _;
    use image::codecs::jpeg::JpegEncoder;

    let img = image::RgbaImage::from_raw(width, height, rgba.to_vec())?;
    let scaled = image::DynamicImage::ImageRgba8(img)
        .resize(
            PREVIEW_MAX_DIM,
            PREVIEW_MAX_DIM,
            image::imageops::FilterType::Triangle,
        )
        .to_rgb8();

    let mut buf = Vec::new();
    JpegEncoder::new_with_quality(&mut buf, 75)
        .encode_image(&scaled)
        .ok()?;
    let b64 = base64::engine::general_purpose::STANDARD.encode(&buf);
    Some(format!("data:image/jpeg;base64,{b64}"))
}

/// Normalize a pixel box into 0..1 fractions of the frame, clamped.
fn normalize_bbox(x: i32, y: i32, w: i32, h: i32, fw: u32, fh: u32) -> [f32; 4] {
    let fw = fw.max(1) as f32;
    let fh = fh.max(1) as f32;
    let x0 = (x as f32 / fw).clamp(0.0, 1.0);
    let y0 = (y as f32 / fh).clamp(0.0, 1.0);
    let x1 = ((x + w) as f32 / fw).clamp(0.0, 1.0);
    let y1 = ((y + h) as f32 / fh).clamp(0.0, 1.0);
    [x0, y0, x1, y1]
}

/// Lowercased, whitespace-collapsed text for comparing two frames' content.
fn normalize_text(s: &str) -> String {
    s.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// Jaccard similarity of two bags of strings, treated as sets. Two empty sets
/// count as identical (a blank screen staying blank is the same state).
fn jaccard(a: &[String], b: &[String]) -> f32 {
    if a.is_empty() && b.is_empty() {
        return 1.0;
    }
    use std::collections::HashSet;
    let sa: HashSet<&String> = a.iter().collect();
    let sb: HashSet<&String> = b.iter().collect();
    let inter = sa.intersection(&sb).count() as f32;
    let union = sa.union(&sb).count() as f32;
    if union == 0.0 {
        0.0
    } else {
        inter / union
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ocr::engine::OcrLine;
    use std::cell::RefCell;

    /// Replays a scripted line-set per frame, so span collapsing can be tested
    /// without models or real pixels.
    struct StubEngine {
        frames: RefCell<Vec<Vec<&'static str>>>,
    }

    impl StubEngine {
        fn new(frames: Vec<Vec<&'static str>>) -> Self {
            let mut f = frames;
            f.reverse(); // pop() from the back yields them in order
            Self {
                frames: RefCell::new(f),
            }
        }
    }

    impl OcrEngine for StubEngine {
        fn recognize(&self, _rgba: &[u8], _w: u32, _h: u32) -> Result<Vec<OcrLine>, String> {
            let texts = self.frames.borrow_mut().pop().unwrap_or_default();
            Ok(texts
                .into_iter()
                .enumerate()
                .map(|(i, t)| OcrLine {
                    text: t.to_string(),
                    x: 0,
                    y: (i as i32) * 10,
                    width: 100,
                    height: 10,
                })
                .collect())
        }
        fn source(&self) -> &'static str {
            "stub"
        }
    }

    fn frame(t: f64) -> SampledFrame {
        SampledFrame {
            t_secs: t,
            rgba: vec![0; 4], // the stub ignores pixels
            width: 100,
            height: 100,
        }
    }

    #[test]
    fn identical_frames_collapse_into_one_span() {
        let frames = vec![frame(0.0), frame(1.0), frame(2.0)];
        let engine = StubEngine::new(vec![
            vec!["File", "Edit"],
            vec!["File", "Edit"],
            vec!["File", "Edit"],
        ]);
        let tl =
            build_timeline(&frames, 3.0, &engine, &TimelineOpts::default(), &mut |_| {}).unwrap();
        assert_eq!(tl.spans.len(), 1);
        assert_eq!(tl.spans[0].start, 0.0);
        // The last span runs to the end of the video.
        assert_eq!(tl.spans[0].end, 3.0);
        assert_eq!(tl.engine, "stub");
    }

    #[test]
    fn a_screen_change_starts_a_new_span_closed_at_the_next_start() {
        let frames = vec![frame(0.0), frame(1.0), frame(2.0)];
        let engine = StubEngine::new(vec![
            vec!["File", "Edit"],
            vec!["File", "Edit"],
            vec!["Settings", "Privacy"], // a real change
        ]);
        let tl =
            build_timeline(&frames, 4.0, &engine, &TimelineOpts::default(), &mut |_| {}).unwrap();
        assert_eq!(tl.spans.len(), 2);
        // First span is closed at the second span's start, not at its own frame.
        assert_eq!(tl.spans[0].start, 0.0);
        assert_eq!(tl.spans[0].end, 2.0);
        assert_eq!(tl.spans[1].start, 2.0);
        assert_eq!(tl.spans[1].end, 4.0);
    }

    #[test]
    fn progress_ticks_once_per_frame_even_when_a_frame_merges_away() {
        // Three frames, two the same screen: every frame costs an OCR pass, so a merged one skipping its tick stalls the bar.
        let frames = vec![frame(0.0), frame(1.0), frame(2.0)];
        let engine = StubEngine::new(vec![
            vec!["File"],
            vec!["File"], // merges into the first span
            vec!["Settings"],
        ]);
        let mut ticks: Vec<ReadTick> = Vec::new();
        let tl = build_timeline(&frames, 3.0, &engine, &TimelineOpts::default(), &mut |t| {
            ticks.push(t)
        })
        .unwrap();

        assert_eq!(ticks.len(), 3, "one tick per frame read, merged or not");
        assert_eq!(ticks.iter().map(|t| t.done).collect::<Vec<_>>(), [1, 2, 3]);
        assert!(ticks.iter().all(|t| t.total == 3));
        // Spans only grow on a real screen change.
        assert_eq!(ticks.iter().map(|t| t.spans).collect::<Vec<_>>(), [1, 1, 2]);
        // A merged frame's elements are discarded, so the count must not double up.
        assert_eq!(tl.stats.elements, 2);
        assert_eq!(tl.stats.frames_read, 3);
        assert_eq!(tl.stats.duration_secs, 3.0);
    }

    #[test]
    fn elements_carry_normalized_boxes_ids_and_source() {
        let engine = StubEngine::new(vec![vec!["Export"]]);
        let tl = build_timeline(
            &[frame(0.0)],
            1.0,
            &engine,
            &TimelineOpts::default(),
            &mut |_| {},
        )
        .unwrap();
        let el = &tl.spans[0].elements[0];
        assert_eq!(el.id, 0);
        assert_eq!(el.kind, "text");
        assert_eq!(el.content, "Export");
        assert_eq!(el.source, "stub");
        // 100x10 box in a 100x100 frame.
        assert_eq!(el.bbox, [0.0, 0.0, 1.0, 0.1]);
        // A structured element must never leak raw pixel coordinates.
        assert!(el.bbox.iter().all(|v| (0.0..=1.0).contains(v)));
    }

    #[test]
    fn normalize_bbox_is_fraction_of_frame() {
        assert_eq!(
            normalize_bbox(0, 0, 960, 540, 1920, 1080),
            [0.0, 0.0, 0.5, 0.5]
        );
        // Overflowing boxes clamp to 1.0.
        assert_eq!(normalize_bbox(1900, 0, 100, 50, 1920, 1080)[2], 1.0);
    }

    #[test]
    fn normalize_text_ignores_case_and_spacing() {
        assert_eq!(normalize_text("  File   Edit "), "file edit");
    }

    #[test]
    fn jaccard_matches_expected() {
        let a = vec!["file".to_string(), "edit".to_string(), "view".to_string()];
        let b = vec!["file".to_string(), "edit".to_string(), "view".to_string()];
        assert_eq!(jaccard(&a, &b), 1.0);
        let c = vec!["file".to_string()];
        assert!(jaccard(&a, &c) < MERGE_JACCARD);
        // A blank screen staying blank is the same state, not a change.
        assert_eq!(jaccard(&[], &[]), 1.0);
    }
}
