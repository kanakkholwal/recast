//! The one typed mapping between the document and the engine's `Scene`, both ways.
//! Units differ on purpose: the document is `0..1` and seconds, the v1 structs inside `Scene` keep `0..100` and milliseconds; every conversion lives here.

mod read;
#[cfg(test)]
mod tests;
mod write;

use recast_scene::v1::Easing;

use crate::document::Node;
use crate::value::{self, Ease};

pub use read::{to_render_state, to_scene, MapError};
pub use write::{
    from_render_state as from_render_state_public, from_scene, MediaFile, MediaRefs, TrackFile,
};

/// Ids of the singleton elements, fixed so a document is readable and two migrations of one bundle agree.
pub mod ids {
    pub const RECORDING: &str = "rec";
    pub const CAMERA_MEDIA: &str = "cam";
    pub const MIC: &str = "mic";
    pub const SYSTEM: &str = "sys";
    pub const CURSOR_TRACK: &str = "cur";
    pub const WORDS_TRACK: &str = "wrd";
    pub const SCREEN: &str = "scr";
    pub const CAMERA: &str = "bubble";
    pub const CURSOR: &str = "pointer";
    pub const CAPTIONS: &str = "caps";
    pub const FOLLOW: &str = "follow";
    pub const DODGE: &str = "dodge";
    pub const KEYS: &str = "keys";
}

/// Passthrough keys the mapping gives a home; anything else lands under `<unknowns>`.
pub const PLACED_PASSTHROUGH: &[&str] = &[
    "transcript",
    "cursorStyle",
    "dismissedSilences",
    "cutsEnabled",
    "autoZoomEnabled",
    "autoZoomApplied",
    "layoutMode",
    "lastAppliedPresetId",
    "motionTone",
];

const EPS: f64 = 1e-9;

pub(crate) fn frac_from_pct(v: f64) -> f64 {
    v / 100.0
}

// Rounded to 1e-9 so `0.4 * 100` comes back as the `40` it was written from.
pub(crate) fn pct_from_frac(v: f64) -> f64 {
    (v * 100.0 * 1e9).round() / 1e9
}

pub(crate) fn secs_from_ms(v: f64) -> f64 {
    v / 1000.0
}

pub(crate) fn ms_from_secs(v: f64) -> f64 {
    (v * 1000.0 * 1e6).round() / 1e6
}

pub(crate) fn ease_from(e: Easing) -> Ease {
    Ease {
        x1: e.x1,
        y1: e.y1,
        x2: e.x2,
        y2: e.y2,
    }
}

pub(crate) fn easing_from(e: Ease) -> Easing {
    Easing {
        x1: e.x1,
        y1: e.y1,
        x2: e.x2,
        y2: e.y2,
    }
}

/// Typed writes that elide the default, so an untouched project serialises to nothing.
pub(crate) trait Elide {
    fn num(&mut self, name: &str, value: f64, default: f64);
    fn secs(&mut self, name: &str, value: f64, default: f64);
    fn frac(&mut self, name: &str, value: f64, default: f64);
    fn text(&mut self, name: &str, value: &str, default: &str);
    fn ease(&mut self, name: &str, value: Easing, default: Easing);
    fn color(&mut self, name: &str, value: &str, default: &str);
}

impl Elide for Node {
    fn num(&mut self, name: &str, value: f64, default: f64) {
        if (value - default).abs() > EPS {
            self.set(name, value::fmt_num(value));
        }
    }

    fn secs(&mut self, name: &str, value: f64, default: f64) {
        if (value::quantize_ms(value) - value::quantize_ms(default)).abs() > EPS {
            self.set(name, value::fmt_secs(value));
        }
    }

    fn frac(&mut self, name: &str, value: f64, default: f64) {
        self.num(name, value, default);
    }

    fn text(&mut self, name: &str, value: &str, default: &str) {
        if value != default {
            self.set(name, value);
        }
    }

    fn ease(&mut self, name: &str, value: Easing, default: Easing) {
        if value != default {
            self.set(name, value::fmt_ease(ease_from(value)));
        }
    }

    fn color(&mut self, name: &str, value: &str, default: &str) {
        if value != default {
            self.set(name, canonical_color(value));
        }
    }
}

/// Typed reads that fall back to the default the writer elided against.
pub(crate) trait Read {
    fn num_or(&self, name: &str, default: f64) -> f64;
    fn int_or(&self, name: &str, default: i64) -> i64;
    fn text_or(&self, name: &str, default: &str) -> String;
    fn ease_or(&self, name: &str, default: Easing) -> Easing;
    fn flag_or(&self, name: &str, default: bool) -> bool;
}

impl Read for Node {
    fn num_or(&self, name: &str, default: f64) -> f64 {
        self.attr(name)
            .and_then(|v| value::parse_num(v).ok())
            .unwrap_or(default)
    }

    fn int_or(&self, name: &str, default: i64) -> i64 {
        self.attr(name)
            .and_then(|v| value::parse_int(v).ok())
            .unwrap_or(default)
    }

    fn text_or(&self, name: &str, default: &str) -> String {
        self.attr(name)
            .map_or_else(|| default.to_owned(), str::to_owned)
    }

    fn ease_or(&self, name: &str, default: Easing) -> Easing {
        self.attr(name)
            .and_then(|v| value::parse_ease(v).ok())
            .map_or(default, easing_from)
    }

    fn flag_or(&self, name: &str, default: bool) -> bool {
        self.attr(name)
            .and_then(|v| value::parse_bool(v).ok())
            .unwrap_or(default)
    }
}

/// v1 colours are whatever CSS the editor wrote; the document takes hex only, so the rest is rendered through `recast-color`.
pub(crate) fn canonical_color(css: &str) -> String {
    match value::parse_color(css) {
        Ok(hex) => hex,
        Err(_) => recast_color::parse_css_color(css).map_or_else(|| css.to_owned(), |c| c.to_hex()),
    }
}

pub(crate) fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-4
}
