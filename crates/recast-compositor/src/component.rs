//! Component instances as the renderer takes them: a recipe number, a surface, and the parameters flattened into two vec4s.
//! The recipe-to-slot mapping is here rather than in the shader's head, so a parameter that moves is one edit in one place.

use recast_color::Srgba;
use recast_scene::component::{self, GraphicSpec, Manifest, Surface};
use recast_scene::composition::TextAlign;

use crate::eval::TextItemDraw;
use crate::plane::Homography;

/// One instance ready to draw.
#[derive(Debug, Clone, PartialEq)]
pub struct ComponentParams {
    pub recipe: u32,
    /// Where it draws, in canvas pixels.
    pub rect: [f32; 4],
    /// Progress through its own window, 0 to 1.
    pub progress: f32,
    pub p0: [f32; 4],
    pub p1: [f32; 4],
    pub tint: Srgba,
    /// The second colour, for a recipe that blends two.
    pub tint2: Srgba,
    /// Set when it rides a tilted card.
    pub warp: Option<Homography>,
}

/// The surface an instance draws on, once the manifest has had its say.
#[must_use]
pub fn surface_of(spec: &GraphicSpec) -> Surface {
    match spec.resolution() {
        component::Resolution::Ready { surface, .. } => surface,
        _ => spec.fallback_surface,
    }
}

/// Reads a parameter through the manifest, so an unset one is its declared default rather than zero.
/// With no manifest, the instance's own text is all there is, which is what a placeholder places itself by.
fn raw<'a>(manifest: Option<&'a Manifest>, spec: &'a GraphicSpec, name: &str) -> Option<&'a str> {
    manifest
        .and_then(|m| m.value(&spec.params, name))
        .or_else(|| spec.params.get(name).map(String::as_str))
}

fn num(manifest: Option<&Manifest>, spec: &GraphicSpec, name: &str, fallback: f64) -> f32 {
    raw(manifest, spec, name)
        .and_then(|text| text.trim().parse::<f64>().ok())
        .unwrap_or(fallback) as f32
}

fn colour(manifest: Option<&Manifest>, spec: &GraphicSpec, name: &str) -> Srgba {
    raw(manifest, spec, name)
        .and_then(recast_color::parse_css_color)
        .unwrap_or(Srgba::opaque(255, 255, 255))
}

/// The placeholder's own colour, so an unresolved instance is visible without being mistaken for content.
const PLACEHOLDER_TINT: Srgba = Srgba {
    r: 255,
    g: 0,
    b: 200,
    a: 255,
};

/// Builds the draw parameters for one instance.
/// `surface` is the rect it draws in (the whole canvas for an overlay, the card for a screen recipe) and `radius_px` the rounding to clip to.
#[must_use]
pub fn params_for(
    spec: &GraphicSpec,
    time: f64,
    surface: [f32; 4],
    radius_px: f32,
    warp: Option<Homography>,
) -> ComponentParams {
    let resolution = spec.resolution();
    let recipe = resolution.recipe();
    let manifest = match &resolution {
        component::Resolution::Ready { name, .. } => component::manifest(name),
        _ => None,
    };
    let progress = spec.progress(time) as f32;
    let mut params = ComponentParams {
        recipe,
        rect: surface,
        progress,
        p0: [0.0; 4],
        p1: [0.0, 0.0, 0.0, radius_px],
        tint: PLACEHOLDER_TINT,
        tint2: PLACEHOLDER_TINT,
        warp,
    };
    match recipe {
        1 => {
            params.p0 = [
                num(manifest, spec, "cx", 0.5),
                num(manifest, spec, "cy", 0.5),
                num(manifest, spec, "radius", 0.3),
                num(manifest, spec, "feather", 0.25),
            ];
            params.p1[0] = num(manifest, spec, "strength", 0.6);
            params.tint = colour(manifest, spec, "tint");
        }
        2 => {
            let softness = num(manifest, spec, "softness", 0.08);
            params.p0 = [
                num(manifest, spec, "radius", 0.08),
                num(manifest, spec, "angle", 0.0).to_radians(),
                softness,
                reveal_front(progress, num(manifest, spec, "hold", 0.2), softness),
            ];
            params.tint = colour(manifest, spec, "fill");
            params.rect = placement(manifest, spec, surface);
        }
        3 => {
            params.p0 = [
                num(manifest, spec, "width", 0.25),
                num(manifest, spec, "angle", 35.0).to_radians(),
                num(manifest, spec, "cycles", 1.0),
                num(manifest, spec, "strength", 0.35),
            ];
            params.tint = colour(manifest, spec, "tint");
        }
        4 => {
            params.p0 = [
                num(manifest, spec, "spread", 0.55),
                num(manifest, spec, "cycles", 1.0),
                num(manifest, spec, "strength", 0.85),
                0.0,
            ];
            params.tint = colour(manifest, spec, "a");
            params.tint2 = colour(manifest, spec, "b");
        }
        5 | 6 => {
            let softness = num(manifest, spec, "softness", 0.06);
            params.p0 = [
                num(manifest, spec, "radius", 0.04),
                0.0,
                softness,
                reveal_front(progress, num(manifest, spec, "hold", 0.6), softness),
            ];
            params.tint = colour(manifest, spec, "fill");
            params.rect = placement(manifest, spec, surface);
        }
        _ => {
            // Unresolved: a written placement is honoured, so one missing component marks its corner rather than the frame.
            params.rect = placement(manifest, spec, surface);
        }
    }
    params
}

/// How far the wipe has crossed its box, from before the leading edge to past the trailing one.
/// Computed here rather than in the shader so the panel and the words it carries cannot drift apart.
#[must_use]
pub fn reveal_front(progress: f32, hold: f32, softness: f32) -> f32 {
    let softness = softness.max(1e-3);
    let phase = ((1.0 - hold.clamp(0.0, 1.0)) * 0.5).max(1e-4);
    if progress < phase {
        return lerp(-softness, 1.0 + softness, progress / phase);
    }
    if progress > 1.0 - phase {
        return lerp(
            1.0 + softness,
            -softness,
            (progress - (1.0 - phase)) / phase,
        );
    }
    1.0 + softness
}

/// The same envelope as a scalar, for the text a panel carries: in over the
/// first phase, out over the last, full in between.
#[must_use]
pub fn reveal_alpha(progress: f32, hold: f32) -> f32 {
    let phase = ((1.0 - hold.clamp(0.0, 1.0)) * 0.5).max(1e-4);
    if progress < phase {
        return (progress / phase).clamp(0.0, 1.0);
    }
    if progress > 1.0 - phase {
        return (1.0 - (progress - (1.0 - phase)) / phase).clamp(0.0, 1.0);
    }
    1.0
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// The words a component puts on screen, placed inside its own box.
/// Recipes that draw no text answer with nothing, which is most of them.
#[must_use]
pub fn text_for(spec: &GraphicSpec, time: f64, surface: [f32; 4]) -> Vec<TextItemDraw> {
    let resolution = spec.resolution();
    let component::Resolution::Ready { name, recipe, .. } = &resolution else {
        return Vec::new();
    };
    let manifest = component::manifest(name);
    let rect = placement(manifest, spec, surface);
    let progress = spec.progress(time) as f32;
    let colour = colour(manifest, spec, "color");
    match recipe {
        5 => stacked(
            &text_of(manifest, spec, "title"),
            &text_of(manifest, spec, "subtitle"),
            rect,
            num(manifest, spec, "size", 0.12) * surface[3],
            num(manifest, spec, "subSize", 0.05) * surface[3],
            align_of(num(manifest, spec, "align", 0.0)),
            colour,
            reveal_alpha(progress, num(manifest, spec, "hold", 0.6)),
        ),
        6 => stacked(
            &text_of(manifest, spec, "name"),
            &text_of(manifest, spec, "role"),
            inset(rect),
            num(manifest, spec, "size", 0.06) * surface[3],
            num(manifest, spec, "subSize", 0.035) * surface[3],
            TextAlign::Start,
            colour,
            reveal_alpha(progress, num(manifest, spec, "hold", 0.7)),
        ),
        7 => stacked(
            &counted(manifest, spec, progress),
            "",
            rect,
            num(manifest, spec, "size", 0.14) * surface[3],
            0.0,
            align_of(num(manifest, spec, "align", 0.0)),
            colour,
            1.0,
        ),
        _ => Vec::new(),
    }
}

/// The number this frame, formatted as the instance asked for it.
fn counted(manifest: Option<&Manifest>, spec: &GraphicSpec, progress: f32) -> String {
    let from = num(manifest, spec, "from", 0.0);
    let to = num(manifest, spec, "to", 100.0);
    let decimals = num(manifest, spec, "decimals", 0.0).clamp(0.0, 6.0) as usize;
    let value = lerp(from, to, progress.clamp(0.0, 1.0));
    format!(
        "{}{:.*}{}",
        text_of(manifest, spec, "prefix"),
        decimals,
        value,
        text_of(manifest, spec, "suffix")
    )
}

/// A headline with an optional second line under it, centred in the box as one block.
#[expect(
    clippy::too_many_arguments,
    reason = "the two strings, their two sizes, the box, the alignment, the colour and the alpha are all independent"
)]
fn stacked(
    lead: &str,
    under: &str,
    rect: [f32; 4],
    lead_px: f32,
    under_px: f32,
    align: TextAlign,
    colour: Srgba,
    alpha: f32,
) -> Vec<TextItemDraw> {
    if lead.trim().is_empty() && under.trim().is_empty() {
        return Vec::new();
    }
    let gap = under_px * 0.6;
    let block = lead_px
        + if under.trim().is_empty() {
            0.0
        } else {
            gap + under_px
        };
    let top = rect[1] + (rect[3] - block).max(0.0) * 0.5;
    let mut out = Vec::new();
    if !lead.trim().is_empty() {
        out.push(TextItemDraw {
            rect: [rect[0], top, rect[2], lead_px],
            content: lead.to_owned(),
            size_px: lead_px,
            color: colour,
            align,
            weight: 700.0,
            line_height: 1.0,
            alpha,
        });
    }
    if !under.trim().is_empty() {
        out.push(TextItemDraw {
            rect: [rect[0], top + lead_px + gap, rect[2], under_px],
            content: under.to_owned(),
            size_px: under_px,
            color: colour,
            align,
            weight: 400.0,
            line_height: 1.0,
            alpha,
        });
    }
    out
}

/// A bar's words sit in from its edges rather than against them.
fn inset(rect: [f32; 4]) -> [f32; 4] {
    let pad = rect[2] * 0.06;
    [rect[0] + pad, rect[1], rect[2] - pad * 2.0, rect[3]]
}

fn text_of(manifest: Option<&Manifest>, spec: &GraphicSpec, name: &str) -> String {
    raw(manifest, spec, name).unwrap_or_default().to_owned()
}

fn align_of(value: f32) -> TextAlign {
    if value < -0.5 {
        return TextAlign::Start;
    }
    if value > 0.5 {
        return TextAlign::End;
    }
    TextAlign::Center
}

/// An overlay recipe that declares `x y w h` places itself inside the canvas; one that does not covers it.
fn placement(manifest: Option<&Manifest>, spec: &GraphicSpec, canvas: [f32; 4]) -> [f32; 4] {
    if raw(manifest, spec, "x").is_none() {
        return canvas;
    }
    let x = num(manifest, spec, "x", 0.0);
    let y = num(manifest, spec, "y", 0.0);
    let w = num(manifest, spec, "w", 1.0);
    let h = num(manifest, spec, "h", 1.0);
    [
        canvas[0] + x * canvas[2],
        canvas[1] + y * canvas[3],
        w * canvas[2],
        h * canvas[3],
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(component: &str, params: &[(&str, &str)]) -> GraphicSpec {
        GraphicSpec {
            id: "g1".into(),
            component: component.into(),
            start: 0.0,
            duration: 4.0,
            params: params
                .iter()
                .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
                .collect(),
            fallback_surface: Surface::Overlay,
        }
    }

    const CANVAS: [f32; 4] = [0.0, 0.0, 200.0, 100.0];

    #[test]
    fn an_unset_parameter_takes_the_manifests_default_rather_than_zero() {
        let set = params_for(
            &spec("spotlight@1.0", &[("radius", "0.75")]),
            2.0,
            CANVAS,
            0.0,
            None,
        );
        let unset = params_for(&spec("spotlight@1.0", &[]), 2.0, CANVAS, 0.0, None);

        assert!((set.p0[2] - 0.75).abs() < 1e-6);
        assert!((unset.p0[2] - 0.3).abs() < 1e-6, "the declared default");
        assert!((unset.p0[0] - 0.5).abs() < 1e-6);
    }

    #[test]
    fn a_component_that_does_not_resolve_draws_the_placeholder_over_its_surface() {
        let params = params_for(&spec("nope@1.0", &[]), 1.0, CANVAS, 0.0, None);

        assert_eq!(params.recipe, component::PLACEHOLDER);
        assert_eq!(params.rect, CANVAS);
        assert_eq!(params.tint, PLACEHOLDER_TINT, "loud, not invisible");
    }

    /// One broken component should mark its own corner, not hatch the frame.
    #[test]
    fn an_unresolved_instance_that_wrote_a_placement_keeps_it() {
        let params = params_for(
            &spec(
                "nope@1.0",
                &[("x", "0.5"), ("y", "0.0"), ("w", "0.5"), ("h", "0.5")],
            ),
            1.0,
            CANVAS,
            0.0,
            None,
        );

        assert_eq!(params.recipe, component::PLACEHOLDER);
        assert_eq!(params.rect, [100.0, 0.0, 100.0, 50.0]);
    }

    #[test]
    fn an_angle_reaches_the_shader_in_radians() {
        let params = params_for(
            &spec("sweep@1.0", &[("angle", "90")]),
            1.0,
            CANVAS,
            0.0,
            None,
        );

        assert!((params.p0[1] - std::f32::consts::FRAC_PI_2).abs() < 1e-6);
    }

    #[test]
    fn progress_is_the_instances_own_window_not_the_timeline() {
        let mut s = spec("spotlight@1.0", &[]);
        s.start = 10.0;
        s.duration = 4.0;

        assert!((params_for(&s, 12.0, CANVAS, 0.0, None).progress - 0.5).abs() < 1e-6);
        assert!((params_for(&s, 3.0, CANVAS, 0.0, None).progress - 0.0).abs() < 1e-6);
    }

    #[test]
    fn a_recipe_with_no_placement_parameters_covers_its_whole_surface() {
        let params = params_for(&spec("spotlight@1.0", &[]), 1.0, CANVAS, 0.0, None);

        assert_eq!(params.rect, CANVAS, "spotlight declares no x/y/w/h");
    }

    #[test]
    fn the_surface_comes_from_the_manifest_and_falls_back_to_the_element() {
        let mut screen = spec("sweep@1.0", &[]);
        screen.fallback_surface = Surface::Overlay;
        assert_eq!(
            surface_of(&screen),
            Surface::Screen,
            "the manifest wins over the element"
        );

        let mut unresolved = spec("nope@1.0", &[]);
        unresolved.fallback_surface = Surface::Screen;
        assert_eq!(
            surface_of(&unresolved),
            Surface::Screen,
            "nothing to ask, so the element decides"
        );
    }

    #[test]
    fn the_corner_radius_of_the_surface_always_reaches_the_same_slot() {
        let params = params_for(&spec("sweep@1.0", &[]), 1.0, CANVAS, 18.0, None);

        assert!((params.p1[3] - 18.0).abs() < 1e-6);
    }

    #[test]
    fn a_parameter_that_is_not_a_number_falls_back_rather_than_poisoning_the_uniform() {
        let params = params_for(
            &spec("spotlight@1.0", &[("radius", "wide")]),
            1.0,
            CANVAS,
            0.0,
            None,
        );

        assert!((params.p0[2] - 0.3).abs() < 1e-6);
        assert!(params.p0.iter().all(|v| v.is_finite()));
    }

    #[test]
    fn an_overlay_that_places_itself_is_positioned_inside_the_canvas() {
        let placed = params_for(
            &spec(
                "shape-reveal@1.0",
                &[("x", "0.25"), ("y", "0.5"), ("w", "0.5"), ("h", "0.25")],
            ),
            1.0,
            CANVAS,
            0.0,
            None,
        );

        assert_eq!(placed.rect, [50.0, 50.0, 100.0, 25.0]);
    }

    /// Every recipe number a `case` label names, whether it stands alone or
    /// shares an arm (`case 5u, 6u:`), which two recipes drawing one panel do.
    fn shader_arms() -> std::collections::BTreeSet<u32> {
        let source = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/shaders/component.wgsl"
        ))
        .expect("the shader source");
        let mut found = std::collections::BTreeSet::new();
        for line in source.lines().map(str::trim) {
            let Some(labels) = line.strip_prefix("case ").and_then(|l| l.split(':').next()) else {
                continue;
            };
            for label in labels.split(',') {
                if let Ok(recipe) = label.trim().trim_end_matches('u').parse::<u32>() {
                    found.insert(recipe);
                }
            }
        }
        found
    }

    #[test]
    fn every_registry_entry_that_draws_has_a_recipe_the_shader_answers_to() {
        let arms = shader_arms();
        assert!(arms.len() >= 4, "the parser found nothing: {arms:?}");

        for manifest in recast_scene::component::REGISTRY {
            assert_eq!(
                arms.contains(&manifest.recipe),
                manifest.shape,
                "{} says shape={} but the shader disagrees",
                manifest.name,
                manifest.shape
            );
        }
    }

    /// Everything one instance puts on screen: the uniform and the words. A
    /// parameter has to move one of them or it is a lie in the manifest.
    fn drawn(component: &str, params: &[(&str, &str)]) -> (ComponentParams, Vec<TextItemDraw>) {
        let instance = spec(component, params);
        (
            params_for(&instance, 1.0, CANVAS, 0.0, None),
            text_for(&instance, 1.0, CANVAS),
        )
    }

    #[test]
    fn every_registered_parameter_is_read_by_the_recipe_that_declares_it() {
        let placement = ["x", "y", "w", "h"];
        for manifest in recast_scene::component::REGISTRY {
            let pin = format!(
                "{}@{}.{}",
                manifest.name, manifest.version.major, manifest.version.minor
            );
            let context = context(manifest.name);
            for param in manifest.params {
                if placement.contains(&param.name) {
                    continue;
                }
                let mut with = context.to_vec();
                with.retain(|(name, _)| *name != param.name);
                with.push((param.name, alternative(param)));

                assert_ne!(
                    drawn(&pin, &with),
                    drawn(&pin, context),
                    "{}.{} is declared but changes nothing",
                    manifest.name,
                    param.name
                );
            }
        }
    }

    #[test]
    fn a_counter_reads_the_number_it_is_at_rather_than_where_it_ends() {
        let quarter = text_for(
            &spec(
                "counter@1.0",
                &[("from", "0"), ("to", "100"), ("prefix", "$")],
            ),
            1.0,
            CANVAS,
        );

        assert_eq!(quarter.len(), 1);
        assert_eq!(quarter[0].content, "$25", "a quarter of the way through");
    }

    #[test]
    fn a_title_card_with_no_subtitle_draws_one_line_and_with_one_draws_two() {
        let alone = text_for(&spec("title-card@1.0", &[("title", "Recast")]), 1.0, CANVAS);
        let pair = text_for(
            &spec(
                "title-card@1.0",
                &[("title", "Recast"), ("subtitle", "A demo")],
            ),
            1.0,
            CANVAS,
        );

        assert_eq!(alone.len(), 1);
        assert_eq!(pair.len(), 2);
        assert!(
            pair[1].rect[1] > pair[0].rect[1],
            "the subtitle sits under it"
        );
        assert!(pair[0].size_px > pair[1].size_px, "and smaller");
    }

    #[test]
    fn a_recipe_that_draws_no_shape_is_still_asked_for_its_words() {
        let counter = recast_scene::component::manifest("counter").expect("registered");

        assert!(!counter.shape, "a counter is text and nothing else");
        assert!(!text_for(&spec("counter@1.0", &[]), 1.0, CANVAS).is_empty());
    }

    #[test]
    fn the_wipe_front_crosses_the_panel_and_comes_back() {
        let hold = 0.6;

        let opening = reveal_front(0.05, hold, 0.06);
        let held = reveal_front(0.5, hold, 0.06);
        let closing = reveal_front(0.98, hold, 0.06);

        assert!(opening < held, "still arriving");
        assert!(closing < held, "and leaving again");
        assert!((reveal_alpha(0.5, hold) - 1.0).abs() < 1e-6);
        assert!(reveal_alpha(0.02, hold) < 0.2, "faded in with it");
    }

    /// A value the parameter's own type allows that is not its default, so a
    /// parameter the recipe ignores shows up as no change at all.
    fn alternative(param: &recast_scene::component::ParamSpec) -> &'static str {
        use recast_scene::component::ParamType;
        match param.ty {
            ParamType::Color => "#0a0b0c",
            ParamType::Bool => "true",
            ParamType::Angle | ParamType::Number { .. } => "7",
            ParamType::Fraction => "0.42",
            ParamType::Text => "changed",
        }
    }

    /// What a component needs set for a dependent parameter to have anything to
    /// act on: a subtitle size means nothing with no subtitle.
    fn context(component: &str) -> &'static [(&'static str, &'static str)] {
        match component {
            "title-card" => &[("subtitle", "Sub")],
            "lower-third" => &[("role", "Role")],
            _ => &[],
        }
    }
}
