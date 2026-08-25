use crate::sample::{ClickAnchor, CursorSample};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SmoothingOptions {
    /// Gaussian sigma in milliseconds. Zero or less returns the input unchanged.
    pub sigma_ms: f64,
    pub snap_to_clicks: bool,
    /// Half-width of the cosine snap ramp, in milliseconds.
    pub snap_window_ms: f64,
}

impl Default for SmoothingOptions {
    fn default() -> Self {
        Self {
            sigma_ms: 0.0,
            snap_to_clicks: true,
            snap_window_ms: 80.0,
        }
    }
}

/// The 0..100 UI slider as a Gaussian sigma in milliseconds.
pub fn smoothing_strength_to_sigma_ms(strength: f64) -> f64 {
    strength.clamp(0.0, 100.0) * 1.5
}

pub struct SmoothResult {
    pub samples: Vec<CursorSample>,
    pub click_anchors: Vec<ClickAnchor>,
}

/// Time-weighted Gaussian window, then an optional cosine-shaped pull through
/// the exact click positions. Without the snap, smoothing rounds the corner at
/// a click and the pointer misses what the user actually clicked.
pub fn smooth_cursor_path(raw: &[CursorSample], opts: SmoothingOptions) -> SmoothResult {
    let click_anchors = click_anchors(raw);

    if raw.len() < 2 || opts.sigma_ms <= 0.0 {
        return SmoothResult {
            samples: raw.to_vec(),
            click_anchors,
        };
    }

    let sigma_us = opts.sigma_ms * 1000.0;
    let window_us = sigma_us * 3.0;
    let snap_us = opts.snap_window_ms.max(0.0) * 1000.0;
    let inv_2_sigma2 = 1.0 / (2.0 * sigma_us * sigma_us);

    // lo and hi only ever advance, because the samples are time-sorted.
    let mut smoothed = Vec::with_capacity(raw.len());
    let mut lo = 0usize;
    let mut hi = 0usize;
    for center in raw {
        let min_t = center.timestamp_us as f64 - window_us;
        let max_t = center.timestamp_us as f64 + window_us;
        while lo < raw.len() && (raw[lo].timestamp_us as f64) < min_t {
            lo += 1;
        }
        while hi < raw.len() && (raw[hi].timestamp_us as f64) <= max_t {
            hi += 1;
        }

        let (mut sum_w, mut sum_x, mut sum_y) = (0.0, 0.0, 0.0);
        for s in &raw[lo..hi] {
            let dt = s.timestamp_us as f64 - center.timestamp_us as f64;
            let w = (-(dt * dt) * inv_2_sigma2).exp();
            sum_w += w;
            sum_x += w * s.x;
            sum_y += w * s.y;
        }
        smoothed.push(match sum_w > 0.0 {
            true => CursorSample {
                x: sum_x / sum_w,
                y: sum_y / sum_w,
                ..*center
            },
            false => *center,
        });
    }

    if opts.snap_to_clicks && snap_us > 0.0 {
        for anchor in &click_anchors {
            for s in smoothed.iter_mut() {
                let dt = (s.timestamp_us as f64 - anchor.timestamp_us as f64).abs();
                if dt > snap_us {
                    continue;
                }
                let falloff = 0.5 + 0.5 * ((dt / snap_us) * std::f64::consts::PI).cos();
                s.x = s.x * (1.0 - falloff) + anchor.x * falloff;
                s.y = s.y * (1.0 - falloff) + anchor.y * falloff;
            }
        }
    }

    SmoothResult {
        samples: smoothed,
        click_anchors,
    }
}

/// Rising edges of either button. Collected even when smoothing is off, because
/// the editor draws them on the timeline.
fn click_anchors(raw: &[CursorSample]) -> Vec<ClickAnchor> {
    raw.windows(2)
        .filter(|pair| {
            let (prev, curr) = (pair[0], pair[1]);
            (!prev.left_down && curr.left_down) || (!prev.right_down && curr.right_down)
        })
        .map(|pair| ClickAnchor {
            timestamp_us: pair[1].timestamp_us,
            x: pair[1].x,
            y: pair[1].y,
        })
        .collect()
}
