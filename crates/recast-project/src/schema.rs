//! The v3 vocabulary, frozen 2026-09-08: every element, its attributes, their types, and what may nest where.
//! One table feeds the parser's text rule, the writer's attribute order, validation, and the docs; nothing else describes the format.

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AttrType {
    Id,
    /// Source seconds unless the element says `axis="output"`; written with three decimals.
    Seconds,
    Number {
        min: Option<f64>,
        max: Option<f64>,
    },
    /// `0..1`.
    Fraction,
    /// `0..100`.
    Percent,
    Int,
    Bool,
    Text,
    Color,
    /// A colour or the word `none`.
    ColorOrNone,
    Ease,
    Enum(&'static [&'static str]),
    Src,
    /// The id of an element of this kind.
    Ref(&'static str),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdRule {
    Required,
    Optional,
    None,
}

#[derive(Debug, Clone, Copy)]
pub struct AttrSpec {
    pub name: &'static str,
    pub ty: AttrType,
    pub required: bool,
    pub doc: &'static str,
}

pub struct ElementSpec {
    pub kind: &'static str,
    pub doc: &'static str,
    pub id: IdRule,
    pub attrs: Vec<AttrSpec>,
    pub children: Vec<&'static str>,
    /// Carries element content (user text).
    pub text: bool,
    /// Attributes outside the table are parameters, not mistakes (`graphic`).
    pub open_attrs: bool,
}

const fn a(name: &'static str, ty: AttrType, doc: &'static str) -> AttrSpec {
    AttrSpec {
        name,
        ty,
        required: false,
        doc,
    }
}

const fn req(name: &'static str, ty: AttrType, doc: &'static str) -> AttrSpec {
    AttrSpec {
        name,
        ty,
        required: true,
        doc,
    }
}

fn el(
    kind: &'static str,
    doc: &'static str,
    id: IdRule,
    attrs: &[AttrSpec],
    children: &[&'static str],
) -> ElementSpec {
    ElementSpec {
        kind,
        doc,
        id,
        attrs: attrs.to_vec(),
        children: children.to_vec(),
        text: false,
        open_attrs: false,
    }
}

const NUM: AttrType = AttrType::Number {
    min: None,
    max: None,
};
const NON_NEG: AttrType = AttrType::Number {
    min: Some(0.0),
    max: None,
};
const ID: AttrSpec = req("id", AttrType::Id, "");
const AT: AttrSpec = req("at", AttrType::Seconds, "Start, source seconds.");
const DUR: AttrSpec = req("dur", AttrType::Seconds, "Length in seconds.");

const ANNOTATION_COMMON: [AttrSpec; 16] = [
    ID,
    AT,
    DUR,
    a("rampIn", AttrType::Seconds, "Fade-in seconds."),
    a("rampOut", AttrType::Seconds, "Fade-out seconds."),
    a("easeIn", AttrType::Ease, ""),
    a("easeOut", AttrType::Ease, ""),
    a("stroke", NON_NEG, "Stroke width, px."),
    a("strokeColor", AttrType::Color, ""),
    a(
        "strokeStyle",
        AttrType::Enum(&["solid", "dashed", "dotted"]),
        "",
    ),
    a("fill", AttrType::ColorOrNone, ""),
    a("opacity", AttrType::Fraction, ""),
    a("z", AttrType::Int, "Stacking order within annotations."),
    a("locked", AttrType::Bool, ""),
    a("hidden", AttrType::Bool, ""),
    a(
        "space",
        AttrType::Enum(&["video", "frame"]),
        "Coordinate space; video is the default.",
    ),
];

fn annotation(kind: &'static str, doc: &'static str, extra: &[AttrSpec]) -> ElementSpec {
    let mut attrs = ANNOTATION_COMMON.to_vec();
    attrs.extend_from_slice(extra);
    ElementSpec {
        kind,
        doc,
        id: IdRule::Required,
        attrs,
        children: vec!["glow", "bind", "transform"],
        text: false,
        open_attrs: false,
    }
}

const BOX: [AttrSpec; 4] = [
    a("x", NUM, "Left, fraction of the space."),
    a("y", NUM, "Top."),
    a("w", NUM, "Width."),
    a("h", NUM, "Height."),
];

fn build() -> Vec<ElementSpec> {
    let mut out = Vec::new();
    out.extend(root_and_refs());
    out.extend(timeline_group());
    out.extend(background_group());
    out.extend(camera_group());
    out.extend(annotation_group());
    out.extend(caption_group());
    out.extend(audio_and_extras());
    out.extend(extras_group());
    out
}

fn root_and_refs() -> Vec<ElementSpec> {
    vec![
        el(
            "recast",
            "The document root.",
            IdRule::None,
            &[
                req("v", AttrType::Int, "Format version."),
                a(
                    "aspect",
                    AttrType::Enum(&["source", "16:9", "9:16", "1:1", "1.91:1"]),
                    "Output canvas aspect.",
                ),
                a(
                    "pad",
                    AttrType::Percent,
                    "Padding, percent of the shorter source edge.",
                ),
                a("timebase", AttrType::Enum(&["source-seconds"]), ""),
            ],
            &[
                "vars",
                "media",
                "track",
                "timeline",
                "background",
                "screen",
                "camera",
                "cursor",
                "annotations",
                "captions",
                "audio",
                "sequence",
                "graphic",
                "shader",
                "editor",
                "unknowns",
            ],
        ),
        el(
            "vars",
            "Typed variables elements reference as `$name`.",
            IdRule::None,
            &[],
            &["var"],
        ),
        el(
            "var",
            "",
            IdRule::None,
            &[
                req("name", AttrType::Text, ""),
                req(
                    "type",
                    AttrType::Enum(&[
                        "color", "number", "int", "bool", "text", "select", "font", "vec2",
                        "angle", "asset",
                    ]),
                    "",
                ),
                req("value", AttrType::Text, ""),
                a("path", AttrType::Text, "Inspector grouping."),
                a("min", NUM, ""),
                a("max", NUM, ""),
                a("step", NUM, ""),
                a(
                    "options",
                    AttrType::Text,
                    "Comma-separated choices for select.",
                ),
            ],
            &[],
        ),
        el(
            "media",
            "A media file the document draws or plays.",
            IdRule::Required,
            &[
                ID,
                req("kind", AttrType::Enum(&["video", "audio", "image"]), ""),
                req("src", AttrType::Src, ""),
                a("w", AttrType::Int, ""),
                a("h", AttrType::Int, ""),
                a("fps", NON_NEG, ""),
                a("dur", AttrType::Seconds, ""),
                a(
                    "offset",
                    AttrType::Seconds,
                    "Seconds this track starts after the recording.",
                ),
                a("hash", AttrType::Text, "Content hash of the file."),
            ],
            &[],
        ),
        el(
            "track",
            "A data track kept in its own file; the body is never inline.",
            IdRule::Required,
            &[
                ID,
                req(
                    "kind",
                    AttrType::Enum(&["cursor", "words", "peaks", "silences", "ocr", "anim"]),
                    "",
                ),
                req("src", AttrType::Src, ""),
                a("hash", AttrType::Text, ""),
                a("n", AttrType::Int, "Rows in the track."),
                a(
                    "span",
                    AttrType::Text,
                    "First and last second, space separated.",
                ),
                a("ro", AttrType::Bool, "Captured, never edited."),
                a("engine", AttrType::Text, ""),
                a("model", AttrType::Text, ""),
                a("lang", AttrType::Text, ""),
            ],
            &[],
        ),
    ]
}

fn timeline_group() -> Vec<ElementSpec> {
    vec![
        el(
            "timeline",
            "How the recording is cut down.",
            IdRule::None,
            &[
                a("src", AttrType::Ref("media"), ""),
                a("in", AttrType::Seconds, "First kept second."),
                a("out", AttrType::Seconds, "Last kept second."),
            ],
            &["cuts", "split", "clip"],
        ),
        el(
            "cuts",
            "",
            IdRule::None,
            &[a(
                "disabled",
                AttrType::Bool,
                "Cuts authored but not applied.",
            )],
            &["cut", "dismissed"],
        ),
        el(
            "cut",
            "A removed range.",
            IdRule::Required,
            &[
                ID,
                AT,
                DUR,
                a("source", AttrType::Enum(&["manual", "silence"]), ""),
            ],
            &[],
        ),
        el(
            "dismissed",
            "A silence suggestion the user declined.",
            IdRule::Required,
            &[ID, AT, DUR],
            &[],
        ),
        el(
            "split",
            "A marker dividing the kept material into clips.",
            IdRule::Required,
            &[ID, AT],
            &[],
        ),
        el(
            "clip",
            "Per-clip control: speed, entrance and exit, camera layout, audio.",
            IdRule::Required,
            &[
                ID,
                AT,
                a(
                    "speed",
                    AttrType::Number {
                        min: Some(0.05),
                        max: Some(16.0),
                    },
                    "Playback rate.",
                ),
                a("gain", AttrType::Fraction, ""),
                a("muted", AttrType::Bool, ""),
                a(
                    "layout",
                    AttrType::Enum(&["pip", "splitH", "splitV", "screenOnly", "cameraOnly"]),
                    "",
                ),
                a(
                    "fraction",
                    AttrType::Fraction,
                    "Camera share of the frame in a split.",
                ),
                a("side", AttrType::Enum(&["start", "end"]), ""),
            ],
            &["enter", "exit"],
        ),
        el(
            "enter",
            "",
            IdRule::None,
            &[
                req("kind", AttrType::Text, ""),
                a("dur", AttrType::Seconds, ""),
                a("ease", AttrType::Ease, ""),
                a("dir", AttrType::Text, ""),
                a("intensity", NUM, ""),
            ],
            &[],
        ),
        el(
            "exit",
            "",
            IdRule::None,
            &[
                req("kind", AttrType::Text, ""),
                a("dur", AttrType::Seconds, ""),
                a("ease", AttrType::Ease, ""),
                a("dir", AttrType::Text, ""),
                a("intensity", NUM, ""),
            ],
            &[],
        ),
    ]
}

fn background_group() -> Vec<ElementSpec> {
    vec![
        el(
            "background",
            "",
            IdRule::None,
            &[a("blur", NON_NEG, "")],
            &["solid", "gradient", "image"],
        ),
        el(
            "solid",
            "",
            IdRule::None,
            &[req("color", AttrType::Color, "")],
            &[],
        ),
        el(
            "gradient",
            "",
            IdRule::None,
            &[a("angle", NUM, "Degrees, CSS convention.")],
            &["stop"],
        ),
        el(
            "stop",
            "",
            IdRule::None,
            &[
                req("at", AttrType::Percent, "Position along the line."),
                req("color", AttrType::Color, ""),
            ],
            &[],
        ),
        el(
            "image",
            "",
            IdRule::None,
            &[req("src", AttrType::Src, "")],
            &[],
        ),
        el(
            "screen",
            "The recorded screen.",
            IdRule::Required,
            &[ID, a("src", AttrType::Ref("media"), "")],
            &["corner", "shadow", "zooms", "bind", "transform"],
        ),
        el(
            "corner",
            "",
            IdRule::None,
            &[req("pct", AttrType::Percent, "")],
            &[],
        ),
        el(
            "shadow",
            "",
            IdRule::None,
            &[
                a("blur", NON_NEG, ""),
                a("spread", NUM, ""),
                a("dy", NUM, ""),
                a("opacity", AttrType::Fraction, ""),
                a("color", AttrType::Color, ""),
            ],
            &[],
        ),
        el(
            "zooms",
            "",
            IdRule::None,
            &[
                a("disabled", AttrType::Bool, ""),
                a("auto", AttrType::Bool, "Auto-zoom suggestions on."),
                a("applied", AttrType::Bool, "Suggestions were applied."),
            ],
            &["zoom"],
        ),
        el(
            "zoom",
            "",
            IdRule::Required,
            &[
                ID,
                AT,
                DUR,
                a(
                    "scale",
                    AttrType::Number {
                        min: Some(1.0),
                        max: Some(3.0),
                    },
                    "",
                ),
                a("cx", AttrType::Fraction, "Focus x in the video."),
                a("cy", AttrType::Fraction, ""),
                a("rampIn", AttrType::Seconds, ""),
                a("rampOut", AttrType::Seconds, ""),
                a("easeIn", AttrType::Ease, ""),
                a("easeOut", AttrType::Ease, ""),
                a("blur", AttrType::Fraction, "Motion blur strength."),
                a("hidden", AttrType::Bool, ""),
                a("source", AttrType::Enum(&["manual", "auto"]), ""),
            ],
            &[],
        ),
    ]
}

fn camera_group() -> Vec<ElementSpec> {
    vec![
    el(
        "camera",
        "The camera bubble.",
        IdRule::Required,
        &[
            ID,
            a("src", AttrType::Ref("media"), ""),
            a("enabled", AttrType::Bool, ""),
            a("shape", AttrType::Enum(&["circle", "rounded"]), ""),
            a("corner", AttrType::Fraction, "Rounding for the rounded shape."),
            a("mirror", AttrType::Bool, ""),
            a("shadow", AttrType::Fraction, ""),
            a("preset", AttrType::Text, "Animation preset."),
            a("x", AttrType::Fraction, ""),
            a("y", AttrType::Fraction, ""),
            a("w", AttrType::Fraction, ""),
            a("h", AttrType::Fraction, ""),
            a("space", AttrType::Enum(&["video", "canvas"]), ""),
            a("layoutTransition", AttrType::Seconds, "Seconds a layout change takes, output time."),
            a("layoutEase", AttrType::Ease, ""),
        ],
        &["bind", "transform"],
    ),
    el(
        "bind",
        "A property driven by a signal through a map.",
        IdRule::Required,
        &[
            ID,
            req("prop", AttrType::Text, "Space-separated property names."),
            req("src", AttrType::Text, "Signal: time, cursor, cursor.x, cursor.y, cursor.pressed, cursor.idle, zoom.scale, zoom.center, audio.level(track), word.active, layer(id).prop."),
            req("map", AttrType::Enum(&["keys", "linear", "wave", "step", "dodge", "clamp", "follow", "format"]), "No lag map: a frame depends only on its own time."),
            a("strength", AttrType::Fraction, ""),
            a("dur", AttrType::Seconds, ""),
            a("ease", AttrType::Ease, ""),
            a("period", NON_NEG, ""),
            a("from", NUM, ""),
            a("to", NUM, ""),
            a("loop", AttrType::Bool, ""),
            a("in", AttrType::Seconds, "Window start on the element's clock; absent means the whole element."),
            a("out", AttrType::Seconds, "Window end."),
        ],
        &["key"],
    ),
    el("key", "", IdRule::Required, &[ID, AT, a("x", AttrType::Fraction, ""), a("y", AttrType::Fraction, ""), a("w", AttrType::Fraction, ""), a("h", AttrType::Fraction, ""), a("ease", AttrType::Ease, "")], &[]),
    el(
        "cursor",
        "The pointer lane.",
        IdRule::Required,
        &[
            ID,
            a("track", AttrType::Ref("track"), ""),
            a("enabled", AttrType::Bool, ""),
            a("style", AttrType::Text, "dot or an extension sprite id."),
            a("size", NON_NEG, ""),
            a("smoothing", AttrType::Fraction, ""),
            a("snap", AttrType::Bool, "Snap to click positions."),
            a("snapWindow", AttrType::Seconds, ""),
            a("highlight", AttrType::Bool, ""),
            a("highlightColor", AttrType::Color, ""),
            a("highlightOpacity", AttrType::Fraction, ""),
            a("idleHide", AttrType::Bool, ""),
            a("idleTimeout", AttrType::Seconds, ""),
            a("blur", AttrType::Fraction, ""),
            a("bounce", NON_NEG, ""),
            a("bounceDur", AttrType::Seconds, ""),
            a("sway", AttrType::Fraction, ""),
            a("ease", AttrType::Ease, "Motion easing between samples."),
        ],
        &[],
    ),
    ]
}

fn annotation_group() -> Vec<ElementSpec> {
    vec![
        el(
            "annotations",
            "",
            IdRule::None,
            &[a("disabled", AttrType::Bool, "")],
            &["rect", "ellipse", "arrow", "img", "blur", "text"],
        ),
        annotation(
            "rect",
            "",
            &[BOX[0], BOX[1], BOX[2], BOX[3], a("radius", NON_NEG, "")],
        ),
        annotation("ellipse", "", &[BOX[0], BOX[1], BOX[2], BOX[3]]),
        annotation(
            "arrow",
            "",
            &[
                a("x1", NUM, ""),
                a("y1", NUM, ""),
                a("x2", NUM, ""),
                a("y2", NUM, ""),
                a("head", NUM, "Head length as a fraction of the line."),
            ],
        ),
        annotation(
            "img",
            "",
            &[
                BOX[0],
                BOX[1],
                BOX[2],
                BOX[3],
                req("src", AttrType::Src, ""),
                a("radius", NON_NEG, ""),
                a(
                    "fit",
                    AttrType::Enum(&["cover", "contain", "fill"]),
                    "How it fills its box when the aspects disagree. Sequence items only.",
                ),
            ],
        ),
        annotation(
            "blur",
            "",
            &[
                BOX[0],
                BOX[1],
                BOX[2],
                BOX[3],
                a("strength", AttrType::Fraction, ""),
                a("variant", AttrType::Text, ""),
                a("tint", AttrType::Color, ""),
                a("radius", NON_NEG, ""),
            ],
        ),
        ElementSpec {
            text: true,
            ..annotation(
                "text",
                "Text content is the element body.",
                &[
                    BOX[0],
                    BOX[1],
                    BOX[2],
                    BOX[3],
                    a("font", AttrType::Text, ""),
                    a("weight", AttrType::Int, ""),
                    a("size", NON_NEG, ""),
                    a("color", AttrType::Color, ""),
                    a("align", AttrType::Enum(&["left", "center", "right"]), ""),
                    a("lineHeight", NON_NEG, ""),
                ],
            )
        },
        el(
            "transform",
            "The layer's 3D transform, drawn as a homography. Offsets are frame fractions (z in frame heights, away is positive), turns in degrees.",
            IdRule::None,
            &[
                a("x", NUM, ""),
                a("y", NUM, ""),
                a("z", NUM, ""),
                a("rx", NUM, ""),
                a("ry", NUM, ""),
                a("rz", NUM, ""),
                a("scale", NON_NEG, ""),
                a("ax", AttrType::Fraction, "Anchor x within the card."),
                a("ay", AttrType::Fraction, "Anchor y within the card."),
                a("persp", NON_NEG, "Focal length in frame heights; default 2."),
            ],
            &[],
        ),
        el(
            "glow",
            "",
            IdRule::None,
            &[
                a("color", AttrType::Color, ""),
                a("blur", NON_NEG, ""),
                a("opacity", AttrType::Fraction, ""),
            ],
            &[],
        ),
    ]
}

fn caption_group() -> Vec<ElementSpec> {
    vec![
        el(
            "captions",
            "",
            IdRule::Required,
            &[
                ID,
                a("track", AttrType::Ref("track"), ""),
                a("enabled", AttrType::Bool, ""),
                a("font", AttrType::Text, ""),
                a("weight", AttrType::Int, ""),
                a(
                    "size",
                    AttrType::Percent,
                    "Font size, percent of the video height.",
                ),
                a("pos", AttrType::Enum(&["top", "bottom"]), ""),
                a("align", AttrType::Enum(&["left", "center", "right"]), ""),
                a("offset", AttrType::Percent, ""),
                a("color", AttrType::Color, ""),
                a("muted", AttrType::Color, "Colour of words not yet spoken."),
                a("uppercase", AttrType::Bool, ""),
                a("spacing", NUM, "Letter spacing."),
                a("bg", AttrType::Text, "Background style name."),
                a("bgColor", AttrType::Color, ""),
                a("bgOpacity", AttrType::Fraction, ""),
                a("padX", NON_NEG, "Em."),
                a("padY", NON_NEG, "Em."),
                a("radius", NON_NEG, "Em."),
                a("lineHeight", NON_NEG, ""),
                a("outline", NON_NEG, ""),
                a("outlineColor", AttrType::Color, ""),
                a("maxLines", AttrType::Int, ""),
                a("maxChars", AttrType::Int, ""),
            ],
            &["animation"],
        ),
        el(
            "animation",
            "",
            IdRule::None,
            &[
                a("chunk", AttrType::Text, ""),
                a("size", AttrType::Int, ""),
                a("emphasis", AttrType::Text, ""),
                a("emphasisColor", AttrType::Color, ""),
                a("highlight", AttrType::Text, ""),
                a("entrance", AttrType::Text, ""),
                a("entranceDur", AttrType::Seconds, ""),
                a("holdGaps", AttrType::Bool, ""),
            ],
            &[],
        ),
    ]
}

fn audio_and_extras() -> Vec<ElementSpec> {
    vec![
        el(
            "audio",
            "",
            IdRule::None,
            &[
                a("gain", AttrType::Fraction, ""),
                a("muted", AttrType::Bool, ""),
                a("fadeIn", AttrType::Seconds, ""),
                a("fadeOut", AttrType::Seconds, ""),
                a("normalize", AttrType::Bool, ""),
            ],
            &["system", "mic", "voice", "music"],
        ),
        el(
            "system",
            "System audio.",
            IdRule::Required,
            &[
                ID,
                a("src", AttrType::Src, ""),
                a("gain", AttrType::Fraction, ""),
                a("muted", AttrType::Bool, ""),
            ],
            &[],
        ),
        el(
            "mic",
            "Microphone.",
            IdRule::Required,
            &[
                ID,
                a("src", AttrType::Src, ""),
                a("gain", AttrType::Fraction, ""),
                a("muted", AttrType::Bool, ""),
            ],
            &[],
        ),
        el(
            "voice",
            "A detached voice track.",
            IdRule::Required,
            &[
                ID,
                req("src", AttrType::Src, ""),
                a("gain", AttrType::Fraction, ""),
                a("muted", AttrType::Bool, ""),
            ],
            &[],
        ),
        el(
            "music",
            "A clip placed on the OUTPUT timeline.",
            IdRule::Required,
            &[
                ID,
                req("src", AttrType::Src, ""),
                a("axis", AttrType::Enum(&["output"]), ""),
                a("at", AttrType::Seconds, "Output seconds."),
                a("offset", AttrType::Seconds, "Seconds into the file."),
                a("dur", AttrType::Seconds, ""),
                a("gain", AttrType::Fraction, ""),
                a("muted", AttrType::Bool, ""),
                a("fadeIn", AttrType::Seconds, ""),
                a("fadeOut", AttrType::Seconds, ""),
                a("loop", AttrType::Bool, ""),
                a("duck", AttrType::Bool, ""),
                a("provider", AttrType::Text, ""),
                a("track", AttrType::Text, "Provider track id."),
                a("attribution", AttrType::Text, ""),
                a("license", AttrType::Text, ""),
            ],
            &[],
        ),
    ]
}

fn extras_group() -> Vec<ElementSpec> {
    vec![
        el(
            "sequence",
            "An authored composition: items on the OUTPUT clock, with no recording underneath. Their `at` and `dur` are output seconds, unlike the same elements inside <annotations>.",
            IdRule::Optional,
            &[
                a(
                    "transition",
                    AttrType::Enum(&["none", "dissolve"]),
                    "How one item gives way to the next.",
                ),
                a(
                    "transitionDur",
                    NON_NEG,
                    "How long a transition takes, in output seconds.",
                ),
            ],
            &["img", "text"],
        ),
        ElementSpec {
            open_attrs: true,
            ..el(
                "graphic",
                "A component instance; other attributes are its parameters.",
                IdRule::Required,
                &[
                    ID,
                    req("component", AttrType::Text, "name@version"),
                    a("at", AttrType::Seconds, ""),
                    a("dur", AttrType::Seconds, ""),
                ],
                &[],
            )
        },
        el(
            "shader",
            "A component instance painted over the screen card; its uniforms are the component's parameters.",
            IdRule::Required,
            &[
                ID,
                req(
                    "component",
                    AttrType::Text,
                    "name@major.minor of a registered screen component.",
                ),
                a("at", AttrType::Seconds, ""),
                a("dur", AttrType::Seconds, ""),
            ],
            &["uniform"],
        ),
        el(
            "uniform",
            "",
            IdRule::None,
            &[
                req("name", AttrType::Text, ""),
                req("value", AttrType::Text, ""),
            ],
            &[],
        ),
        el(
            "editor",
            "UI state the engine ignores.",
            IdRule::None,
            &[
                a("layout", AttrType::Enum(&["auto", "crop"]), ""),
                a("preset", AttrType::Text, ""),
                a("motionTone", AttrType::Text, ""),
            ],
            &[],
        ),
        el(
            "unknowns",
            "Keys a migration could not place; preserved, never meaningful.",
            IdRule::None,
            &[],
            &["unknown"],
        ),
        el(
            "unknown",
            "",
            IdRule::None,
            &[
                req("key", AttrType::Text, ""),
                req("json", AttrType::Text, ""),
            ],
            &[],
        ),
    ]
}

/// The table, built once. Data, not code: nothing in it depends on runtime state.
#[must_use]
pub fn schema() -> &'static [ElementSpec] {
    static SCHEMA: std::sync::OnceLock<Vec<ElementSpec>> = std::sync::OnceLock::new();
    SCHEMA.get_or_init(build)
}

#[must_use]
pub fn element(kind: &str) -> Option<&'static ElementSpec> {
    schema().iter().find(|e| e.kind == kind)
}

#[must_use]
pub fn attr_type(kind: &str, name: &str) -> Option<AttrType> {
    element(kind)?
        .attrs
        .iter()
        .find(|a| a.name == name)
        .map(|a| a.ty)
}

#[must_use]
pub fn has_text(kind: &str) -> bool {
    element(kind).is_some_and(|e| e.text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_child_kind_names_an_element() {
        for spec in schema() {
            for child in &spec.children {
                assert!(
                    element(child).is_some(),
                    "<{}> allows unknown child <{child}>",
                    spec.kind
                );
            }
        }
    }

    #[test]
    fn every_element_kind_is_unique_and_reachable_from_the_root() {
        let mut kinds: Vec<&str> = schema().iter().map(|e| e.kind).collect();
        kinds.sort_unstable();
        kinds.dedup();
        assert_eq!(kinds.len(), schema().len());
        let mut reachable = vec!["recast"];
        let mut i = 0;
        while i < reachable.len() {
            for child in &element(reachable[i]).unwrap().children {
                if !reachable.contains(child) {
                    reachable.push(child);
                }
            }
            i += 1;
        }
        for spec in schema() {
            assert!(
                reachable.contains(&spec.kind),
                "<{}> is unreachable",
                spec.kind
            );
        }
    }

    #[test]
    fn required_ids_are_declared_as_attributes() {
        for spec in schema().iter().filter(|e| e.id == IdRule::Required) {
            assert!(
                spec.attrs.iter().any(|a| a.name == "id"),
                "<{}> requires an id but does not declare it",
                spec.kind
            );
        }
    }

    #[test]
    fn annotations_share_the_common_attributes_and_text_carries_content() {
        assert_eq!(attr_type("rect", "opacity"), Some(AttrType::Fraction));
        assert_eq!(attr_type("arrow", "x1"), Some(NUM));
        assert!(has_text("text"));
        assert!(!has_text("rect"));
    }
}
