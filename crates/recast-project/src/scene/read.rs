//! Document to `RenderState` (and so `Scene`): the inverse of `write`, reading every attribute back with the same default it was elided against.
//! Planned elements the engine does not draw yet (`vars`, `graphic`, `shader`, extra binds) are left in the document and ignored here.

use recast_scene::bind::{
    Binding, Key, LayerBinding, LayerRef, LayerTransform, Map as BindMap, Signal, Transform3,
};
use recast_scene::v1::easing::Easing;
use recast_scene::v1::nodes::{
    Annotation, AnnotationAnchor, AnnotationGlow, AnnotationKind, AnnotationStroke,
    AnnotationStrokeStyle, AudioClip, AudioClipRole, AudioClipSource, CameraClipLayout,
    CameraKeyframe, CameraLayout, CameraOverlaySettings, CameraPlacement, LayoutSide,
    ShadowSettings, ZoomRegion,
};
use recast_scene::v1::CutRange;
use recast_scene::v1::{RenderState, SceneAnimSpec, SegmentAnim, SegmentSpeed};
use recast_scene::Scene;
use serde_json::{json, Map, Value};

use super::write::default_annotation;
use super::{ids, ms_from_secs, pct_from_frac, Read};
use crate::document::{Document, Node};
use crate::value;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MapError {
    #[error("<{element}{id}> needs {attr}")]
    Missing {
        element: String,
        id: String,
        attr: &'static str,
    },
    #[error("<{element}{id}> {attr}: {message}")]
    Bad {
        element: String,
        id: String,
        attr: &'static str,
        message: String,
    },
    #[error("the document is version {0}; this reader takes version {1}")]
    Version(u32, u32),
}

fn id_suffix(node: &Node) -> String {
    node.id()
        .map_or_else(String::new, |id| format!(" id=\"{id}\""))
}

fn required(node: &Node, attr: &'static str) -> Result<f64, MapError> {
    let raw = node.attr(attr).ok_or_else(|| MapError::Missing {
        element: node.kind.clone(),
        id: id_suffix(node),
        attr,
    })?;
    value::parse_num(raw).map_err(|e| MapError::Bad {
        element: node.kind.clone(),
        id: id_suffix(node),
        attr,
        message: e.to_string(),
    })
}

/// # Errors On a wrong version or an element missing a required numeric attribute.
pub fn to_scene(doc: &Document) -> Result<Scene, MapError> {
    to_render_state(doc).map(|state| recast_scene::migrate::to_scene(&state))
}

/// # Errors As `to_scene`.
pub fn to_render_state(doc: &Document) -> Result<RenderState, MapError> {
    match doc.version() {
        Some(v) if v == crate::FORMAT_VERSION => {}
        other => return Err(MapError::Version(other.unwrap_or(0), crate::FORMAT_VERSION)),
    }
    let root = &doc.root;
    let mut state = RenderState {
        output_aspect: root.attr("aspect").map(str::to_owned),
        padding: root.num_or("pad", 0.0),
        ..RenderState::default()
    };
    if let Some(tl) = root.child("timeline") {
        timeline(tl, &mut state)?;
    }
    if let Some(bg) = root.child("background") {
        background(bg, &mut state);
    }
    if let Some(screen) = root.child("screen") {
        screen_into(screen, &mut state)?;
        push_binds(screen, LayerRef::Screen, &mut state.bindings);
        push_transform(screen, LayerRef::Screen, &mut state.transforms);
    }
    if let Some(camera) = root.child("camera") {
        state.camera_overlay = camera_settings(
            camera,
            std::mem::take(&mut state.camera_overlay.clip_layouts),
        )?;
        push_binds(camera, LayerRef::Camera, &mut state.bindings);
        push_transform(camera, LayerRef::Camera, &mut state.transforms);
    }
    if let Some(cursor) = root.child("cursor") {
        cursor_into(cursor, &mut state);
    }
    if let Some(group) = root.child("annotations") {
        state.annotations_enabled = !group.flag("disabled");
        for node in &group.children {
            let a = annotation(node)?;
            push_binds(
                node,
                LayerRef::Annotation { id: a.id.clone() },
                &mut state.bindings,
            );
            push_transform(
                node,
                LayerRef::Annotation { id: a.id.clone() },
                &mut state.transforms,
            );
            state.annotations.push(a);
        }
    }
    state.caption_style = root.child("captions").map(captions);
    if let Some(audio) = root.child("audio") {
        audio_into(audio, &mut state)?;
    }
    if let Some(editor) = root.child("editor") {
        for (attr, key) in [
            ("layout", "layoutMode"),
            ("preset", "lastAppliedPresetId"),
            ("motionTone", "motionTone"),
        ] {
            if let Some(v) = editor.attr(attr) {
                state.passthrough.insert(key.into(), json!(v));
            }
        }
    }
    if let Some(unknowns) = root.child("unknowns") {
        for u in unknowns.children_of("unknown") {
            if let (Some(key), Some(raw)) = (u.attr("key"), u.attr("json")) {
                let parsed = serde_json::from_str(raw).unwrap_or_else(|_| json!(raw));
                state.passthrough.insert(key.to_owned(), parsed);
            }
        }
    }
    Ok(state)
}

fn timeline(tl: &Node, state: &mut RenderState) -> Result<(), MapError> {
    state.trim_start = tl.num_or("in", 0.0);
    state.trim_end = required(tl, "out")?;
    if let Some(cuts) = tl.child("cuts") {
        if cuts.flag("disabled") {
            state.passthrough.insert("cutsEnabled".into(), json!(false));
        }
        for cut in cuts.children_of("cut") {
            let at = required(cut, "at")?;
            let mut extra = Map::new();
            if let Some(id) = cut.id() {
                extra.insert("id".into(), json!(id));
            }
            extra.insert("source".into(), json!(cut.text_or("source", "manual")));
            state.cuts.push(CutRange {
                start: at,
                end: at + required(cut, "dur")?,
                extra,
            });
        }
        let dismissed: Vec<Value> = cuts
            .children_of("dismissed")
            .map(|d| Ok(json!({ "start": required(d, "at")?, "end": required(d, "at")? + required(d, "dur")? })))
            .collect::<Result<_, MapError>>()?;
        if !dismissed.is_empty() {
            state
                .passthrough
                .insert("dismissedSilences".into(), Value::Array(dismissed));
        }
    }
    for split in tl.children_of("split") {
        state.split_points.push(required(split, "at")?);
    }
    for clip in tl.children_of("clip") {
        clip_into(clip, state)?;
    }
    Ok(())
}

fn clip_into(clip: &Node, state: &mut RenderState) -> Result<(), MapError> {
    let at = required(clip, "at")?;
    let speed = clip.num_or("speed", 1.0);
    if (speed - 1.0).abs() > 1e-9 {
        state.segment_speeds.push(SegmentSpeed { start: at, speed });
    }
    let anim_in = clip.child("enter").map(anim_spec).transpose()?;
    let anim_out = clip.child("exit").map(anim_spec).transpose()?;
    if anim_in.is_some() || anim_out.is_some() {
        state.scene_animations.push(SegmentAnim {
            start: at,
            anim_in,
            anim_out,
        });
    }
    if let Some(layout) = clip.attr("layout") {
        let side = match clip.attr("side") {
            Some("end") => LayoutSide::End,
            _ => LayoutSide::Start,
        };
        let fraction = clip.num_or("fraction", 0.5);
        let layout = match layout {
            "splitH" => CameraLayout::SplitH { fraction, side },
            "splitV" => CameraLayout::SplitV { fraction, side },
            "screenOnly" => CameraLayout::ScreenOnly,
            "cameraOnly" => CameraLayout::CameraOnly,
            _ => CameraLayout::Pip,
        };
        state
            .camera_overlay
            .clip_layouts
            .push(CameraClipLayout { start: at, layout });
    }
    Ok(())
}

fn anim_spec(node: &Node) -> Result<SceneAnimSpec, MapError> {
    let kind = node.attr("kind").ok_or_else(|| MapError::Missing {
        element: node.kind.clone(),
        id: String::new(),
        attr: "kind",
    })?;
    Ok(SceneAnimSpec {
        kind: kind.to_owned(),
        duration_ms: ms_from_secs(node.num_or("dur", 0.0)),
        easing: node.ease_or("ease", Default::default()),
        dir: node.attr("dir").map(str::to_owned),
        intensity: node
            .attr("intensity")
            .and_then(|v| value::parse_num(v).ok()),
    })
}

fn background(bg: &Node, state: &mut RenderState) {
    state.background_blur = bg.num_or("blur", 0.0);
    if let Some(solid) = bg.child("solid") {
        state.background_type = "color".into();
        state.background_value = solid.text_or("color", "#111111");
    } else if let Some(g) = bg.child("gradient") {
        state.background_type = "gradient".into();
        let gradient = recast_color::Gradient {
            angle: g.num_or("angle", 180.0),
            stops: g
                .children_of("stop")
                .filter_map(|s| {
                    Some(recast_color::GradientStop {
                        color: recast_color::parse_hex(s.attr("color")?)?,
                        pos: s.num_or("at", 0.0),
                    })
                })
                .collect(),
        };
        state.background_value = recast_color::serialize_gradient(&gradient);
    } else if let Some(image) = bg.child("image") {
        let src = image.text_or("src", "");
        state.background_type = if src.starts_with("ext:") || src.starts_with("asset:") {
            "wallpaper"
        } else {
            "image"
        }
        .into();
        state.background_value = src_to_v1(&src);
    }
}

/// The document's `file:///` link is v1's bare path; everything else is spelled the same.
fn src_to_v1(src: &str) -> String {
    src.strip_prefix("file:///")
        .map_or_else(|| src.to_owned(), str::to_owned)
}

fn screen_into(screen: &Node, state: &mut RenderState) -> Result<(), MapError> {
    if let Some(corner) = screen.child("corner") {
        state.border_radius = corner.num_or("pct", 0.0);
    }
    if let Some(shadow) = screen.child("shadow") {
        let d = ShadowSettings::default();
        state.shadow = ShadowSettings {
            enabled: true,
            blur: shadow.num_or("blur", d.blur),
            spread: shadow.num_or("spread", d.spread),
            offset_y: shadow.num_or("dy", d.offset_y),
            opacity: pct_from_frac(shadow.num_or("opacity", d.opacity / 100.0)),
            color: shadow.text_or("color", &d.color),
        };
    }
    if let Some(zooms) = screen.child("zooms") {
        state.focus_enabled = !zooms.flag("disabled");
        if zooms.flag("auto") {
            state
                .passthrough
                .insert("autoZoomEnabled".into(), json!(true));
        }
        if zooms.flag("applied") {
            state
                .passthrough
                .insert("autoZoomApplied".into(), json!(true));
        }
        for zoom in zooms.children_of("zoom") {
            state.zoom_regions.push(zoom_region(zoom)?);
        }
    }
    Ok(())
}

fn zoom_region(z: &Node) -> Result<ZoomRegion, MapError> {
    let start = required(z, "at")?;
    let mut extra = Map::new();
    if let Some(id) = z.id() {
        extra.insert("id".into(), json!(id));
    }
    extra.insert("source".into(), json!(z.text_or("source", "manual")));
    Ok(ZoomRegion {
        start,
        end: start + required(z, "dur")?,
        scale: z.num_or("scale", 1.8),
        ease_in: z.ease_or("easeIn", Default::default()),
        ease_out: z.ease_or("easeOut", Default::default()),
        ramp_in: z.num_or("rampIn", 0.0),
        ramp_out: z.num_or("rampOut", 0.0),
        center_x: z.num_or("cx", 0.5),
        center_y: z.num_or("cy", 0.5),
        hidden: z.flag("hidden"),
        motion_blur: z.num_or("blur", 0.0),
        extra,
    })
}

/// Generic `<bind>` children become state bindings; the camera's own keys, follow and dodge are its settings, not bindings.
fn push_binds(node: &Node, layer: LayerRef, out: &mut Vec<LayerBinding>) {
    for bind in node.children_of("bind") {
        let map = bind.attr("map").unwrap_or("");
        if node.kind == "camera" && crate::validate::STACKABLE.contains(&map) {
            continue;
        }
        if let Some(binding) = binding_of(bind) {
            out.push(LayerBinding {
                layer: layer.clone(),
                binding,
            });
        }
    }
}

/// A `<transform>` child becomes the layer's transform; absent attributes keep the identity's values.
fn push_transform(node: &Node, layer: LayerRef, out: &mut Vec<LayerTransform>) {
    let Some(t) = node.child("transform") else {
        return;
    };
    let d = Transform3::IDENTITY;
    let transform = Transform3 {
        x: t.num_or("x", d.x),
        y: t.num_or("y", d.y),
        z: t.num_or("z", d.z),
        rx: t.num_or("rx", d.rx),
        ry: t.num_or("ry", d.ry),
        rz: t.num_or("rz", d.rz),
        scale: t.num_or("scale", d.scale),
        anchor_x: t.num_or("ax", d.anchor_x),
        anchor_y: t.num_or("ay", d.anchor_y),
        perspective: t.num_or("persp", d.perspective),
    };
    if !transform.is_identity() {
        out.push(LayerTransform { layer, transform });
    }
}

/// A `<bind>` element as the engine's typed binding; `None` when the source or map does not parse (the validator reports it).
pub fn binding_of(bind: &Node) -> Option<Binding> {
    let signal = Signal::parse(bind.attr("src")?).ok()?;
    let num = |name: &str| bind.attr(name).and_then(|v| v.parse::<f64>().ok());
    let map = match bind.attr("map")? {
        "keys" => BindMap::Keys(
            bind.children_of("key")
                .filter_map(|k| {
                    Some(Key {
                        at: k.attr("at")?.parse().ok()?,
                        value: k.attr("value")?.parse().ok()?,
                        ease: k.ease_or("ease", bind.ease_or("ease", Easing::default())),
                    })
                })
                .collect(),
        ),
        "linear" => BindMap::Linear {
            from: num("from").unwrap_or(0.0),
            to: num("to").unwrap_or(1.0),
            period: num("period"),
            looped: bind.flag("loop"),
        },
        "wave" => BindMap::Wave {
            from: num("from").unwrap_or(0.0),
            to: num("to").unwrap_or(1.0),
            period: num("period").unwrap_or(1.0),
        },
        "step" => BindMap::Step {
            from: num("from").unwrap_or(0.0),
            to: num("to").unwrap_or(1.0),
        },
        "clamp" => BindMap::Clamp {
            from: num("from").unwrap_or(0.0),
            to: num("to").unwrap_or(1.0),
        },
        _ => return None,
    };
    Some(Binding {
        prop: bind.attr("prop")?.to_owned(),
        signal,
        map,
        window: num("in").zip(num("out")),
    })
}

fn camera_settings(
    camera: &Node,
    clip_layouts: Vec<CameraClipLayout>,
) -> Result<CameraOverlaySettings, MapError> {
    let d = CameraOverlaySettings::default();
    let follow = camera
        .children_of("bind")
        .find(|b| b.attr("map") == Some("follow"));
    let dodge = camera
        .children_of("bind")
        .find(|b| b.attr("map") == Some("dodge"));
    let keys = camera
        .children_of("bind")
        .find(|b| b.attr("map") == Some("keys"));
    let keyframes = keys
        .map(|bind| {
            bind.children_of("key")
                .map(|k| {
                    Ok(CameraKeyframe {
                        at_sec: required(k, "at")?,
                        placement: placement(k, &d.default_placement),
                    })
                })
                .collect::<Result<Vec<_>, MapError>>()
        })
        .transpose()?
        .unwrap_or_default();
    Ok(CameraOverlaySettings {
        enabled: camera.flag("enabled"),
        mirror: camera.flag_or("mirror", d.mirror),
        shape: camera.text_or("shape", &d.shape),
        corner_radius: camera.num_or("corner", d.corner_radius),
        animation_preset: camera.text_or("preset", &d.animation_preset),
        zoom_follow: follow.is_some(),
        zoom_follow_strength: follow.map_or(d.zoom_follow_strength, |f| {
            f.num_or("strength", d.zoom_follow_strength)
        }),
        zoom_follow_duration: follow.map_or(d.zoom_follow_duration, |f| {
            f.num_or("dur", d.zoom_follow_duration)
        }),
        zoom_follow_easing: follow.map_or(d.zoom_follow_easing, |f| {
            f.ease_or("ease", d.zoom_follow_easing)
        }),
        default_placement: placement(camera, &d.default_placement),
        motion_segments: Vec::new(),
        keyframes,
        clip_layouts,
        layout_transition: camera.num_or("layoutTransition", d.layout_transition),
        layout_transition_easing: camera.ease_or("layoutEase", d.layout_transition_easing),
        cursor_dodge: dodge.is_some(),
        cursor_dodge_strength: dodge.map_or(d.cursor_dodge_strength, |b| {
            b.num_or("strength", d.cursor_dodge_strength)
        }),
        keyframe_easing: keys.map_or(d.keyframe_easing, |k| k.ease_or("ease", d.keyframe_easing)),
        shadow: camera.num_or("shadow", d.shadow),
    })
}

fn placement(node: &Node, d: &CameraPlacement) -> CameraPlacement {
    CameraPlacement {
        x: node.num_or("x", d.x),
        y: node.num_or("y", d.y),
        width: node.num_or("w", d.width),
        height: node.num_or("h", d.height),
    }
}

fn cursor_into(cursor: &Node, state: &mut RenderState) {
    let d = RenderState::default();
    state.cursor_enabled = cursor.flag("enabled");
    if let Some(style) = cursor.attr("style") {
        state.passthrough.insert("cursorStyle".into(), json!(style));
    }
    state.cursor_size = cursor.num_or("size", d.cursor_size);
    state.cursor_smoothing = pct_from_frac(cursor.num_or("smoothing", d.cursor_smoothing / 100.0));
    state.cursor_snap_to_clicks = cursor.flag_or("snap", d.cursor_snap_to_clicks);
    state.cursor_snap_window_ms =
        ms_from_secs(cursor.num_or("snapWindow", d.cursor_snap_window_ms / 1000.0));
    state.cursor_highlight_clicks = cursor.flag_or("highlight", d.cursor_highlight_clicks);
    state.cursor_highlight_color = cursor.text_or("highlightColor", &d.cursor_highlight_color);
    state.cursor_highlight_opacity =
        pct_from_frac(cursor.num_or("highlightOpacity", d.cursor_highlight_opacity / 100.0));
    state.cursor_hide_when_idle = cursor.flag("idleHide");
    state.cursor_idle_timeout = cursor.num_or("idleTimeout", d.cursor_idle_timeout);
    state.cursor_motion_blur = cursor.num_or("blur", d.cursor_motion_blur);
    state.cursor_click_bounce = cursor.num_or("bounce", d.cursor_click_bounce);
    state.cursor_bounce_speed_ms =
        ms_from_secs(cursor.num_or("bounceDur", d.cursor_bounce_speed_ms / 1000.0));
    state.cursor_sway = cursor.num_or("sway", d.cursor_sway);
    state.cursor_motion_easing = cursor
        .attr("ease")
        .and_then(|e| value::parse_ease(e).ok())
        .map(super::easing_from);
}

fn annotation(node: &Node) -> Result<Annotation, MapError> {
    let d = default_annotation();
    let start = required(node, "at")?;
    let fill = match node.attr("fill") {
        None => d.fill.clone(),
        Some("none") => "transparent".to_owned(),
        Some(color) => color.to_owned(),
    };
    Ok(Annotation {
        id: node.id().unwrap_or("").to_owned(),
        start,
        end: start + required(node, "dur")?,
        ramp_in: node.num_or("rampIn", d.ramp_in),
        ramp_out: node.num_or("rampOut", d.ramp_out),
        ease_in: node.ease_or("easeIn", d.ease_in),
        ease_out: node.ease_or("easeOut", d.ease_out),
        stroke: AnnotationStroke {
            width: node.num_or("stroke", d.stroke.width),
            color: node.text_or("strokeColor", &d.stroke.color),
            style: match node.attr("strokeStyle") {
                Some("dashed") => AnnotationStrokeStyle::Dashed,
                Some("dotted") => AnnotationStrokeStyle::Dotted,
                _ => AnnotationStrokeStyle::Solid,
            },
        },
        fill,
        kind: annotation_kind(node)?,
        name: node.attr("name").map(str::to_owned),
        z_index: node.int_or("z", i64::from(d.z_index)) as i32,
        locked: node.flag("locked"),
        hidden: node.flag("hidden"),
        opacity: node.num_or("opacity", d.opacity),
        glow: node.child("glow").map(|g| AnnotationGlow {
            color: g.text_or("color", "#ffffff"),
            blur: g.num_or("blur", 0.0),
            opacity: g.num_or("opacity", 1.0),
        }),
        anchor: if node.attr("space") == Some("frame") {
            AnnotationAnchor::Frame
        } else {
            AnnotationAnchor::Video
        },
    })
}

fn annotation_kind(node: &Node) -> Result<AnnotationKind, MapError> {
    let n = |name| node.num_or(name, 0.0);
    Ok(match node.kind.as_str() {
        "rect" => AnnotationKind::Rect {
            x: n("x"),
            y: n("y"),
            w: n("w"),
            h: n("h"),
            radius: n("radius"),
        },
        "ellipse" => AnnotationKind::Ellipse {
            x: n("x"),
            y: n("y"),
            w: n("w"),
            h: n("h"),
        },
        "arrow" => AnnotationKind::Arrow {
            x1: n("x1"),
            y1: n("y1"),
            x2: n("x2"),
            y2: n("y2"),
            head_size: node.num_or("head", 0.15),
        },
        "img" => AnnotationKind::Image {
            x: n("x"),
            y: n("y"),
            w: n("w"),
            h: n("h"),
            path: src_to_v1(&node.text_or("src", "")),
            opacity: node.num_or("opacity", 1.0),
            radius: n("radius"),
        },
        "blur" => AnnotationKind::Blur {
            x: n("x"),
            y: n("y"),
            w: n("w"),
            h: n("h"),
            strength: node.num_or("strength", 0.5),
            variant: node.text_or("variant", "glass"),
            tint_color: node.text_or("tint", "#000000"),
            radius: n("radius"),
        },
        "text" => AnnotationKind::Text {
            x: n("x"),
            y: n("y"),
            w: n("w"),
            h: n("h"),
            content: node.text.clone().unwrap_or_default(),
            font_family: node.text_or("font", ""),
            font_size: node.num_or("size", 0.0),
            font_weight: node.num_or("weight", 400.0),
            color: node.text_or("color", "#ffffff"),
            align: node.text_or("align", "left"),
            line_height: node.num_or("lineHeight", 1.2),
        },
        other => {
            return Err(MapError::Bad {
                element: other.to_owned(),
                id: id_suffix(node),
                attr: "kind",
                message: "not an annotation kind".into(),
            })
        }
    })
}

fn captions(node: &Node) -> recast_captions::CaptionStyle {
    let d = recast_captions::CaptionStyle::default();
    recast_captions::CaptionStyle {
        enabled: node.flag("enabled"),
        font_family: node.text_or("font", &d.font_family),
        font_weight: node.int_or("weight", i64::from(d.font_weight)) as u32,
        font_size_pct: node.num_or("size", d.font_size_pct),
        position: node.text_or("pos", &d.position),
        align: node.text_or("align", &d.align),
        offset_pct: node.num_or("offset", d.offset_pct),
        color: node.text_or("color", &d.color),
        muted_color: node.text_or("muted", &d.muted_color),
        uppercase: node.flag("uppercase"),
        letter_spacing: node.num_or("spacing", d.letter_spacing),
        background: node.text_or("bg", &d.background),
        background_color: node.text_or("bgColor", &d.background_color),
        background_opacity: pct_from_frac(node.num_or("bgOpacity", d.background_opacity / 100.0)),
        box_padding_x_em: node.num_or("padX", d.box_padding_x_em),
        box_padding_y_em: node.num_or("padY", d.box_padding_y_em),
        box_radius_em: node.num_or("radius", d.box_radius_em),
        line_height: node.num_or("lineHeight", d.line_height),
        outline_width: node.num_or("outline", d.outline_width),
        outline_color: node.text_or("outlineColor", &d.outline_color),
        max_lines: node.int_or("maxLines", i64::from(d.max_lines)) as u32,
        max_chars_per_line: node.int_or("maxChars", i64::from(d.max_chars_per_line)) as u32,
        animation: node.child("animation").map(caption_animation),
    }
}

fn caption_animation(node: &Node) -> recast_captions::CaptionAnimation {
    let d = recast_captions::CaptionAnimation::default();
    recast_captions::CaptionAnimation {
        chunk: node.text_or("chunk", &d.chunk),
        chunk_size: node.int_or("size", i64::from(d.chunk_size)) as u32,
        emphasis: node.text_or("emphasis", &d.emphasis),
        emphasis_color: node.text_or("emphasisColor", &d.emphasis_color),
        highlight: node.attr("highlight").map(str::to_owned),
        entrance: node.text_or("entrance", &d.entrance),
        entrance_ms: ms_from_secs(node.num_or("entranceDur", d.entrance_ms / 1000.0)),
        hold_gaps: node.flag_or("holdGaps", d.hold_gaps),
    }
}

fn audio_into(audio: &Node, state: &mut RenderState) -> Result<(), MapError> {
    let s = &mut state.audio_settings;
    let d = recast_scene::v1::nodes::AudioSettings::default();
    s.volume = pct_from_frac(audio.num_or("gain", d.volume / 100.0));
    s.muted = audio.flag("muted");
    s.fade_in = audio.num_or("fadeIn", d.fade_in);
    s.fade_out = audio.num_or("fadeOut", d.fade_out);
    s.normalize_loudness = audio.flag("normalize");
    if let Some(sys) = audio.child("system") {
        s.system_volume = pct_from_frac(sys.num_or("gain", d.system_volume / 100.0));
        s.system_muted = sys.flag("muted");
    }
    if let Some(mic) = audio.child("mic") {
        s.mic_volume = pct_from_frac(mic.num_or("gain", d.mic_volume / 100.0));
        s.mic_muted = mic.flag("muted");
    }
    for music in audio.children_of("music") {
        state.music_clips.push(music_clip(music)?);
    }
    Ok(())
}

fn music_clip(node: &Node) -> Result<AudioClip, MapError> {
    let src = node.attr("src").ok_or_else(|| MapError::Missing {
        element: node.kind.clone(),
        id: id_suffix(node),
        attr: "src",
    })?;
    let source = match node.attr("provider") {
        Some(provider) => AudioClipSource::Provider {
            provider_id: provider.to_owned(),
            track_id: node.text_or("track", ""),
            asset_path: src_to_v1(src),
            attribution: node.attr("attribution").map(str::to_owned),
            license: node.attr("license").map(str::to_owned),
        },
        None => AudioClipSource::Local {
            path: src_to_v1(src),
        },
    };
    Ok(AudioClip {
        id: node.id().unwrap_or("").to_owned(),
        source,
        role: AudioClipRole::Music,
        start_output_sec: node.num_or("at", 0.0),
        offset_sec: node.num_or("offset", 0.0),
        duration_sec: node.num_or("dur", 0.0),
        gain: pct_from_frac(node.num_or("gain", 1.0)),
        muted: node.flag("muted"),
        fade_in: node.num_or("fadeIn", 0.0),
        fade_out: node.num_or("fadeOut", 0.0),
        looping: node.flag("loop"),
        ducking: node.flag("duck"),
    })
}

#[allow(dead_code)]
fn _ids_used() -> &'static str {
    ids::SCREEN
}
