//! End-to-end harness for the OCR pipeline. These tests drive the REAL decode
//! path, so they need FFmpeg on the machine (and, for the OCR leg, a network
//! round trip to fetch the models). They are `#[ignore]`d so CI stays hermetic:
//! the pure logic is covered by the unit tests in the sibling modules, and the
//! Rust CI job deliberately never executes FFmpeg.
//!
//! Run them by hand (`ocr` is a default feature, so no extra flag is needed):
//!
//! ```text
//! cargo test --lib ocr::harness -- --ignored --nocapture
//! ```

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::ffmpeg::{configure_silent_command, ffmpeg_path};

use super::engine::{OcrEngine, OcrsEngine};
use super::frames::{probe_dims, sample_frames, SampleOpts, SampleTick};
use super::timeline::{build_timeline, TimelineOpts};

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
    let mut ticks: Vec<SampleTick> = Vec::new();
    let frames = sample_frames(&video, &opts, &mut |t| ticks.push(t)).expect("sample");

    // The bar counts every frame the decoder walked, not just kept ones; ffmpeg's fps rounding makes the tail approximate.
    let last = ticks.last().expect("at least one tick");
    assert!(
        (26..=28).contains(&last.scanned),
        "expected ~27 frames walked, got {}",
        last.scanned
    );
    assert!(ticks.iter().all(|t| t.total > 0), "duration was probed");
    // A tick reports keeps made BEFORE its own frame was judged, so the running count only climbs and never overshoots.
    assert!(ticks.windows(2).all(|w| w[0].kept <= w[1].kept));
    assert!(ticks.iter().all(|t| (t.kept as usize) <= frames.len()));

    // 9s at the 3fps coarse rate is ~27 candidate frames, and only the 3 scene starts carry new information.
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
    let frames = sample_frames(&video, &SampleOpts::default(), &mut |_| {}).expect("sample");
    assert!(!frames.is_empty());

    let opts = TimelineOpts { previews: true };
    let timeline =
        build_timeline(&frames, duration, &engine, &opts, &mut |_| {}).expect("timeline");
    assert!(!timeline.spans.is_empty(), "no spans produced");
    assert_eq!(timeline.stats.frames_read as usize, frames.len());
    // Previews were requested, so the review UI has something to show.
    assert!(timeline.spans.iter().all(|s| s
        .preview
        .as_deref()
        .is_some_and(|p| p.starts_with("data:image/jpeg;base64,"))));

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

/// Fetch the two ocrs models, returning `(detection, recognition)`. Bypasses the
/// app's `AppHandle`-based model dir so the harness stays headless. Cached in a
/// STABLE dir across runs, so a repeat run does not re-download 12 MB and the
/// timings in the benchmark below measure compute, not the network.
fn download_models(_dir: &Path) -> Result<(PathBuf, PathBuf), String> {
    const DETECTION: &str = "https://ocrs-models.s3-accelerate.amazonaws.com/text-detection.rten";
    const RECOGNITION: &str =
        "https://ocrs-models.s3-accelerate.amazonaws.com/text-recognition.rten";

    let cache = std::env::temp_dir().join("recast-ocr-models");
    std::fs::create_dir_all(&cache).map_err(|e| format!("create model cache: {e}"))?;
    let det = cache.join("text-detection.rten");
    let rec = cache.join("text-recognition.rten");
    if det.exists() && rec.exists() {
        return Ok((det, rec));
    }

    let rt = tokio::runtime::Runtime::new().map_err(|e| format!("tokio runtime: {e}"))?;
    rt.block_on(async {
        let client = reqwest::Client::new();
        if !det.exists() {
            crate::transcription::download_file(&client, DETECTION, None, &det, |_, _| {}).await?;
        }
        if !rec.exists() {
            crate::transcription::download_file(&client, RECOGNITION, None, &rec, |_, _| {})
                .await?;
        }
        Ok::<(), String>(())
    })?;
    Ok((det, rec))
}

/// Where the time actually goes on a realistic clip. Prints per-stage timings so
/// a slow read can be attributed rather than guessed at.
///
/// Run it against both profiles to see the build-mode gap:
/// ```text
/// cargo test --lib ocr::harness::benchmark -- --ignored --nocapture
/// cargo test --release --lib ocr::harness::benchmark -- --ignored --nocapture
/// ```
#[test]
#[ignore = "benchmark: needs ffmpeg + models"]
fn benchmark_where_the_time_goes() {
    use std::time::Instant;

    let dir = temp_dir("bench");
    // A 7s 1080p clip with text, i.e. the shape of a real screen recording.
    let video = dir.join("bench.mp4");
    let mut cmd = Command::new(ffmpeg_path());
    cmd.args([
        "-nostdin",
        "-y",
        "-loglevel",
        "error",
        "-f",
        "lavfi",
        "-i",
        "color=c=white:s=1920x1080:d=7:r=30",
        "-vf",
        "drawtext=text='Export Settings':fontcolor=black:fontsize=48:x=100:y=100,\
         drawtext=text='Frame rate 60fps':fontcolor=black:fontsize=48:x=100:y=200",
        "-pix_fmt",
        "yuv420p",
    ]);
    cmd.arg(&video);
    configure_silent_command(&mut cmd);
    assert!(cmd.output().expect("ffmpeg").status.success());

    let profile = if cfg!(debug_assertions) {
        "DEBUG"
    } else {
        "RELEASE"
    };

    let (_, _, duration) = probe_dims(&video).expect("probe");

    let t = Instant::now();
    let frames = sample_frames(&video, &SampleOpts::default(), &mut |_| {}).expect("sample");
    let sample_ms = t.elapsed().as_millis();

    let models = download_models(&dir).expect("models");
    let t = Instant::now();
    let engine = OcrsEngine::new(&models.0, &models.1).expect("engine");
    let load_ms = t.elapsed().as_millis();

    let t = Instant::now();
    let timeline = build_timeline(
        &frames,
        duration,
        &engine,
        &TimelineOpts::default(),
        &mut |_| {},
    )
    .expect("timeline");
    let ocr_ms = t.elapsed().as_millis();

    let per_frame = if frames.is_empty() {
        0
    } else {
        ocr_ms / frames.len() as u128
    };
    println!(
        "\n[{profile}] {duration:.1}s video, {} frames OCR'd, {} spans\n  \
         sample (decode+gate): {sample_ms} ms\n  \
         model load:           {load_ms} ms\n  \
         OCR:                  {ocr_ms} ms  ({per_frame} ms/frame)\n  \
         TOTAL:                {} ms\n",
        frames.len(),
        timeline.spans.len(),
        sample_ms + load_ms + ocr_ms
    );

    let _ = std::fs::remove_dir_all(&dir);
}
