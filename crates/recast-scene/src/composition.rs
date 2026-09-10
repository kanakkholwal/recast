//! An authored composition: a sequence of items on the OUTPUT clock, with a transition between neighbours.
//! Unlike everything else in the scene there is no recording underneath, so an item's own window is the whole of its timing.

use serde::{Deserialize, Serialize};

/// How one item gives way to the next. A closed set, like every other vocabulary here.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Transition {
    /// A hard cut: the outgoing item is gone the frame the next one starts.
    #[default]
    None,
    /// The two overlap, one fading out as the other fades in.
    Dissolve,
}

impl Transition {
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "none" => Some(Self::None),
            "dissolve" => Some(Self::Dissolve),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Dissolve => "dissolve",
        }
    }
}

/// A box in output-frame fractions, which is how an authored item is placed: there is no video to anchor to.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemBox {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Default for ItemBox {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            w: 1.0,
            h: 1.0,
        }
    }
}

/// How an image fills its box when the aspects disagree.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Fit {
    /// Fills the box, cropping the overflow.
    #[default]
    Cover,
    /// Fits inside the box, leaving the remainder clear.
    Contain,
    /// Takes the box exactly, aspect be damned.
    Fill,
}

impl Fit {
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "cover" => Some(Self::Cover),
            "contain" => Some(Self::Contain),
            "fill" => Some(Self::Fill),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Cover => "cover",
            Self::Contain => "contain",
            Self::Fill => "fill",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ItemContent {
    #[serde(rename_all = "camelCase")]
    Image {
        /// Resolved by the host, the same way an annotation image is.
        src: String,
        #[serde(default)]
        fit: Fit,
        #[serde(default)]
        radius: f64,
    },
    #[serde(rename_all = "camelCase")]
    Text {
        content: String,
        /// Share of the frame height, so a title is the same size at any output resolution.
        size: f64,
        color: String,
        #[serde(default)]
        align: TextAlign,
        #[serde(default)]
        weight: f64,
        #[serde(default)]
        line_height: f64,
    },
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TextAlign {
    Start,
    #[default]
    Center,
    End,
}

impl TextAlign {
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "start" | "left" => Some(Self::Start),
            "center" => Some(Self::Center),
            "end" | "right" => Some(Self::End),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Center => "center",
            Self::End => "end",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    pub id: String,
    /// Output seconds.
    pub at: f64,
    pub dur: f64,
    #[serde(default = "ItemBox::default")]
    pub area: ItemBox,
    #[serde(default = "one")]
    pub opacity: f64,
    pub content: ItemContent,
}

fn one() -> f64 {
    1.0
}

impl Item {
    #[must_use]
    pub fn end(&self) -> f64 {
        self.at + self.dur.max(0.0)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Composition {
    #[serde(default)]
    pub transition: Transition,
    /// How long a transition takes, in output seconds.
    #[serde(default)]
    pub transition_dur: f64,
    pub items: Vec<Item>,
}

impl Composition {
    /// Where the composition ends, which is what the timeline's output duration becomes.
    #[must_use]
    pub fn duration(&self) -> f64 {
        self.items.iter().map(Item::end).fold(0.0, f64::max)
    }

    /// The items to draw at `time` with the opacity each is at, in document order.
    /// A dissolve overlaps neighbours: the outgoing one fades over the transition while the incoming one rises through it.
    #[must_use]
    pub fn visible(&self, time: f64) -> Vec<(&Item, f64)> {
        let fade = match self.transition {
            Transition::Dissolve => self.transition_dur.max(0.0),
            Transition::None => 0.0,
        };
        self.items
            .iter()
            .filter_map(|item| {
                let alpha = opacity_at(item, time, fade) * item.opacity.clamp(0.0, 1.0);
                (alpha > 0.0).then_some((item, alpha))
            })
            .collect()
    }
}

/// An item's own fade: it rises over `fade` from its start and falls over `fade` before its end.
/// Outside its window it contributes nothing, so a dissolve is two items overlapping rather than a blend of one.
fn opacity_at(item: &Item, time: f64, fade: f64) -> f64 {
    let end = item.end();
    if item.dur <= 0.0 || time < item.at || time > end {
        return 0.0;
    }
    // Never longer than half the item, or a short item would never reach full.
    let fade = fade.min(item.dur * 0.5);
    if fade <= 0.0 {
        return 1.0;
    }
    let rising = (time - item.at) / fade;
    let falling = (end - time) / fade;
    rising.min(falling).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image(id: &str, at: f64, dur: f64) -> Item {
        Item {
            id: id.into(),
            at,
            dur,
            area: ItemBox::default(),
            opacity: 1.0,
            content: ItemContent::Image {
                src: format!("media/{id}.png"),
                fit: Fit::Cover,
                radius: 0.0,
            },
        }
    }

    fn sequence(transition: Transition, dur: f64) -> Composition {
        Composition {
            transition,
            transition_dur: dur,
            items: vec![image("a", 0.0, 4.0), image("b", 3.0, 4.0)],
        }
    }

    #[test]
    fn the_composition_ends_where_its_last_item_does() {
        assert!((sequence(Transition::None, 0.0).duration() - 7.0).abs() < 1e-9);
    }

    #[test]
    fn a_hard_cut_shows_one_item_at_a_time_even_where_the_windows_overlap() {
        let seq = sequence(Transition::None, 0.0);

        let at_overlap = seq.visible(3.5);

        assert_eq!(at_overlap.len(), 2, "both windows cover 3.5");
        assert!(
            at_overlap
                .iter()
                .all(|(_, alpha)| (*alpha - 1.0).abs() < 1e-9),
            "with no transition neither is faded; the later one simply draws on top"
        );
    }

    #[test]
    fn a_dissolve_hands_over_with_the_two_alphas_crossing() {
        let seq = sequence(Transition::Dissolve, 1.0);

        let early = seq.visible(3.25);
        let late = seq.visible(3.75);

        let alpha = |v: &[(&Item, f64)], id: &str| {
            v.iter()
                .find(|(i, _)| i.id == id)
                .map(|(_, a)| *a)
                .unwrap_or(0.0)
        };
        assert!(
            alpha(&early, "a") > alpha(&early, "b"),
            "the outgoing one still leads"
        );
        assert!(
            alpha(&late, "b") > alpha(&late, "a"),
            "and the incoming one takes over"
        );
        assert!(
            (alpha(&early, "a") + alpha(&early, "b") - 1.0).abs() < 0.26,
            "no flash of black"
        );
    }

    #[test]
    fn an_item_shorter_than_the_transition_still_reaches_full() {
        let seq = Composition {
            transition: Transition::Dissolve,
            transition_dur: 10.0,
            items: vec![image("a", 0.0, 1.0)],
        };

        let peak = seq.visible(0.5);

        assert_eq!(peak.len(), 1);
        assert!(
            (peak[0].1 - 1.0).abs() < 1e-9,
            "the fade is capped at half the item"
        );
    }

    #[test]
    fn nothing_draws_outside_an_items_window_or_at_zero_duration() {
        let seq = sequence(Transition::Dissolve, 1.0);

        assert!(seq.visible(-0.5).is_empty());
        assert!(seq.visible(99.0).is_empty());
        assert!(Composition {
            transition: Transition::Dissolve,
            transition_dur: 1.0,
            items: vec![image("a", 0.0, 0.0)],
        }
        .visible(0.0)
        .is_empty());
    }

    #[test]
    fn an_items_own_opacity_multiplies_the_transition_rather_than_replacing_it() {
        let mut seq = sequence(Transition::Dissolve, 1.0);
        seq.items[0].opacity = 0.5;

        let held = seq.visible(2.0);

        let a = held
            .iter()
            .find(|(i, _)| i.id == "a")
            .expect("a is on screen");
        assert!(
            (a.1 - 0.5).abs() < 1e-9,
            "full through the transition, halved by its own opacity"
        );
    }

    /// The editor and the CLI both round-trip this, so the tags are the contract.
    #[test]
    fn the_wire_shape_tags_each_kind_by_name() {
        let json = serde_json::to_value(image("a", 1.0, 2.0)).expect("serialize");

        assert_eq!(json["content"]["kind"], "image");
        assert_eq!(json["content"]["fit"], "cover");
        assert_eq!(
            json["area"],
            serde_json::json!({"x": 0.0, "y": 0.0, "w": 1.0, "h": 1.0})
        );
        let back: Item = serde_json::from_value(json).expect("deserialize");
        assert_eq!(back, image("a", 1.0, 2.0));
    }

    #[test]
    fn every_spelling_the_document_uses_parses_back_to_itself() {
        for t in [Transition::None, Transition::Dissolve] {
            assert_eq!(Transition::parse(t.as_str()), Some(t));
        }
        for f in [Fit::Cover, Fit::Contain, Fit::Fill] {
            assert_eq!(Fit::parse(f.as_str()), Some(f));
        }
        for a in [TextAlign::Start, TextAlign::Center, TextAlign::End] {
            assert_eq!(TextAlign::parse(a.as_str()), Some(a));
        }
        assert_eq!(
            TextAlign::parse("left"),
            Some(TextAlign::Start),
            "the CSS spelling too"
        );
    }
}
