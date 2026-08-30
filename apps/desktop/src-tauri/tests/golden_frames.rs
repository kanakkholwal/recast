//! Pins what the CURRENT export compositor renders, so the wgpu compositor has an
//! exact target rather than 98 "MUST match the preview" comments. Set
//! `UPDATE_GOLDENS=1` to rewrite the fixtures after a deliberate change.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use recast_lib::render::graph::{
    compute_canvas_geometry, RenderGraph, RenderState, SourceVideoMetadata,
};
use recast_testkit::{compare, media, SourceSpec};

const CANVAS_W: u32 = 320;
const CANVAS_H: u32 = 180;
const SAMPLE_FRAMES: [u64; 3] = [0, 15, 44];
const MAX_CHANNEL_DELTA: u8 = 6;
const MAX_MEAN_DELTA: f64 = 0.6;

fn goldens_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/goldens")
}

fn updating() -> bool {
    std::env::var("UPDATE_GOLDENS").as_deref() == Ok("1")
}

struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("recast-golden-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch dir");
        Self(dir)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn base_state() -> RenderState {
    RenderState {
        trim_start: 0.0,
        trim_end: 1.5,
        cursor_enabled: false,
        ..Default::default()
    }
}

fn fixtures() -> Vec<(&'static str, RenderState)> {
    let mut padded = base_state();
    padded.padding = 8.0;
    padded.background_type = "color".into();
    padded.background_value = "#1e293b".into();

    // Border radius, drop shadow and gradients are pre-rasterised PNGs, so `build_export_plan_with` alone can't exercise them.
    let mut zoomed = padded.clone();
    zoomed.zoom_regions = vec![serde_json::from_value(serde_json::json!({
        "start": 0.2,
        "end": 1.4,
        "scale": 2.0,
        "rampIn": 0.3,
        "rampOut": 0.3,
        "centerX": 0.3,
        "centerY": 0.7
    }))
    .expect("zoom fixture")];

    let mut portrait = padded.clone();
    portrait.output_aspect = Some("9:16".into());

    vec![
        ("plain", base_state()),
        ("padded-color", padded),
        ("zoomed", zoomed),
        ("portrait-9x16", portrait),
    ]
}

/// Renders the fixture through the real export filter graph and returns the
/// sampled frames as raw RGBA.
fn render_fixture(
    ffmpeg: &Path,
    source: &Path,
    state: &RenderState,
) -> Result<Vec<Vec<u8>>, String> {
    let geom = compute_canvas_geometry(
        CANVAS_W,
        CANVAS_H,
        state.padding,
        state.output_aspect.as_deref(),
    );
    let plan = RenderGraph::from_state(state)
        .build_export_plan_with(
            SourceVideoMetadata {
                width: CANVAS_W,
                height: CANVAS_H,
                fps: 30.0,
            },
            Path::new("."),
            1,
            None,
            None,
            None,
            None,
            geom,
            None,
        )
        .map_err(|e| e.to_string())?;

    let mut args: Vec<String> = Vec::new();
    let push = |values: &[&str], args: &mut Vec<String>| {
        args.extend(values.iter().map(|v| (*v).to_string()));
    };
    push(&["-hide_banner", "-loglevel", "error", "-i"], &mut args);
    args.push(source.to_string_lossy().into_owned());
    for input in &plan.extra_inputs {
        push(&["-framerate", "30", "-loop", "1", "-i"], &mut args);
        args.push(input.to_string_lossy().into_owned());
    }
    if let Some(fc) = &plan.filter_complex {
        args.push("-filter_complex".to_string());
        args.push(fc.clone());
        args.push("-map".to_string());
        args.push(format!("[{}]", plan.video_map.trim_matches(['[', ']'])));
    }
    push(
        &[
            "-frames:v",
            "45",
            "-an",
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgba",
            "-",
        ],
        &mut args,
    );

    let output = Command::new(ffmpeg)
        .args(&args)
        .stdin(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(format!(
            "ffmpeg failed: {}\nargs: {}",
            String::from_utf8_lossy(&output.stderr).trim(),
            args.join(" ")
        ));
    }

    let frame_bytes = (geom.canvas_w * geom.canvas_h * 4) as usize;
    let frames: Vec<Vec<u8>> = output
        .stdout
        .chunks(frame_bytes)
        .filter(|c| c.len() == frame_bytes)
        .map(<[u8]>::to_vec)
        .collect();

    SAMPLE_FRAMES
        .iter()
        .map(|index| {
            frames
                .get(*index as usize)
                .cloned()
                .ok_or_else(|| format!("frame {index} missing; only {} decoded", frames.len()))
        })
        .collect()
}

fn geometry_for(state: &RenderState) -> (u32, u32) {
    let geom = compute_canvas_geometry(
        CANVAS_W,
        CANVAS_H,
        state.padding,
        state.output_aspect.as_deref(),
    );
    (geom.canvas_w, geom.canvas_h)
}

fn digest(rgba: &[u8]) -> String {
    compare::digest_hex(rgba)
}

fn manifest_path() -> PathBuf {
    goldens_dir().join("manifest.json")
}

fn load_manifest() -> serde_json::Map<String, serde_json::Value> {
    std::fs::read_to_string(manifest_path())
        .ok()
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok())
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default()
}

fn save_manifest(map: &serde_json::Map<String, serde_json::Value>) {
    let json = serde_json::to_string_pretty(map).expect("serialize manifest");
    std::fs::write(
        manifest_path(),
        format!(
            "{json}
"
        ),
    )
    .expect("write manifest");
}

fn write_png(path: &Path, rgba: &[u8], (w, h): (u32, u32)) {
    let buffer = image::RgbaImage::from_raw(w, h, rgba.to_vec()).expect("golden dimensions");
    buffer.save(path).expect("write golden png");
}

fn read_png(path: &Path) -> Vec<u8> {
    image::open(path)
        .expect("read golden png")
        .to_rgba8()
        .into_raw()
}

#[test]
fn the_current_export_compositor_matches_its_goldens() {
    let Some(ffmpeg) = media::ffmpeg_path() else {
        if std::env::var("RECAST_TESTKIT_REQUIRE_FFMPEG").as_deref() == Ok("1") {
            panic!("RECAST_TESTKIT_REQUIRE_FFMPEG=1 but no ffmpeg was found");
        }
        eprintln!("skipping: no ffmpeg");
        return;
    };

    let scratch = Scratch::new("export");
    let source = scratch.0.join("source.mp4");
    let spec = SourceSpec {
        width: CANVAS_W,
        height: CANVAS_H,
        fps: 30,
        duration_secs: 1.6,
        ..Default::default()
    };
    media::write_source(&ffmpeg, spec, &source).expect("synthetic source");

    let dir = goldens_dir();
    std::fs::create_dir_all(&dir).expect("goldens dir");

    let mut manifest = load_manifest();
    let mut manifest_dirty = false;
    let mut failures = Vec::new();
    for (name, state) in fixtures() {
        let rendered = match render_fixture(&ffmpeg, &source, &state) {
            Ok(frames) => frames,
            Err(e) => {
                failures.push(format!("{name}: render failed: {e}"));
                continue;
            }
        };

        for (slot, frame) in rendered.iter().enumerate() {
            let key = format!("{name}-{}", SAMPLE_FRAMES[slot]);
            let png = dir.join(format!("{key}.png"));
            // Read the previous image BEFORE overwriting, or the delta compares the frame against itself and always passes.
            let previous = png.exists().then(|| read_png(&png));
            write_png(&png, frame, geometry_for(&state));

            let actual = digest(frame);
            match manifest.get(&key).and_then(serde_json::Value::as_str) {
                _ if updating() => {
                    manifest.insert(key, serde_json::Value::String(actual));
                    manifest_dirty = true;
                }
                None => {
                    manifest.insert(key, serde_json::Value::String(actual));
                    manifest_dirty = true;
                }
                Some(expected) if expected == actual => {}
                Some(expected) => {
                    let detail = match previous.as_deref().map(|p| compare::frame_delta(p, frame)) {
                        None => "no local image from a previous run to diff against".to_string(),
                        Some(None) => "canvas size changed".to_string(),
                        Some(Some(delta)) if delta.is_within(MAX_CHANNEL_DELTA, MAX_MEAN_DELTA) => {
                            format!(
                                "within visual tolerance but not byte-identical: max {} mean {:.3}",
                                delta.max_channel, delta.mean_channel
                            )
                        }
                        Some(Some(delta)) => format!(
                            "max {} mean {:.3} over {} differing px",
                            delta.max_channel, delta.mean_channel, delta.differing_pixels
                        ),
                    };
                    failures.push(format!(
                        "{key}: digest {} != {} ({detail}); see {}",
                        &actual[..12],
                        &expected[..12],
                        png.display()
                    ));
                }
            }
        }
    }

    if manifest_dirty {
        save_manifest(&manifest);
    }

    assert!(
        failures.is_empty(),
        "export output drifted from the goldens:\n  {}\n\nRe-run with UPDATE_GOLDENS=1 if the change was deliberate.",
        failures.join("\n  ")
    );
}

#[test]
fn every_fixture_produces_a_distinct_composite() {
    let Some(ffmpeg) = media::ffmpeg_path() else {
        return;
    };
    let scratch = Scratch::new("distinct");
    let source = scratch.0.join("source.mp4");
    let spec = SourceSpec {
        width: CANVAS_W,
        height: CANVAS_H,
        fps: 30,
        duration_secs: 1.6,
        ..Default::default()
    };
    media::write_source(&ffmpeg, spec, &source).expect("synthetic source");

    let mut seen: Vec<(String, Vec<u8>)> = Vec::new();
    for (name, state) in fixtures() {
        let frames = render_fixture(&ffmpeg, &source, &state).expect("render");
        for (other_name, other) in &seen {
            if other.len() == frames[1].len() {
                let delta = compare::frame_delta(other, &frames[1]).expect("same size");
                assert!(
                    delta.differing_pixels > 0,
                    "{name} rendered identically to {other_name}; the fixture is not exercising anything"
                );
            }
        }
        seen.push((name.to_string(), frames[1].clone()));
    }
}
