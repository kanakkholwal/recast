use recast_scene::composition::ItemContent;
use recast_scene::v1::nodes::AnnotationKind;
use recast_scene::{LayerSource, Scene};

use crate::item_text::face_key_weight;

/// A face some text asks for, keyed the way the session looks faces up.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FaceRequest {
    /// The CSS family stack as the text names it; empty draws with the fallback face.
    pub stack: String,
    pub weight: u16,
}

impl FaceRequest {
    fn new(stack: &str, weight: f64) -> Self {
        Self {
            stack: stack.trim().to_owned(),
            weight: face_key_weight(weight),
        }
    }
}

/// Every face this scene's words ask for: text annotations, component text and composition
/// titles. Hidden annotations are skipped: they draw nothing and must not block an export.
#[must_use]
pub fn wanted_faces(scene: &Scene) -> Vec<FaceRequest> {
    let mut wanted = Vec::new();
    for annotation in scene.annotations() {
        if annotation.hidden {
            continue;
        }
        if let AnnotationKind::Text {
            font_family,
            font_weight,
            ..
        } = &annotation.kind
        {
            add(&mut wanted, FaceRequest::new(font_family, *font_weight));
        }
    }
    for layer in &scene.layers {
        let LayerSource::Graphic(spec) = &layer.source else {
            continue;
        };
        // Sampled across the window, since a component reveals its words over time.
        for fraction in [0.2, 0.4, 0.6, 0.8, 0.95] {
            let time = spec.start + spec.duration * fraction;
            for item in crate::component::text_for(spec, time, [0.0, 0.0, 1.0, 1.0]) {
                add(&mut wanted, FaceRequest::new(&item.font, item.weight));
            }
        }
    }
    for item in scene.composition.iter().flat_map(|c| &c.items) {
        if let ItemContent::Text { font, weight, .. } = &item.content {
            add(&mut wanted, FaceRequest::new(font, *weight));
        }
    }
    wanted
}

/// Two stacks naming one first family at one weight are one face: the session keys them alike.
fn add(wanted: &mut Vec<FaceRequest>, request: FaceRequest) {
    let key = |r: &FaceRequest| (crate::faces::first_family(&r.stack), r.weight);
    if !wanted.iter().any(|seen| key(seen) == key(&request)) {
        wanted.push(request);
    }
}

/// Every image file this scene draws: annotation images and composition slides.
#[must_use]
pub fn wanted_images(scene: &Scene) -> Vec<String> {
    let annotations = scene
        .annotations()
        .into_iter()
        .filter_map(|a| match &a.kind {
            AnnotationKind::Image { path, .. } => Some(path.clone()),
            _ => None,
        });
    let slides = scene
        .composition
        .iter()
        .flat_map(|c| &c.items)
        .filter_map(|item| match &item.content {
            ItemContent::Image { src, .. } => Some(src.clone()),
            ItemContent::Text { .. } => None,
        });
    let mut paths: Vec<String> = Vec::new();
    for path in annotations.chain(slides) {
        if !path.is_empty() && !paths.contains(&path) {
            paths.push(path);
        }
    }
    paths
}

/// The face an unnamed family, or one no face was found for, draws with: the caption
/// style's, which is what the session falls back to.
#[must_use]
pub fn fallback_face(scene: &Scene) -> FaceRequest {
    let style = scene.captions.clone().unwrap_or_default();
    FaceRequest {
        stack: style.font_family.trim().to_owned(),
        weight: u16::try_from(style.font_weight).unwrap_or(400),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use recast_scene::migrate::to_scene;
    use recast_scene::v1::RenderState;

    fn scene(json: serde_json::Value) -> Scene {
        // Over a full default, since a render state names several fields as required.
        let mut state = serde_json::to_value(RenderState::default()).expect("default state");
        if let (Some(base), serde_json::Value::Object(extra)) = (state.as_object_mut(), json) {
            base.extend(extra);
        }
        to_scene(&serde_json::from_value::<RenderState>(state).expect("state"))
    }

    fn text(id: &str, family: &str, weight: f64, hidden: bool) -> serde_json::Value {
        serde_json::json!({
            "id": id, "start": 0.0, "end": 5.0, "hidden": hidden,
            "kind": { "kind": "text", "x": 0.1, "y": 0.1, "w": 0.5, "h": 0.2, "content": "Hi",
                      "fontFamily": family, "fontSize": 0.1, "fontWeight": weight, "color": "#ffffff",
                      "align": "center", "lineHeight": 1.2 }
        })
    }

    fn request(stack: &str, weight: u16) -> FaceRequest {
        FaceRequest {
            stack: stack.to_owned(),
            weight,
        }
    }

    /// Hidden text draws nothing, so asking for its face would block an export for no reason.
    #[test]
    fn a_visible_text_annotation_asks_for_its_face_and_a_hidden_one_does_not() {
        let faces = wanted_faces(&scene(serde_json::json!({
            "annotations": [
                text("a", "'Inter Variable', sans-serif", 600.0, false),
                text("b", "Hidden Face", 400.0, true)
            ]
        })));

        assert_eq!(faces, [request("'Inter Variable', sans-serif", 600)]);
    }

    /// Keyed as the drawing reads them, so a composition's unset weight asks under 0, not a guessed 400.
    #[test]
    fn composition_and_component_text_ask_under_the_key_the_drawing_uses() {
        let faces = wanted_faces(&scene(serde_json::json!({
            "graphics": [{ "id": "g1", "component": "title-card@1.0", "start": 0.0, "duration": 4.0,
                           "params": { "title": "QA", "subtitle": "Sub", "font": "Anton" },
                           "fallbackSurface": "overlay" }],
            "composition": { "items": [{ "id": "t1", "at": 0.0, "dur": 2.0,
                "content": { "kind": "text", "content": "Hi", "font": "Oswald", "size": 0.1, "color": "#ffffff" } }] }
        })));

        assert!(faces.contains(&request("Anton", 700)), "{faces:?}");
        assert!(faces.contains(&request("Anton", 400)), "{faces:?}");
        assert!(faces.contains(&request("Oswald", 0)), "{faces:?}");
    }

    #[test]
    fn two_stacks_naming_one_family_at_one_weight_are_one_face() {
        let faces = wanted_faces(&scene(serde_json::json!({
            "annotations": [text("a", "Anton", 700.0, false), text("b", "'Anton', Impact", 700.0, false)]
        })));

        assert_eq!(faces.len(), 1, "{faces:?}");
    }

    /// The export uploaded annotation images only, so a composition's slides drew nothing.
    #[test]
    fn images_include_composition_slides_once_each() {
        let image = |id: &str| {
            serde_json::json!({ "id": id, "start": 0.0, "end": 5.0,
                "kind": { "kind": "image", "x": 0.0, "y": 0.0, "w": 0.2, "h": 0.2, "path": "C:/a.png" } })
        };
        let images = wanted_images(&scene(serde_json::json!({
            "annotations": [image("i1"), image("i2")],
            "composition": { "items": [{ "id": "s1", "at": 0.0, "dur": 2.0,
                "content": { "kind": "image", "src": "C:/slide.png" } }] }
        })));

        assert_eq!(images, ["C:/a.png", "C:/slide.png"]);
    }

    #[test]
    fn the_fallback_follows_the_caption_style() {
        let state = RenderState {
            caption_style: Some(recast_captions::CaptionStyle {
                font_family: "Anton".into(),
                font_weight: 700,
                ..Default::default()
            }),
            ..Default::default()
        };

        assert_eq!(fallback_face(&to_scene(&state)), request("Anton", 700));
    }
}
