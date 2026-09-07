use serde::{Deserialize, Serialize};

use crate::anim::{
    build_press_events_from_iter, click_anchor_at, click_highlight_at, press_state_at, PressEvent,
};
use crate::sample::{CursorSample, IdlePeriod};

/// Shared 200 ms ramp at each end of an idle period.
pub const IDLE_FADE_US: i64 = 200_000;

/// The recorded track. Press events are derived once on construction because
/// every frame needs them and rebuilding per frame is O(samples) per frame.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CursorTrack {
    pub samples: Vec<CursorSample>,
    #[serde(default)]
    pub idle_periods: Vec<IdlePeriod>,
    #[serde(skip)]
    press_events: Vec<PressEvent>,
}

/// The cursor knobs the frame evaluation needs. A subset of the scene's cursor
/// layer, passed in so this crate does not depend on the scene model.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CursorSettings {
    pub hide_when_idle: bool,
    /// Seconds of stillness before the hide ramp starts.
    pub idle_timeout: f64,
    pub highlight_clicks: bool,
    /// Authored 0..100.
    pub highlight_opacity: f64,
}

/// Everything a renderer needs for one frame. Positions are in source UV before
/// the zoom transform, because the sprite and the highlight apply it differently.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CursorPlacement {
    pub x: f64,
    pub y: f64,
    pub alpha: f64,
    pub pressed: bool,
    pub right: bool,
    pub dragging: bool,
    pub scale: f64,
    pub highlight: Option<Highlight>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Highlight {
    pub x: f64,
    pub y: f64,
    pub alpha: f64,
}

impl CursorTrack {
    pub fn new(samples: Vec<CursorSample>, idle_periods: Vec<IdlePeriod>) -> Self {
        let mut track = Self {
            samples,
            idle_periods,
            press_events: Vec::new(),
        };
        track.rebuild_press_events();
        track
    }

    /// Call after deserialising: the press events are derived rather than
    /// stored, so a round-tripped track would otherwise have no clicks at all.
    pub fn rebuild_press_events(&mut self) {
        self.press_events = build_press_events_from_iter(
            self.samples
                .iter()
                .map(|s| (s.timestamp_us, s.x, s.y, s.left_down, s.right_down)),
        );
    }

    pub fn press_events(&self) -> &[PressEvent] {
        &self.press_events
    }

    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    /// `ease` reshapes the parameter between two captured samples; pass the
    /// identity when no cursor-motion easing is set.
    pub fn resolve(
        &self,
        ts_us: i64,
        source: (u32, u32),
        settings: CursorSettings,
        ease: impl Fn(f64) -> f64,
    ) -> Option<CursorPlacement> {
        if self.samples.is_empty() {
            return None;
        }
        let idle_alpha = match settings.hide_when_idle {
            true => idle_alpha_at(&self.idle_periods, ts_us, settings.idle_timeout),
            false => 1.0,
        };
        let press = press_state_at(ts_us, &self.press_events);
        let base_alpha = idle_alpha.max(press.visible_alpha);

        let (width, height) = (source.0.max(1) as f64, source.1.max(1) as f64);
        let highlight = match settings.highlight_clicks {
            true => click_highlight_at(ts_us, &self.press_events).map(|(x, y, a)| Highlight {
                x: x / width,
                y: y / height,
                alpha: (settings.highlight_opacity / 100.0) * a,
            }),
            false => None,
        };

        let hidden = |highlight: Option<Highlight>| {
            highlight.map(|highlight| CursorPlacement {
                x: 0.0,
                y: 0.0,
                alpha: 0.0,
                pressed: false,
                right: false,
                dragging: false,
                scale: 1.0,
                highlight: Some(highlight),
            })
        };

        if base_alpha <= 0.0 {
            return hidden(highlight);
        }
        let Some(sample) = interpolate_at(&self.samples, ts_us, ease) else {
            return hidden(highlight);
        };
        if !sample.visible {
            return hidden(highlight);
        }

        let (mut x, mut y) = (sample.x, sample.y);
        if let Some((ax, ay, weight)) = click_anchor_at(ts_us, &self.press_events) {
            x = x * (1.0 - weight) + ax * weight;
            y = y * (1.0 - weight) + ay * weight;
        }

        Some(CursorPlacement {
            x: x / width,
            y: y / height,
            alpha: base_alpha,
            pressed: press.pressed_sprite,
            right: press.right,
            dragging: press.dragged,
            scale: press.scale,
            highlight,
        })
    }
}

/// Position and button state at `ts_us`. Booleans flip at the midpoint of the
/// LINEAR parameter even when eased, so click timing stays predictable.
pub fn interpolate_at(
    samples: &[CursorSample],
    ts_us: i64,
    ease: impl Fn(f64) -> f64,
) -> Option<CursorSample> {
    if samples.is_empty() {
        return None;
    }
    let ts = ts_us.max(0) as u64;
    let idx = samples.partition_point(|s| s.timestamp_us < ts);
    if idx >= samples.len() {
        return samples.last().copied();
    }
    if idx == 0 || samples[idx].timestamp_us == ts {
        return Some(samples[idx]);
    }

    let (a, b) = (samples[idx - 1], samples[idx]);
    let range = b.timestamp_us - a.timestamp_us;
    let t_linear = match range > 0 {
        true => (ts - a.timestamp_us) as f64 / range as f64,
        false => 0.0,
    };
    let t = ease(t_linear);
    let pick_a = t_linear < 0.5;
    Some(CursorSample {
        timestamp_us: ts,
        x: a.x + (b.x - a.x) * t,
        y: a.y + (b.y - a.y) * t,
        visible: match pick_a {
            true => a.visible,
            false => b.visible,
        },
        left_down: match pick_a {
            true => a.left_down,
            false => b.left_down,
        },
        right_down: match pick_a {
            true => a.right_down,
            false => b.right_down,
        },
    })
}

/// 1 outside any idle period, 0 deep inside, with a symmetric ramp at each end.
pub fn idle_alpha_at(periods: &[IdlePeriod], ts_us: i64, idle_timeout_sec: f64) -> f64 {
    let threshold_us = (idle_timeout_sec * 1_000_000.0) as i64;
    for period in periods {
        let (start, end) = (period.start_us as i64, period.end_us as i64);
        let fade_start = start + threshold_us;
        if end <= fade_start {
            continue;
        }
        let fade_end = (fade_start + IDLE_FADE_US).min(end);
        let resume_start = (end - IDLE_FADE_US).max(fade_end);
        if ts_us < fade_start || ts_us > end {
            continue;
        }
        if ts_us >= fade_end && ts_us <= resume_start {
            return 0.0;
        }
        if ts_us < fade_end {
            return 1.0 - (ts_us - fade_start) as f64 / (fade_end - fade_start) as f64;
        }
        return 1.0 - (end - ts_us) as f64 / (end - resume_start) as f64;
    }
    1.0
}
