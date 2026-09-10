//! `RenderState` (and so `Scene`) to a document. Every attribute is elided against the v1 default, so an untouched project serialises to its skeleton.
//! Ids already carried by v1 rows (`extra.id`, `Annotation.id`) are kept; rows without one get a minted id in document order.

use recast_scene::bind::{
    Binding, LayerBinding, LayerRef, LayerTransform, Map, Signal, Transform3,
};
use recast_scene::v1::easing::Easing;
use recast_scene::v1::nodes::{
    Annotation, AnnotationKind, AudioClip, AudioClipSource, AudioSettings, CameraLayout,
    CameraOverlaySettings, ShadowSettings, ZoomRegion,
};
use recast_scene::v1::{RenderState, SegmentAnim};
use recast_scene::Scene;
use serde_json::Value;

use super::{
    canonical_color, ease_from, frac_from_pct, ids, near, secs_from_ms, Elide, Read,
    PLACED_PASSTHROUGH,
};
use crate::document::{Document, Node};
use crate::ids::{Id, IdGen};
use crate::value::{self, fmt_secs};

/// A media file the host knows about, described so the document can reference it.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MediaFile {
    pub src: String,
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub duration: f64,
    /// Seconds this file starts after the recording.
    pub offset: f64,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct TrackFile {
    pub src: String,
    pub rows: usize,
    pub span: Option<(f64, f64)>,
    pub engine: Option<String>,
    pub model: Option<String>,
    pub lang: Option<String>,
}

/// Which files exist for this project. The state never names files; the host does.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MediaRefs {
    pub recording: Option<MediaFile>,
    pub camera: Option<MediaFile>,
    pub mic: Option<String>,
    pub system: Option<String>,
    pub cursor: Option<TrackFile>,
    pub words: Option<TrackFile>,
}

impl MediaRefs {
    /// Lifts the refs back out of a document, so a rewrite from state can run where no filesystem is (the webview replica).
    #[must_use]
    pub fn from_document(doc: &Document) -> Self {
        let media = |id: &str| {
            doc.find(id).map(|n| MediaFile {
                src: n.text_or("src", ""),
                width: n.num_or("w", 0.0) as u32,
                height: n.num_or("h", 0.0) as u32,
                fps: n.num_or("fps", 0.0),
                duration: n.num_or("dur", 0.0),
                offset: n.num_or("offset", 0.0),
            })
        };
        let track = |id: &str| {
            doc.find(id).map(|n| TrackFile {
                src: n.text_or("src", ""),
                rows: n.num_or("n", 0.0) as usize,
                span: n.attr("span").and_then(|s| {
                    let mut parts = s.split_whitespace().map(|p| p.parse::<f64>().ok());
                    Some((parts.next()??, parts.next()??))
                }),
                engine: n.attr("engine").map(str::to_owned),
                model: n.attr("model").map(str::to_owned),
                lang: n.attr("lang").map(str::to_owned),
            })
        };
        let src_of = |id: &str| doc.find(id).and_then(|n| n.attr("src")).map(str::to_owned);
        Self {
            recording: media(ids::RECORDING),
            camera: media(ids::CAMERA_MEDIA),
            mic: src_of(ids::MIC),
            system: src_of(ids::SYSTEM),
            cursor: track(ids::CURSOR_TRACK),
            words: track(ids::WORDS_TRACK),
        }
    }
}

impl TrackFile {
    /// The words track header for a transcript in the editor's JSON shape (`segments[].words[].{start,end}`).
    #[must_use]
    pub fn words(src: &str, transcript: &Value) -> Self {
        let words: Vec<(f64, f64)> = transcript
            .get("segments")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .flat_map(|s| {
                s.get("words")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
            })
            .filter_map(|w| Some((w.get("start")?.as_f64()?, w.get("end")?.as_f64()?)))
            .collect();
        let text = |key: &str| {
            transcript
                .get(key)
                .and_then(Value::as_str)
                .map(str::to_owned)
        };
        Self {
            src: src.to_owned(),
            rows: words.len(),
            span: words.first().zip(words.last()).map(|(f, l)| (f.0, l.1)),
            engine: text("engine"),
            model: text("modelId"),
            lang: text("language"),
        }
    }
}

/// The bindings to write: the state's when it carries any, else the base document's generic ones (the editor's whole-state
/// save never names them, so a save must not drop what an agent declared). The camera's keys, follow and dodge come from settings.
fn generic_binds(state: &RenderState, base: Option<&Document>) -> Vec<LayerBinding> {
    if !state.bindings.is_empty() {
        return state.bindings.clone();
    }
    let Some(base) = base else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut take = |node: &Node, layer: LayerRef| {
        for bind in node.children_of("bind") {
            let map = bind.attr("map").unwrap_or("");
            if node.kind == "camera" && crate::validate::STACKABLE.contains(&map) {
                continue;
            }
            if let Some(binding) = super::read::binding_of(bind) {
                out.push(LayerBinding {
                    layer: layer.clone(),
                    binding,
                });
            }
        }
    };
    if let Some(n) = base.root.child("screen") {
        take(n, LayerRef::Screen);
    }
    if let Some(n) = base.root.child("camera") {
        take(n, LayerRef::Camera);
    }
    if let Some(group) = base.root.child("annotations") {
        for n in &group.children {
            if let Some(id) = n.id() {
                take(n, LayerRef::Annotation { id: id.to_owned() });
            }
        }
    }
    out
}

/// Writes each non-identity transform as a `<transform>` child of the element its layer names.
fn attach_transforms(root: &mut Node, transforms: &[LayerTransform]) {
    for lt in transforms {
        if lt.transform.is_identity() {
            continue;
        }
        let target = match &lt.layer {
            LayerRef::Screen => root.child_mut("screen"),
            LayerRef::Camera => root.child_mut("camera"),
            LayerRef::Annotation { id } => root
                .child_mut("annotations")
                .and_then(|g| g.children.iter_mut().find(|n| n.id() == Some(id))),
        };
        let Some(node) = target else {
            continue;
        };
        let (t, d) = (&lt.transform, Transform3::IDENTITY);
        let mut child = Node::new("transform");
        for (name, v, default) in [
            ("x", t.x, d.x),
            ("y", t.y, d.y),
            ("z", t.z, d.z),
            ("rx", t.rx, d.rx),
            ("ry", t.ry, d.ry),
            ("rz", t.rz, d.rz),
            ("scale", t.scale, d.scale),
            ("ax", t.anchor_x, d.anchor_x),
            ("ay", t.anchor_y, d.anchor_y),
            ("persp", t.perspective, d.perspective),
        ] {
            child.num(name, v, default);
        }
        node.children.push(child);
    }
}

/// Writes each material under the element its layer names. Only `<screen>` and `<camera>` take one; an annotation has no card to light.
fn attach_materials(root: &mut Node, materials: &[recast_scene::material::LayerMaterial]) {
    use recast_scene::material::Material;
    for lm in materials {
        if lm.material.is_none() {
            continue;
        }
        let target = match &lm.layer {
            LayerRef::Screen => root.child_mut("screen"),
            LayerRef::Camera => root.child_mut("camera"),
            LayerRef::Annotation { .. } => None,
        };
        let Some(node) = target else {
            continue;
        };
        let (m, d) = (&lm.material, Material::NONE);
        let mut child = Node::new("material");
        for (name, v, default) in [
            ("contact", m.contact, d.contact),
            ("rim", m.rim, d.rim),
            ("rimWidth", m.rim_width, d.rim_width),
        ] {
            child.num(name, v, default);
        }
        node.children.push(child);
    }
}

/// Writes each binding under the element its layer names; a binding for an annotation the state no longer has is dropped with it.
fn attach_binds(root: &mut Node, binds: &[LayerBinding], ids: &mut IdGen) {
    for lb in binds {
        let target = match &lb.layer {
            LayerRef::Screen => root.child_mut("screen"),
            LayerRef::Camera => root.child_mut("camera"),
            LayerRef::Annotation { id } => root
                .child_mut("annotations")
                .and_then(|g| g.children.iter_mut().find(|n| n.id() == Some(id))),
        };
        if let Some(node) = target {
            node.children.push(bind_node(&lb.binding, ids));
        }
    }
}

fn bind_node(b: &Binding, ids: &mut IdGen) -> Node {
    let mut node = Node::new("bind")
        .with_id(&ids.next_id().to_string())
        .with("prop", b.prop.clone())
        .with("src", signal_src(&b.signal));
    match &b.map {
        Map::Keys(keys) => {
            node.set("map", "keys");
            for k in keys {
                let mut key = Node::new("key")
                    .with_id(&ids.next_id().to_string())
                    .with("at", fmt_secs(k.at))
                    .with("value", value::fmt_num(k.value));
                key.ease("ease", k.ease, Easing::default());
                node.children.push(key);
            }
        }
        Map::Linear {
            from,
            to,
            period,
            looped,
        } => {
            node.set("map", "linear");
            node.set("from", value::fmt_num(*from));
            node.set("to", value::fmt_num(*to));
            if let Some(p) = period {
                node.set("period", value::fmt_num(*p));
            }
            node.set_flag("loop", *looped);
        }
        Map::Wave { from, to, period } => {
            node.set("map", "wave");
            node.set("from", value::fmt_num(*from));
            node.set("to", value::fmt_num(*to));
            node.set("period", value::fmt_num(*period));
        }
        Map::Step { from, to } => {
            node.set("map", "step");
            node.set("from", value::fmt_num(*from));
            node.set("to", value::fmt_num(*to));
        }
        Map::Clamp { from, to } => {
            node.set("map", "clamp");
            node.set("from", value::fmt_num(*from));
            node.set("to", value::fmt_num(*to));
        }
    }
    if let Some((i, o)) = b.window {
        node.set("in", fmt_secs(i));
        node.set("out", fmt_secs(o));
    }
    node
}

fn signal_src(s: &Signal) -> String {
    match s {
        Signal::Time => "time".into(),
        Signal::Cursor => "cursor".into(),
        Signal::CursorX => "cursor.x".into(),
        Signal::CursorY => "cursor.y".into(),
        Signal::CursorPressed => "cursor.pressed".into(),
        Signal::CursorIdle => "cursor.idle".into(),
        Signal::ZoomScale => "zoom.scale".into(),
        Signal::ZoomCenter => "zoom.center".into(),
        Signal::ZoomCenterX => "zoom.center.x".into(),
        Signal::ZoomCenterY => "zoom.center.y".into(),
        Signal::AudioLevel(track) => format!("audio.level({track})"),
        Signal::WordActive => "word.active".into(),
        Signal::LayerProp { layer, prop } => format!("layer({layer}).{prop}"),
    }
}

/// Builds the document. `base` supplies what the state cannot carry (`vars`, `graphic`, `shader`, unknown elements), so an edit round trip keeps them.
pub fn from_scene(
    scene: &Scene,
    media: &MediaRefs,
    base: Option<&Document>,
    ids: &mut IdGen,
) -> Document {
    from_render_state(
        &recast_scene::migrate::to_render_state(scene),
        media,
        base,
        ids,
    )
}

pub fn from_render_state(
    state: &RenderState,
    media: &MediaRefs,
    base: Option<&Document>,
    ids: &mut IdGen,
) -> Document {
    reserve_existing(state, base, ids);
    let mut doc = Document::empty();
    let root = &mut doc.root;
    root.set("timebase", "source-seconds");
    if let Some(aspect) = state.output_aspect.as_deref().filter(|a| *a != "source") {
        root.set("aspect", aspect);
    }
    root.num("pad", state.padding, 0.0);

    if let Some(vars) =
        vars_node(state).or_else(|| base.and_then(|b| b.root.child("vars")).cloned())
    {
        root.children.push(vars);
    }
    root.children.extend(media_nodes(media));
    root.children.push(timeline(state, ids));
    root.children.push(background(state));
    root.children.push(screen(state, media, ids));
    root.children.push(camera(state, media, ids));
    root.children.push(cursor(state, media));
    root.children.push(annotations(state));
    let generic = generic_binds(state, base);
    attach_binds(root, &generic, ids);
    attach_transforms(root, &state.transforms);
    attach_materials(root, &state.materials);
    if let Some(style) = &state.caption_style {
        root.children.push(captions(style, media));
    }
    root.children.push(audio(state, media));
    if let Some(composition) = &state.composition {
        root.children.push(sequence_node(composition));
    }
    for graphic in &state.graphics {
        root.children.push(graphic_node(graphic));
    }
    if let Some(base) = base {
        root.children.extend(
            base.root
                .children
                .iter()
                .filter(|c| crate::schema::element(&c.kind).is_none())
                .cloned(),
        );
    }
    root.children.push(editor(state));
    if let Some(unknowns) = unknowns(state) {
        root.children.push(unknowns);
    }
    doc
}

fn reserve_existing(state: &RenderState, base: Option<&Document>, ids: &mut IdGen) {
    let mut taken: Vec<String> = state.annotations.iter().map(|a| a.id.clone()).collect();
    taken.extend(state.zoom_regions.iter().filter_map(|z| extra_id(&z.extra)));
    taken.extend(state.cuts.iter().filter_map(|c| extra_id(&c.extra)));
    taken.extend(state.music_clips.iter().map(|c| c.id.clone()));
    if let Some(base) = base {
        taken.extend(base.ids().into_iter().map(|i| i.to_string()));
        base.root.walk(&mut |n| {
            if let (true, Some(id), Some(at)) = (
                RECYCLED_KINDS.contains(&n.kind.as_str()),
                n.id(),
                n.attr("at"),
            ) {
                ids.remember(&row_key(&n.kind, at), id);
            }
        });
    }
    ids.reserve(taken.iter().map(String::as_str));
}

/// Rows the v1 state carries no id for; a base document's ids are recycled by kind and start so a rewrite is stable.
const RECYCLED_KINDS: &[&str] = &["split", "clip", "dismissed", "key"];

fn row_key(kind: &str, at: &str) -> String {
    format!(
        "{kind}@{}",
        value::parse_num(at).map_or_else(|_| at.to_owned(), fmt_secs)
    )
}

fn extra_id(extra: &serde_json::Map<String, Value>) -> Option<String> {
    extra
        .get("id")
        .and_then(Value::as_str)
        .filter(|id| Id::parse(id).is_ok())
        .map(str::to_owned)
}

fn id_or_mint(existing: Option<String>, ids: &mut IdGen) -> String {
    existing.unwrap_or_else(|| ids.next_id().to_string())
}

fn media_nodes(media: &MediaRefs) -> Vec<Node> {
    let mut out = Vec::new();
    if let Some(rec) = &media.recording {
        out.push(media_node(ids::RECORDING, rec));
    }
    if let Some(cam) = &media.camera {
        out.push(media_node(ids::CAMERA_MEDIA, cam));
    }
    if let Some(cur) = &media.cursor {
        let mut node = track_node(ids::CURSOR_TRACK, "cursor", cur);
        node.set_flag("ro", true);
        out.push(node);
    }
    if let Some(words) = &media.words {
        out.push(track_node(ids::WORDS_TRACK, "words", words));
    }
    out
}

fn media_node(id: &str, file: &MediaFile) -> Node {
    let mut node = Node::new("media")
        .with_id(id)
        .with("kind", "video")
        .with("src", file.src.clone());
    node.num("w", f64::from(file.width), 0.0);
    node.num("h", f64::from(file.height), 0.0);
    node.num("fps", file.fps, 0.0);
    node.secs("dur", file.duration, 0.0);
    node.secs("offset", file.offset, 0.0);
    node
}

fn track_node(id: &str, kind: &str, file: &TrackFile) -> Node {
    let mut node = Node::new("track")
        .with_id(id)
        .with("kind", kind)
        .with("src", file.src.clone());
    node.num("n", file.rows as f64, 0.0);
    if let Some((s, e)) = file.span {
        node.set("span", format!("{} {}", fmt_secs(s), fmt_secs(e)));
    }
    for (name, v) in [
        ("engine", &file.engine),
        ("model", &file.model),
        ("lang", &file.lang),
    ] {
        if let Some(v) = v {
            node.set(name, v.clone());
        }
    }
    node
}

fn timeline(state: &RenderState, ids: &mut IdGen) -> Node {
    let mut tl = Node::new("timeline").with("src", ids::RECORDING);
    tl.secs("in", state.trim_start, 0.0);
    tl.set("out", fmt_secs(state.trim_end));
    tl.children.push(cuts(state, ids));
    for at in &state.split_points {
        let mut split = Node::new("split")
            .with_id(&ids.mint_for(&row_key("split", &fmt_secs(*at))).to_string());
        split.set("at", fmt_secs(*at));
        tl.children.push(split);
    }
    tl.children.extend(clips(state, ids));
    tl
}

fn cuts(state: &RenderState, ids: &mut IdGen) -> Node {
    let mut cuts = Node::new("cuts");
    let enabled = state
        .passthrough
        .get("cutsEnabled")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    cuts.set_flag("disabled", !enabled);
    for cut in &state.cuts {
        let mut node = Node::new("cut").with_id(&id_or_mint(extra_id(&cut.extra), ids));
        node.set("at", fmt_secs(cut.start));
        node.set("dur", fmt_secs(cut.end - cut.start));
        if let Some(source) = cut
            .extra
            .get("source")
            .and_then(Value::as_str)
            .filter(|s| *s != "manual")
        {
            node.set("source", source);
        }
        cuts.children.push(node);
    }
    if let Some(list) = state
        .passthrough
        .get("dismissedSilences")
        .and_then(Value::as_array)
    {
        for item in list {
            if let (Some(start), Some(end)) = (
                item.get("start").and_then(Value::as_f64),
                item.get("end").and_then(Value::as_f64),
            ) {
                let mut node = Node::new("dismissed").with_id(
                    &ids.mint_for(&row_key("dismissed", &fmt_secs(start)))
                        .to_string(),
                );
                node.set("at", fmt_secs(start));
                node.set("dur", fmt_secs(end - start));
                cuts.children.push(node);
            }
        }
    }
    cuts
}

/// Speeds, entrance/exit animations and camera layouts all key on a clip's start; one `<clip>` per distinct start.
fn clips(state: &RenderState, ids: &mut IdGen) -> Vec<Node> {
    let mut starts: Vec<f64> = state.segment_speeds.iter().map(|s| s.start).collect();
    starts.extend(state.scene_animations.iter().map(|a| a.start));
    starts.extend(state.camera_overlay.clip_layouts.iter().map(|l| l.start));
    starts.sort_by(f64::total_cmp);
    starts.dedup_by(|a, b| near(*a, *b));
    starts
        .into_iter()
        .map(|at| {
            let mut clip = Node::new("clip")
                .with_id(&ids.mint_for(&row_key("clip", &fmt_secs(at))).to_string());
            clip.set("at", fmt_secs(at));
            if let Some(speed) = state.segment_speeds.iter().find(|s| near(s.start, at)) {
                clip.num("speed", speed.speed, 1.0);
            }
            if let Some(anim) = state.scene_animations.iter().find(|a| near(a.start, at)) {
                clip.children.extend(anim_nodes(anim));
            }
            if let Some(layout) = state
                .camera_overlay
                .clip_layouts
                .iter()
                .find(|l| near(l.start, at))
            {
                write_layout(&mut clip, layout.layout);
            }
            clip
        })
        .collect()
}

fn anim_nodes(anim: &SegmentAnim) -> Vec<Node> {
    [("enter", &anim.anim_in), ("exit", &anim.anim_out)]
        .into_iter()
        .filter_map(|(kind, spec)| spec.as_ref().map(|s| (kind, s)))
        .map(|(kind, spec)| {
            let mut node = Node::new(kind).with("kind", spec.kind.clone());
            node.secs("dur", secs_from_ms(spec.duration_ms), 0.0);
            node.ease("ease", spec.easing, Default::default());
            if let Some(dir) = &spec.dir {
                node.set("dir", dir.clone());
            }
            if let Some(intensity) = spec.intensity {
                node.set("intensity", value::fmt_num(intensity));
            }
            node
        })
        .collect()
}

fn write_layout(clip: &mut Node, layout: CameraLayout) {
    let (name, fraction, side) = match layout {
        CameraLayout::Pip => ("pip", None, None),
        CameraLayout::SplitH { fraction, side } => ("splitH", Some(fraction), Some(side)),
        CameraLayout::SplitV { fraction, side } => ("splitV", Some(fraction), Some(side)),
        CameraLayout::ScreenOnly => ("screenOnly", None, None),
        CameraLayout::CameraOnly => ("cameraOnly", None, None),
    };
    clip.set("layout", name);
    if let Some(fraction) = fraction {
        clip.set("fraction", value::fmt_num(fraction));
    }
    if let Some(side) = side {
        clip.set(
            "side",
            match side {
                recast_scene::v1::nodes::LayoutSide::Start => "start",
                recast_scene::v1::nodes::LayoutSide::End => "end",
            },
        );
    }
}

fn background(state: &RenderState) -> Node {
    let mut bg = Node::new("background");
    bg.num("blur", state.background_blur, 0.0);
    let value = state.background_value.as_str();
    let child = match state.background_type.as_str() {
        "color" => Node::new("solid").with("color", canonical_color(value)),
        "gradient" => gradient(value),
        _ => Node::new("image").with("src", image_src(value)),
    };
    bg.children.push(child);
    bg
}

fn gradient(css: &str) -> Node {
    let g = recast_color::parse_gradient(css);
    let mut node = Node::new("gradient");
    node.num("angle", g.angle, 180.0);
    for stop in &g.stops {
        node.children.push(
            Node::new("stop")
                .with("at", value::fmt_num(stop.pos))
                .with("color", stop.color.to_hex()),
        );
    }
    node
}

/// v1 spelled five schemes on one string; the document takes the `src` grammar. Absolute paths become `file:///` links until an import copies them.
fn image_src(value: &str) -> String {
    if let Some(id) = value.strip_prefix("ext:") {
        return format!("ext:{id}");
    }
    if let Some(id) = value.strip_prefix("asset:") {
        return format!("asset:{id}");
    }
    if value::parse_src(value).is_ok() {
        return value.to_owned();
    }
    format!(
        "file:///{}",
        value.trim_start_matches('/').replace('\\', "/")
    )
}

fn screen(state: &RenderState, media: &MediaRefs, ids: &mut IdGen) -> Node {
    let mut screen = Node::new("screen").with_id(ids::SCREEN);
    if media.recording.is_some() {
        screen.set("src", ids::RECORDING);
    }
    if state.border_radius != 0.0 {
        screen
            .children
            .push(Node::new("corner").with("pct", value::fmt_num(state.border_radius)));
    }
    if state.shadow.enabled {
        screen.children.push(shadow(&state.shadow));
    }
    screen.children.push(zooms(state, ids));
    screen
}

fn shadow(s: &ShadowSettings) -> Node {
    let d = ShadowSettings::default();
    let mut node = Node::new("shadow");
    node.num("blur", s.blur, d.blur);
    node.num("spread", s.spread, d.spread);
    node.num("dy", s.offset_y, d.offset_y);
    node.frac(
        "opacity",
        frac_from_pct(s.opacity),
        frac_from_pct(d.opacity),
    );
    node.color("color", &s.color, &d.color);
    node
}

fn zooms(state: &RenderState, ids: &mut IdGen) -> Node {
    let mut zooms = Node::new("zooms");
    zooms.set_flag("disabled", !state.focus_enabled);
    zooms.set_flag(
        "auto",
        state
            .passthrough
            .get("autoZoomEnabled")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    );
    zooms.set_flag(
        "applied",
        state
            .passthrough
            .get("autoZoomApplied")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    );
    for z in &state.zoom_regions {
        zooms.children.push(zoom(z, ids));
    }
    zooms
}

fn zoom(z: &ZoomRegion, ids: &mut IdGen) -> Node {
    let mut node = Node::new("zoom").with_id(&id_or_mint(extra_id(&z.extra), ids));
    node.set("at", fmt_secs(z.start));
    node.set("dur", fmt_secs(z.end - z.start));
    node.set("scale", value::fmt_num(z.scale));
    node.set("cx", value::fmt_num(z.center_x));
    node.set("cy", value::fmt_num(z.center_y));
    node.secs("rampIn", z.ramp_in, 0.0);
    node.secs("rampOut", z.ramp_out, 0.0);
    node.ease("easeIn", z.ease_in, Default::default());
    node.ease("easeOut", z.ease_out, Default::default());
    node.frac("blur", z.motion_blur, 0.0);
    node.set_flag("hidden", z.hidden);
    if let Some(source) = z
        .extra
        .get("source")
        .and_then(Value::as_str)
        .filter(|s| *s != "manual")
    {
        node.set("source", source);
    }
    node
}

fn camera(state: &RenderState, media: &MediaRefs, ids: &mut IdGen) -> Node {
    let c = &state.camera_overlay;
    let d = CameraOverlaySettings::default();
    let mut node = Node::new("camera").with_id(ids::CAMERA);
    if media.camera.is_some() {
        node.set("src", ids::CAMERA_MEDIA);
    }
    node.set_flag("enabled", c.enabled);
    node.text("shape", &c.shape, &d.shape);
    node.frac("corner", c.corner_radius, d.corner_radius);
    if c.mirror != d.mirror {
        node.set("mirror", c.mirror.to_string());
    }
    node.frac("shadow", c.shadow, d.shadow);
    node.text("preset", &c.animation_preset, &d.animation_preset);
    node.frac("x", c.default_placement.x, d.default_placement.x);
    node.frac("y", c.default_placement.y, d.default_placement.y);
    node.frac("w", c.default_placement.width, d.default_placement.width);
    node.frac("h", c.default_placement.height, d.default_placement.height);
    node.set("space", "canvas");
    node.secs("layoutTransition", c.layout_transition, d.layout_transition);
    node.ease(
        "layoutEase",
        c.layout_transition_easing,
        d.layout_transition_easing,
    );
    node.children.extend(camera_binds(c, &d, ids));
    node
}

fn camera_binds(
    c: &CameraOverlaySettings,
    d: &CameraOverlaySettings,
    ids: &mut IdGen,
) -> Vec<Node> {
    let mut binds = Vec::new();
    if c.zoom_follow {
        let mut follow = Node::new("bind")
            .with_id(ids::FOLLOW)
            .with("prop", "x y")
            .with("src", "zoom.center")
            .with("map", "follow");
        follow.frac("strength", c.zoom_follow_strength, d.zoom_follow_strength);
        follow.secs("dur", c.zoom_follow_duration, d.zoom_follow_duration);
        follow.ease("ease", c.zoom_follow_easing, d.zoom_follow_easing);
        binds.push(follow);
    }
    if c.cursor_dodge {
        let mut dodge = Node::new("bind")
            .with_id(ids::DODGE)
            .with("prop", "x y")
            .with("src", "cursor")
            .with("map", "dodge");
        dodge.frac("strength", c.cursor_dodge_strength, d.cursor_dodge_strength);
        binds.push(dodge);
    }
    let keys = keyframes(c, ids);
    if !keys.is_empty() {
        let mut bind = Node::new("bind")
            .with_id(ids::KEYS)
            .with("prop", "x y w h")
            .with("src", "time")
            .with("map", "keys");
        bind.ease("ease", c.keyframe_easing, d.keyframe_easing);
        bind.children = keys;
        binds.push(bind);
    }
    binds
}

/// Keyframes as stored, plus motion segments folded into a key pair each: the editor only ever read them folded.
fn keyframes(c: &CameraOverlaySettings, ids: &mut IdGen) -> Vec<Node> {
    let mut keys: Vec<(f64, f64, f64, f64, f64)> = c
        .keyframes
        .iter()
        .map(|k| {
            (
                k.at_sec,
                k.placement.x,
                k.placement.y,
                k.placement.width,
                k.placement.height,
            )
        })
        .collect();
    for m in &c.motion_segments {
        keys.push((m.start, m.from_x, m.from_y, m.from_width, m.from_height));
        keys.push((m.end, m.to_x, m.to_y, m.to_width, m.to_height));
    }
    keys.sort_by(|a, b| a.0.total_cmp(&b.0));
    keys.into_iter()
        .map(|(at, x, y, w, h)| {
            Node::new("key")
                .with_id(&ids.mint_for(&row_key("key", &fmt_secs(at))).to_string())
                .with("at", fmt_secs(at))
                .with("x", value::fmt_num(x))
                .with("y", value::fmt_num(y))
                .with("w", value::fmt_num(w))
                .with("h", value::fmt_num(h))
        })
        .collect()
}

fn cursor(state: &RenderState, media: &MediaRefs) -> Node {
    let d = RenderState::default();
    let mut node = Node::new("cursor").with_id(ids::CURSOR);
    if media.cursor.is_some() {
        node.set("track", ids::CURSOR_TRACK);
    }
    node.set_flag("enabled", state.cursor_enabled);
    if let Some(style) = state
        .passthrough
        .get("cursorStyle")
        .and_then(Value::as_str)
        .filter(|s| *s != "dot")
    {
        node.set("style", style);
    }
    node.num("size", state.cursor_size, d.cursor_size);
    node.frac(
        "smoothing",
        frac_from_pct(state.cursor_smoothing),
        frac_from_pct(d.cursor_smoothing),
    );
    if state.cursor_snap_to_clicks != d.cursor_snap_to_clicks {
        node.set("snap", state.cursor_snap_to_clicks.to_string());
    }
    node.secs(
        "snapWindow",
        secs_from_ms(state.cursor_snap_window_ms),
        secs_from_ms(d.cursor_snap_window_ms),
    );
    if state.cursor_highlight_clicks != d.cursor_highlight_clicks {
        node.set("highlight", state.cursor_highlight_clicks.to_string());
    }
    node.color(
        "highlightColor",
        &state.cursor_highlight_color,
        &d.cursor_highlight_color,
    );
    node.frac(
        "highlightOpacity",
        frac_from_pct(state.cursor_highlight_opacity),
        frac_from_pct(d.cursor_highlight_opacity),
    );
    node.set_flag("idleHide", state.cursor_hide_when_idle);
    node.secs(
        "idleTimeout",
        state.cursor_idle_timeout,
        d.cursor_idle_timeout,
    );
    node.frac("blur", state.cursor_motion_blur, d.cursor_motion_blur);
    node.num("bounce", state.cursor_click_bounce, d.cursor_click_bounce);
    node.secs(
        "bounceDur",
        secs_from_ms(state.cursor_bounce_speed_ms),
        secs_from_ms(d.cursor_bounce_speed_ms),
    );
    node.frac("sway", state.cursor_sway, d.cursor_sway);
    if let Some(ease) = state.cursor_motion_easing {
        node.set("ease", value::fmt_ease(ease_from(ease)));
    }
    node
}

fn annotations(state: &RenderState) -> Node {
    let mut group = Node::new("annotations");
    group.set_flag("disabled", !state.annotations_enabled);
    for a in &state.annotations {
        group.children.push(annotation(a));
    }
    group
}

pub(super) fn default_annotation() -> Annotation {
    // Every field but these has a serde default; deserialising the minimum is how those defaults are read without duplicating them.
    serde_json::from_value(serde_json::json!({
        "id": "_", "start": 0.0, "end": 1.0,
        "kind": { "kind": "rect", "x": 0.0, "y": 0.0, "w": 0.0, "h": 0.0, "radius": 0.0 }
    }))
    .unwrap_or_else(|_| unreachable!("the minimal annotation always deserialises"))
}

fn annotation(a: &Annotation) -> Node {
    let d = default_annotation();
    let (kind, mut node) = annotation_kind(&a.kind);
    let _ = kind;
    node.set("id", a.id.clone());
    node.set("at", fmt_secs(a.start));
    node.set("dur", fmt_secs(a.end - a.start));
    node.secs("rampIn", a.ramp_in, d.ramp_in);
    node.secs("rampOut", a.ramp_out, d.ramp_out);
    node.ease("easeIn", a.ease_in, d.ease_in);
    node.ease("easeOut", a.ease_out, d.ease_out);
    node.num("stroke", a.stroke.width, d.stroke.width);
    node.color("strokeColor", &a.stroke.color, &d.stroke.color);
    let style = format!("{:?}", a.stroke.style).to_ascii_lowercase();
    node.text(
        "strokeStyle",
        &style,
        &format!("{:?}", d.stroke.style).to_ascii_lowercase(),
    );
    if a.fill != d.fill {
        node.set(
            "fill",
            if a.fill == "transparent" {
                "none".to_owned()
            } else {
                canonical_color(&a.fill)
            },
        );
    }
    node.frac("opacity", a.opacity, d.opacity);
    node.num("z", f64::from(a.z_index), f64::from(d.z_index));
    node.set_flag("locked", a.locked);
    node.set_flag("hidden", a.hidden);
    if let Some(name) = &a.name {
        node.set("name", name.clone());
    }
    if matches!(a.anchor, recast_scene::v1::nodes::AnnotationAnchor::Frame) {
        node.set("space", "frame");
    }
    if let Some(glow) = &a.glow {
        let mut g = Node::new("glow").with("color", canonical_color(&glow.color));
        g.num("blur", glow.blur, 0.0);
        g.frac("opacity", glow.opacity, 1.0);
        node.children.push(g);
    }
    node
}

fn annotation_kind(kind: &AnnotationKind) -> (&'static str, Node) {
    let boxed = |n: Node, x: f64, y: f64, w: f64, h: f64| {
        n.with("x", value::fmt_num(x))
            .with("y", value::fmt_num(y))
            .with("w", value::fmt_num(w))
            .with("h", value::fmt_num(h))
    };
    match kind {
        AnnotationKind::Rect { x, y, w, h, radius } => {
            let mut n = boxed(Node::new("rect"), *x, *y, *w, *h);
            n.num("radius", *radius, 0.0);
            ("rect", n)
        }
        AnnotationKind::Ellipse { x, y, w, h } => {
            ("ellipse", boxed(Node::new("ellipse"), *x, *y, *w, *h))
        }
        AnnotationKind::Arrow {
            x1,
            y1,
            x2,
            y2,
            head_size,
        } => {
            let mut n = Node::new("arrow")
                .with("x1", value::fmt_num(*x1))
                .with("y1", value::fmt_num(*y1))
                .with("x2", value::fmt_num(*x2))
                .with("y2", value::fmt_num(*y2));
            n.num("head", *head_size, 0.15);
            ("arrow", n)
        }
        AnnotationKind::Image {
            x,
            y,
            w,
            h,
            path,
            opacity,
            radius,
        } => {
            let mut n = boxed(Node::new("img"), *x, *y, *w, *h).with("src", image_src(path));
            n.frac("opacity", *opacity, 1.0);
            n.num("radius", *radius, 0.0);
            ("img", n)
        }
        AnnotationKind::Blur {
            x,
            y,
            w,
            h,
            strength,
            variant,
            tint_color,
            radius,
        } => {
            let mut n = boxed(Node::new("blur"), *x, *y, *w, *h);
            n.frac("strength", *strength, 0.5);
            n.text("variant", variant, "glass");
            n.color("tint", tint_color, "#000000");
            n.num("radius", *radius, 0.0);
            ("blur", n)
        }
        AnnotationKind::Text {
            x,
            y,
            w,
            h,
            content,
            font_family,
            font_size,
            font_weight,
            color,
            align,
            line_height,
        } => {
            let mut n = boxed(Node::new("text"), *x, *y, *w, *h);
            n.text = Some(content.clone());
            n.set("font", font_family.clone());
            n.num("weight", *font_weight, 400.0);
            n.set("size", value::fmt_num(*font_size));
            n.set("color", canonical_color(color));
            n.text("align", align, "left");
            n.num("lineHeight", *line_height, 1.2);
            ("text", n)
        }
        AnnotationKind::Unsupported => ("rect", Node::new("rect")),
    }
}

fn captions(style: &recast_captions::CaptionStyle, media: &MediaRefs) -> Node {
    let d = recast_captions::CaptionStyle::default();
    let mut node = Node::new("captions").with_id(ids::CAPTIONS);
    if media.words.is_some() {
        node.set("track", ids::WORDS_TRACK);
    }
    node.set_flag("enabled", style.enabled);
    node.text("font", &style.font_family, &d.font_family);
    node.num(
        "weight",
        f64::from(style.font_weight),
        f64::from(d.font_weight),
    );
    node.num("size", style.font_size_pct, d.font_size_pct);
    node.text("pos", &style.position, &d.position);
    node.text("align", &style.align, &d.align);
    node.num("offset", style.offset_pct, d.offset_pct);
    node.color("color", &style.color, &d.color);
    node.color("muted", &style.muted_color, &d.muted_color);
    node.set_flag("uppercase", style.uppercase);
    node.num("spacing", style.letter_spacing, d.letter_spacing);
    node.text("bg", &style.background, &d.background);
    node.color("bgColor", &style.background_color, &d.background_color);
    node.frac(
        "bgOpacity",
        frac_from_pct(style.background_opacity),
        frac_from_pct(d.background_opacity),
    );
    node.num("padX", style.box_padding_x_em, d.box_padding_x_em);
    node.num("padY", style.box_padding_y_em, d.box_padding_y_em);
    node.num("radius", style.box_radius_em, d.box_radius_em);
    node.num("lineHeight", style.line_height, d.line_height);
    node.num("outline", style.outline_width, d.outline_width);
    node.color("outlineColor", &style.outline_color, &d.outline_color);
    node.num(
        "maxLines",
        f64::from(style.max_lines),
        f64::from(d.max_lines),
    );
    node.num(
        "maxChars",
        f64::from(style.max_chars_per_line),
        f64::from(d.max_chars_per_line),
    );
    if let Some(anim) = &style.animation {
        node.children.push(caption_animation(anim));
    }
    node
}

fn caption_animation(anim: &recast_captions::CaptionAnimation) -> Node {
    let d = recast_captions::CaptionAnimation::default();
    let mut node = Node::new("animation");
    node.text("chunk", &anim.chunk, &d.chunk);
    node.num("size", f64::from(anim.chunk_size), f64::from(d.chunk_size));
    node.text("emphasis", &anim.emphasis, &d.emphasis);
    node.color("emphasisColor", &anim.emphasis_color, &d.emphasis_color);
    if let Some(h) = &anim.highlight {
        node.set("highlight", h.clone());
    }
    node.text("entrance", &anim.entrance, &d.entrance);
    node.secs(
        "entranceDur",
        secs_from_ms(anim.entrance_ms),
        secs_from_ms(d.entrance_ms),
    );
    if anim.hold_gaps != d.hold_gaps {
        node.set("holdGaps", anim.hold_gaps.to_string());
    }
    node
}

fn audio(state: &RenderState, media: &MediaRefs) -> Node {
    let s = &state.audio_settings;
    let d = AudioSettings::default();
    let mut node = Node::new("audio");
    node.frac("gain", frac_from_pct(s.volume), frac_from_pct(d.volume));
    node.set_flag("muted", s.muted);
    node.secs("fadeIn", s.fade_in, d.fade_in);
    node.secs("fadeOut", s.fade_out, d.fade_out);
    node.set_flag("normalize", s.normalize_loudness);
    if let Some(src) = &media.system {
        let mut sys = Node::new("system")
            .with_id(ids::SYSTEM)
            .with("src", src.clone());
        sys.frac(
            "gain",
            frac_from_pct(s.system_volume),
            frac_from_pct(d.system_volume),
        );
        sys.set_flag("muted", s.system_muted);
        node.children.push(sys);
    }
    if let Some(src) = &media.mic {
        let mut mic = Node::new("mic").with_id(ids::MIC).with("src", src.clone());
        mic.frac(
            "gain",
            frac_from_pct(s.mic_volume),
            frac_from_pct(d.mic_volume),
        );
        mic.set_flag("muted", s.mic_muted);
        node.children.push(mic);
    }
    for clip in &state.music_clips {
        node.children.push(music(clip));
    }
    node
}

fn music(clip: &AudioClip) -> Node {
    let mut node = Node::new("music").with_id(&clip.id).with("axis", "output");
    match &clip.source {
        AudioClipSource::Local { path } => node.set("src", image_src(path)),
        AudioClipSource::Provider {
            provider_id,
            track_id,
            asset_path,
            attribution,
            license,
        } => {
            node.set("src", image_src(asset_path));
            node.set("provider", provider_id.clone());
            node.set("track", track_id.clone());
            if let Some(a) = attribution {
                node.set("attribution", a.clone());
            }
            if let Some(l) = license {
                node.set("license", l.clone());
            }
        }
    }
    node.secs("at", clip.start_output_sec, 0.0);
    node.secs("offset", clip.offset_sec, 0.0);
    node.secs("dur", clip.duration_sec, 0.0);
    node.frac("gain", frac_from_pct(clip.gain), 1.0);
    node.set_flag("muted", clip.muted);
    node.secs("fadeIn", clip.fade_in, 0.0);
    node.secs("fadeOut", clip.fade_out, 0.0);
    node.set_flag("loop", clip.looping);
    node.set_flag("duck", clip.ducking);
    node
}

/// The composition, written with the same elements the annotations use; the parent is what puts them on the output clock.
fn sequence_node(composition: &recast_scene::composition::Composition) -> Node {
    use recast_scene::composition::{ItemContent, Transition};
    let mut node = Node::new("sequence");
    if composition.transition != Transition::None {
        node.set("transition", composition.transition.as_str());
    }
    if composition.transition_dur != 0.0 {
        node.set("transitionDur", value::fmt_secs(composition.transition_dur));
    }
    for item in &composition.items {
        let mut child = Node::new(match &item.content {
            ItemContent::Image { .. } => "img",
            ItemContent::Text { .. } => "text",
        });
        child.set("id", item.id.clone());
        child.set("at", value::fmt_secs(item.at));
        child.set("dur", value::fmt_secs(item.dur));
        for (name, number, default) in [
            ("x", item.area.x, 0.0),
            ("y", item.area.y, 0.0),
            ("w", item.area.w, 1.0),
            ("h", item.area.h, 1.0),
            ("opacity", item.opacity, 1.0),
        ] {
            if number != default {
                child.set(name, value::fmt_num(number));
            }
        }
        match &item.content {
            ItemContent::Image { src, fit, radius } => {
                child.set("src", src.clone());
                if *fit != recast_scene::composition::Fit::default() {
                    child.set("fit", fit.as_str());
                }
                if *radius != 0.0 {
                    child.set("radius", value::fmt_num(*radius));
                }
            }
            ItemContent::Text {
                content,
                font,
                size,
                color,
                align,
                weight,
                line_height,
            } => {
                child.text = Some(content.clone());
                if !font.is_empty() {
                    child.set("font", font.clone());
                }
                child.set("size", value::fmt_num(*size));
                child.set("color", color.clone());
                if *align != recast_scene::composition::TextAlign::default() {
                    child.set("align", align.as_str());
                }
                if *weight != 400.0 {
                    child.set("weight", value::fmt_num(*weight));
                }
                if *line_height != super::read::DEFAULT_ITEM_LINE_HEIGHT {
                    child.set("lineHeight", value::fmt_num(*line_height));
                }
            }
        }
        node.children.push(child);
    }
    node
}

/// `<vars>` from the state, or nothing when it declares none, in which case the base document's block still stands.
fn vars_node(state: &RenderState) -> Option<Node> {
    if state.vars.is_empty() {
        return None;
    }
    let mut node = Node::new("vars");
    for var in &state.vars {
        let mut child = Node::new("var");
        child.set("name", var.name.clone());
        child.set("type", var.ty.as_str());
        child.set("value", var.value.clone());
        if let Some(path) = &var.path {
            child.set("path", path.clone());
        }
        for (name, number) in [("min", var.min), ("max", var.max), ("step", var.step)] {
            if let Some(number) = number {
                child.set(name, value::fmt_num(number));
            }
        }
        if !var.options.is_empty() {
            child.set("options", var.options.join(","));
        }
        node.children.push(child);
    }
    Some(node)
}

/// The element it came from, so a `<shader>` does not become a `<graphic>` on the way back.
fn graphic_node(spec: &recast_scene::component::GraphicSpec) -> Node {
    use recast_scene::component::Surface;
    let shader = spec.fallback_surface == Surface::Screen;
    let mut node = Node::new(if shader { "shader" } else { "graphic" });
    node.set("id", spec.id.clone());
    node.set("component", spec.component.clone());
    if spec.start != 0.0 {
        node.set("at", value::fmt_secs(spec.start));
    }
    if spec.duration != 0.0 {
        node.set("dur", value::fmt_secs(spec.duration));
    }
    for (name, param) in &spec.params {
        match shader {
            true => node.children.push(
                Node::new("uniform")
                    .with("name", name.clone())
                    .with("value", param.clone()),
            ),
            false => node.set(name, param.clone()),
        }
    }
    node
}

fn editor(state: &RenderState) -> Node {
    let mut node = Node::new("editor");
    for (attr, key) in [
        ("layout", "layoutMode"),
        ("preset", "lastAppliedPresetId"),
        ("motionTone", "motionTone"),
    ] {
        if let Some(v) = state.passthrough.get(key).and_then(Value::as_str) {
            node.set(attr, v);
        }
    }
    node
}

fn unknowns(state: &RenderState) -> Option<Node> {
    let leftover: Vec<(&String, &Value)> = state
        .passthrough
        .iter()
        .filter(|(k, _)| !PLACED_PASSTHROUGH.contains(&k.as_str()))
        .collect();
    if leftover.is_empty() {
        return None;
    }
    let mut node = Node::new("unknowns");
    for (key, value) in leftover {
        node.children.push(
            Node::new("unknown")
                .with("key", key.clone())
                .with("json", value.to_string()),
        );
    }
    Some(node)
}
