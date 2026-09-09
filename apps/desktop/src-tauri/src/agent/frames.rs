//! `frames`: what the recording looks like at evenly spaced OUTPUT times, as small JPEGs on disk the agent can open.
//! Cuts are honoured through the time map, so a frame inside a removed span is never shown.

use std::path::PathBuf;

use recast_time::TimeMap;
use serde::Serialize;
use sha2::{Digest, Sha256};

use super::axis::to_source;
use super::track::Window;

/// Enough to read a screen recording's flow; more is a scrub, which the agent should do with a narrower window.
pub const MAX_FRAMES: u32 = 24;
pub const DEFAULT_FRAMES: u32 = 12;
/// Wide enough to read UI text at a glance, small enough that a dozen stay cheap to look at.
pub const FRAME_WIDTH: u32 = 640;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Frame {
    /// OUTPUT seconds, the clock every other agent verb speaks.
    pub output: f64,
    /// The source second the frame was decoded at.
    pub source: f64,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FramesView {
    pub label: &'static str,
    pub window: (f64, f64),
    pub width: u32,
    pub frames: Vec<Frame>,
    pub hint: &'static str,
}

/// The output times a request samples: `count` frames centred in equal slices of the window.
pub fn sample_times(window: (f64, f64), count: u32) -> Vec<f64> {
    let count = count.clamp(1, MAX_FRAMES);
    let span = (window.1 - window.0).max(0.0);
    (0..count)
        .map(|i| window.0 + span * (f64::from(i) + 0.5) / f64::from(count))
        .collect()
}

/// Where a project's frames live: one directory per project path, so a second read reuses the files.
pub fn frames_dir(project: &str) -> PathBuf {
    let digest = Sha256::digest(project.as_bytes());
    std::env::temp_dir()
        .join("recast-frames")
        .join(hex::encode(&digest[..8]))
}

/// Builds the view, decoding through `extract` (source seconds, width) and writing each JPEG next to the others.
/// Frames that fail to decode are skipped rather than failing the whole read.
pub fn frames_view(
    project: &str,
    map: &TimeMap,
    window: Option<Window>,
    count: u32,
    extract: &dyn Fn(f64, u32) -> Option<Vec<u8>>,
) -> std::io::Result<FramesView> {
    let window = window.map_or((0.0, map.output_duration), |w| (w.start, w.end));
    let dir = frames_dir(project);
    std::fs::create_dir_all(&dir)?;
    let mut frames = Vec::new();
    for output in sample_times(window, count) {
        let source = to_source(map, output);
        let path = dir.join(format!("{output:08.3}.jpg"));
        if !path.is_file() {
            let Some(bytes) = extract(source, FRAME_WIDTH) else {
                continue;
            };
            std::fs::write(&path, bytes)?;
        }
        frames.push(Frame {
            output,
            source,
            path: path.to_string_lossy().into_owned(),
        });
    }
    Ok(FramesView {
        label: super::perception::CONTENT_LABEL,
        window,
        width: FRAME_WIDTH,
        frames,
        hint: "open the paths to look; pass from and to (output seconds) with a small count to scrub a moment",
    })
}

/// Drops a project's cached frames, for when its media changed underneath them.
#[cfg(test)]
pub fn forget(project: &str) {
    let _ = std::fs::remove_dir_all(frames_dir(project));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::axis::time_map;
    use crate::render::graph::RenderState;
    use recast_scene::v1::CutRange;

    #[test]
    fn samples_sit_in_the_middle_of_equal_slices_and_the_count_is_clamped() {
        assert_eq!(sample_times((0.0, 10.0), 2), vec![2.5, 7.5]);
        assert_eq!(sample_times((4.0, 4.0), 3).len(), 3);
        assert_eq!(sample_times((0.0, 1.0), 0).len(), 1);
        assert_eq!(sample_times((0.0, 1.0), 99).len() as u32, MAX_FRAMES);
    }

    #[test]
    fn frames_skip_cut_spans_reuse_files_and_tolerate_a_failed_decode() {
        let mut state = RenderState {
            trim_end: 10.0,
            ..RenderState::default()
        };
        state.cuts.push(CutRange {
            start: 2.0,
            end: 6.0,
            extra: Default::default(),
        });
        let map = time_map(&state);
        let project = format!("test-{}", std::process::id());
        forget(&project);
        let asked = std::cell::RefCell::new(Vec::new());
        let extract = |source: f64, width: u32| {
            asked.borrow_mut().push((source, width));
            (source < 9.0).then(|| vec![0xFF, 0xD8, 0xFF])
        };
        let view = frames_view(&project, &map, None, 3, &extract).unwrap();
        assert_eq!(view.window, (0.0, 6.0));
        assert_eq!(
            view.frames.len(),
            2,
            "the last sample decodes at source 9, which the fake refuses"
        );
        assert_eq!(view.frames[0].output, 1.0);
        assert_eq!(view.frames[1].output, 3.0);
        assert_eq!(
            view.frames[1].source, 7.0,
            "output 3 s is source 7 s past the 2..6 cut"
        );
        assert!(std::path::Path::new(&view.frames[0].path).is_file());
        assert_eq!(asked.borrow().len(), 3);
        frames_view(&project, &map, None, 3, &extract).unwrap();
        assert_eq!(
            asked.borrow().len(),
            4,
            "only the failed frame is decoded again"
        );
        forget(&project);
        assert!(!frames_dir(&project).exists());
    }
}
