//! OCR engine abstraction. `OcrsEngine` ships now; native OS engines slot in behind the same trait without touching the timeline code.

#[cfg(feature = "ocr")]
use std::path::Path;

#[cfg(feature = "ocr")]
use ocrs::{ImageSource, OcrEngine as Ocrs, OcrEngineParams};
#[cfg(feature = "ocr")]
use rten::Model;
#[cfg(feature = "ocr")]
use rten_imageproc::{BoundingRect, RotatedRect};

/// One recognized line of text with its axis-aligned bounding box, in the pixel
/// space of the frame that was recognized.
#[derive(Debug, Clone)]
pub struct OcrLine {
    pub text: String,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

/// A source of OCR over raw RGBA frames. Used sequentially (one frame at a time);
/// the video sampler already keeps the frame count small, so there is no need to
/// assume the underlying engine is `Sync`.
pub trait OcrEngine {
    /// Recognize text lines in a raw RGBA buffer of the given dimensions.
    fn recognize(&self, rgba: &[u8], width: u32, height: u32) -> Result<Vec<OcrLine>, String>;
    /// Stable identifier for the engine that produced a line, e.g. `"ocrs"`.
    fn source(&self) -> &'static str;
}

/// Pure-Rust ocrs on rten; the detection and recognition models load ONCE, since per-frame loading would dominate a whole-video pass.
/// The only part needing the ocrs crates, so the only part behind the `ocr` feature; everything else compiles and reports the engine as absent.
#[cfg(feature = "ocr")]
pub struct OcrsEngine {
    inner: Ocrs,
}

#[cfg(feature = "ocr")]
impl OcrsEngine {
    pub fn new(det_model: &Path, rec_model: &Path) -> Result<Self, String> {
        let detection_model =
            Model::load_file(det_model).map_err(|e| format!("load detection model: {e}"))?;
        let recognition_model =
            Model::load_file(rec_model).map_err(|e| format!("load recognition model: {e}"))?;
        let inner = Ocrs::new(OcrEngineParams {
            detection_model: Some(detection_model),
            recognition_model: Some(recognition_model),
            ..Default::default()
        })
        .map_err(|e| format!("init OCR engine: {e}"))?;
        Ok(Self { inner })
    }
}

#[cfg(feature = "ocr")]
impl OcrEngine for OcrsEngine {
    fn recognize(&self, rgba: &[u8], width: u32, height: u32) -> Result<Vec<OcrLine>, String> {
        // ocrs wants RGB; drop the alpha channel from the frame's RGBA buffer.
        let rgba_img = image::RgbaImage::from_raw(width, height, rgba.to_vec())
            .ok_or_else(|| "RGBA buffer size does not match dimensions".to_string())?;
        let rgb = image::DynamicImage::ImageRgba8(rgba_img).into_rgb8();
        let source = ImageSource::from_bytes(rgb.as_raw(), rgb.dimensions())
            .map_err(|e| format!("image source: {e}"))?;
        let input = self
            .inner
            .prepare_input(source)
            .map_err(|e| format!("prepare input: {e}"))?;

        // Detect boxes, group into reading-order lines, then recognize: `get_text()` would discard the geometry we need.
        let words = self
            .inner
            .detect_words(&input)
            .map_err(|e| format!("detect words: {e}"))?;
        let lines = self.inner.find_text_lines(&input, &words);
        let recognized = self
            .inner
            .recognize_text(&input, &lines)
            .map_err(|e| format!("recognize text: {e}"))?;

        let mut out = Vec::new();
        for (line_words, line) in lines.iter().zip(recognized.iter()) {
            let Some(line) = line.as_ref() else { continue };
            let text = line.to_string();
            if text.trim().is_empty() {
                continue;
            }
            if let Some((x, y, w, h)) = union_bounds(line_words) {
                out.push(OcrLine {
                    text,
                    x,
                    y,
                    width: w,
                    height: h,
                });
            }
        }
        Ok(out)
    }

    fn source(&self) -> &'static str {
        "ocrs"
    }
}

/// Axis-aligned union of a line's word rectangles as `(x, y, width, height)`.
#[cfg(feature = "ocr")]
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
