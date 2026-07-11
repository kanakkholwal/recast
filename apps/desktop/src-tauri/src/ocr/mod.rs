//! On-device OCR for screen understanding (agent automation).
//!
//! Runs the pure-Rust [`ocrs`] engine on its `rten` runtime (no C / ONNX-Runtime
//! dependency), so it builds on every target including Intel Mac, mirroring the
//! `tract` choice for silence detection. This is a leaf module: it takes a
//! screenshot's raw RGBA pixels (produced by the capture layer) plus the two
//! ocrs `.rten` model paths, and returns recognized text lines with bounding
//! boxes. Model download-on-first-use and the CLI / command / MCP wiring layer
//! on top of this. Gated behind the off-by-default `ocr` Cargo feature.

use std::path::Path;

use ocrs::{ImageSource, OcrEngine, OcrEngineParams};
use rten::Model;
use rten_imageproc::{BoundingRect, RotatedRect};
use serde::Serialize;

/// One recognized line of text with its axis-aligned bounding box, in pixels.
/// This is the unit an LLM agent reasons over ("text X at box Y").
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TextRegion {
    pub text: String,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

/// Recognize text in a raw RGBA screenshot buffer. `det_model` / `rec_model` are
/// the ocrs `.rten` detection + recognition model files. Returns one region per
/// detected text line (empty lines dropped).
pub fn recognize_rgba(
    det_model: &Path,
    rec_model: &Path,
    rgba: &[u8],
    width: u32,
    height: u32,
) -> Result<Vec<TextRegion>, String> {
    let detection_model =
        Model::load_file(det_model).map_err(|e| format!("load detection model: {e}"))?;
    let recognition_model =
        Model::load_file(rec_model).map_err(|e| format!("load recognition model: {e}"))?;
    let engine = OcrEngine::new(OcrEngineParams {
        detection_model: Some(detection_model),
        recognition_model: Some(recognition_model),
        ..Default::default()
    })
    .map_err(|e| format!("init OCR engine: {e}"))?;

    // ocrs wants RGB; drop the alpha channel from the screenshot's RGBA buffer.
    let rgba_img = image::RgbaImage::from_raw(width, height, rgba.to_vec())
        .ok_or_else(|| "RGBA buffer size does not match dimensions".to_string())?;
    let rgb = image::DynamicImage::ImageRgba8(rgba_img).into_rgb8();
    let source = ImageSource::from_bytes(rgb.as_raw(), rgb.dimensions())
        .map_err(|e| format!("image source: {e}"))?;
    let input = engine
        .prepare_input(source)
        .map_err(|e| format!("prepare input: {e}"))?;

    let words = engine
        .detect_words(&input)
        .map_err(|e| format!("detect words: {e}"))?;
    let lines = engine.find_text_lines(&input, &words);
    let recognized = engine
        .recognize_text(&input, &lines)
        .map_err(|e| format!("recognize text: {e}"))?;

    let mut regions = Vec::new();
    for (line_words, line) in lines.iter().zip(recognized.iter()) {
        let Some(line) = line.as_ref() else { continue };
        let text = line.to_string();
        if text.trim().is_empty() {
            continue;
        }
        if let Some((x, y, w, h)) = union_bounds(line_words) {
            regions.push(TextRegion {
                text,
                x,
                y,
                width: w,
                height: h,
            });
        }
    }
    Ok(regions)
}

/// Axis-aligned union of a line's word rectangles as `(x, y, width, height)`.
fn union_bounds(words: &[RotatedRect]) -> Option<(i32, i32, i32, i32)> {
    let mut iter = words.iter();
    let first = iter.next()?.bounding_rect();
    let mut min_x = first.left();
    let mut min_y = first.top();
    let mut max_x = first.left() + first.width();
    let mut max_y = first.top() + first.height();
    for w in iter {
        let r = w.bounding_rect();
        min_x = min_x.min(r.left());
        min_y = min_y.min(r.top());
        max_x = max_x.max(r.left() + r.width());
        max_y = max_y.max(r.top() + r.height());
    }
    Some((
        min_x as i32,
        min_y as i32,
        (max_x - min_x) as i32,
        (max_y - min_y) as i32,
    ))
}
