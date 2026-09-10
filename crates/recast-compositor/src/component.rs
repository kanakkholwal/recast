//! Component instances as the renderer takes them: a recipe number, a surface, and the parameters flattened into two vec4s.
//! The recipe-to-slot mapping is here rather than in the shader's head, so a parameter that moves is one edit in one place.

use recast_color::Srgba;
use recast_scene::component::{self, GraphicSpec, Manifest, Surface};

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
            params.p0 = [
                num(manifest, spec, "radius", 0.08),
                num(manifest, spec, "angle", 0.0).to_radians(),
                num(manifest, spec, "softness", 0.08),
                num(manifest, spec, "hold", 0.2),
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
        _ => {
            // Unresolved: a written placement is honoured, so one missing component marks its corner rather than the frame.
            params.rect = placement(manifest, spec, surface);
        }
    }
    params
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

    #[test]
    fn every_registry_entry_has_a_recipe_the_shader_answers_to() {
        let arms = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/shaders/component.wgsl"
        ))
        .expect("the shader source");
        for manifest in recast_scene::component::REGISTRY {
            let arm = format!("case {}u:", manifest.recipe);
            assert!(
                arms.contains(&arm),
                "{} pins recipe {} and the shader has no arm for it",
                manifest.name,
                manifest.recipe
            );
        }
    }

    #[test]
    fn every_registered_parameter_is_read_by_the_recipe_that_declares_it() {
        let placement = ["x", "y", "w", "h"];
        for manifest in recast_scene::component::REGISTRY {
            for param in manifest.params {
                if placement.contains(&param.name) {
                    continue;
                }
                let moved = params_for(
                    &spec(
                        &format!(
                            "{}@{}.{}",
                            manifest.name, manifest.version.major, manifest.version.minor
                        ),
                        &[(param.name, alternative(param))],
                    ),
                    1.0,
                    CANVAS,
                    0.0,
                    None,
                );
                let default = params_for(
                    &spec(
                        &format!(
                            "{}@{}.{}",
                            manifest.name, manifest.version.major, manifest.version.minor
                        ),
                        &[],
                    ),
                    1.0,
                    CANVAS,
                    0.0,
                    None,
                );
                assert_ne!(
                    moved, default,
                    "{}.{} is declared but changes nothing",
                    manifest.name, param.name
                );
            }
        }
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
        }
    }
}
