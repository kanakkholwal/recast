//! End-to-end harness for the OCR pipeline. These tests drive the REAL decode
//! path, so they need FFmpeg on the machine (and, for the OCR leg, a network
//! round trip to fetch the models). They are `#[ignore]`d so CI stays hermetic:
//! the pure logic is covered by the unit tests in the sibling modules, and the
//! Rust CI job deliberately never executes FFmpeg.
//!
//! Run them by hand:
//!
//! ```text
//! cargo test --features ocr --lib ocr::harness -- --ignored --nocapture
//! ```

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::ffmpeg::{configure_silent_command, ffmpeg_path};

use super::engine::{OcrEngine, OcrsEngine};
use super::frames::{probe_dims, sample_frames, SampleOpts};
use super::timeline::build_timeline;

/// Build a video with `scenes` hard cuts between flat colours, each `secs` long.
///
/// Flat colour is deliberate: it is the case that breaks a naive detector. A solid
/// frame has no luma gradient, so every scene hashes identically under dHash, and
/// only the colour-aware score can tell them apart. If the sampler ever regresses
/// to a grayscale or hash-only gate, this video is what catches it.
fn synth_video(dir: &Path, scenes: &[&str], secs: u32) -> Result<PathBuf, String> {
    let out = dir.join("synth.mp4");
    let mut cmd = Command::new(ffmpeg_path());
    cmd.args(["-nostdin", "-y", "-loglevel", "error"]);
    for color in scenes {
        cmd.args([
            "-f",
            "lavfi",
            "-i",
            &format!("color=c={color}:s=640x360:d={secs}:r=10"),
        ]);
    }
    let inputs: String = (0..scenes.len()).map(|i| format!("[{i}:v]")).collect();
    cmd.args([
        "-filter_complex",
        &format!("{inputs}concat=n={}:v=1[v]", scenes.len()),
        "-map",
        "[v]",
        "-pix_fmt",
        "yuv420p",
    ]);
    cmd.arg(&out);
    configure_silent_command(&mut cmd);
    let res = cmd.output().map_err(|e| format!("ffmpeg spawn: {e}"))?;
    if !res.status.success() {
        return Err(format!(
            "synth video failed: {}",
            String::from_utf8_lossy(&res.stderr)
        ));
    }
    Ok(out)
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("recast-ocr-harness-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

/// The decode + sampler path against a real file: three hard cuts must yield
/// exactly three kept frames out of the ~90 the coarse pass walks.
#[test]
#[ignore = "needs ffmpeg on the machine"]
fn sampler_keeps_one_frame_per_scene() {
    let dir = temp_dir("sampler");
    let video = synth_video(&dir, &["red", "green", "blue"], 3).expect("synth video");

    let (w, h, duration) = probe_dims(&video).expect("probe");
    assert_eq!((w, h), (640, 360));
    assert!((duration - 9.0).abs() < 0.5, "duration was {duration}");

    let opts = SampleOpts::default();
    let frames = sample_frames(&video, &opts).expect("sample");

    // 9s at the 3fps coarse rate is ~27 candidate frames; only the 3 scene starts
    // carry new information.
    assert_eq!(
        frames.len(),
        3,
        "expected one frame per scene, got {} at {:?}",
        frames.len(),
        frames.iter().map(|f| f.t_secs).collect::<Vec<_>>()
    );
    assert!(frames[0].t_secs < 0.5);
    assert!((frames[1].t_secs - 3.0).abs() < 0.5);
    assert!((frames[2].t_secs - 6.0).abs() < 0.5);
    for f in &frames {
        assert_eq!(f.rgba.len(), (f.width as usize) * (f.height as usize) * 4);
    }

    let _ = std::fs::remove_dir_all(&dir);
}

/// The whole pipeline against a real file: decode, sample, OCR with the real ocrs
/// models, and collapse into spans. Needs the network the first time to fetch the
/// two `.rten` models into the temp dir.
#[test]
#[ignore = "needs ffmpeg and a network round trip for the models"]
fn ocr_reads_text_off_a_real_video() {
    let dir = temp_dir("ocr");

    // A frame with real text on it, held long enough to be sampled.
    let video = dir.join("text.mp4");
    let mut cmd = Command::new(ffmpeg_path());
    cmd.args([
        "-nostdin",
        "-y",
        "-loglevel",
        "error",
        "-f",
        "lavfi",
        "-i",
        "color=c=white:s=1280x720:d=3:r=10",
        "-vf",
        "drawtext=text='Export Settings':fontcolor=black:fontsize=72:x=(w-tw)/2:y=(h-th)/2",
        "-pix_fmt",
        "yuv420p",
    ]);
    cmd.arg(&video);
    configure_silent_command(&mut cmd);
    let res = cmd.output().expect("ffmpeg spawn");
    assert!(
        res.status.success(),
        "drawtext video failed (is libfreetype compiled in?): {}",
        String::from_utf8_lossy(&res.stderr)
    );

    let models = download_models(&dir).expect("download models");
    let engine = OcrsEngine::new(&models.0, &models.1).expect("build engine");
    assert_eq!(engine.source(), "ocrs");

    let (_, _, duration) = probe_dims(&video).expect("probe");
    let frames = sample_frames(&video, &SampleOpts::default()).expect("sample");
    assert!(!frames.is_empty());

    let timeline = build_timeline(&frames, duration, &engine).expect("timeline");
    assert!(!timeline.spans.is_empty(), "no spans produced");

    let text: String = timeline
        .spans
        .iter()
        .flat_map(|s| s.elements.iter())
        .map(|e| e.content.to_lowercase())
        .collect::<Vec<_>>()
        .join(" ");
    println!("recognized: {text}");
    assert!(
        text.contains("export") || text.contains("settings"),
        "OCR did not read the drawn text, got: {text:?}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// Fetch the two ocrs models into `dir`, returning `(detection, recognition)`.
/// Bypasses the app's `AppHandle`-based model dir so the harness stays headless.
fn download_models(dir: &Path) -> Result<(PathBuf, PathBuf), String> {
    const DETECTION: &str = "https://ocrs-models.s3-accelerate.amazonaws.com/text-detection.rten";
    const RECOGNITION: &str =
        "https://ocrs-models.s3-accelerate.amazonaws.com/text-recognition.rten";

    let det = dir.join("text-detection.rten");
    let rec = dir.join("text-recognition.rten");
    let rt = tokio::runtime::Runtime::new().map_err(|e| format!("tokio runtime: {e}"))?;
    rt.block_on(async {
        let client = reqwest::Client::new();
        crate::transcription::download_file(&client, DETECTION, None, &det, |_, _| {}).await?;
        crate::transcription::download_file(&client, RECOGNITION, None, &rec, |_, _| {}).await?;
        Ok::<(), String>(())
    })?;
    Ok((det, rec))
}
