//! Output-side arguments appended after the filter graph and before the codec:
//! the duration cap, `-shortest`, and license metadata.
//!
//! Split out of `run_export_job`. Nothing here pushes an `-i`, so it is free of
//! the input-index arithmetic the rest of that function is threaded through.

use crate::commands::export::cuts_speed::{output_duration_cap, SpeedSegment};
use crate::commands::types::ExportRequest;
use crate::render::node_types::AudioClip;

/// What the output tail decided, for the progress UI.
pub(crate) struct OutputTail {
    /// The real length of the output file: the `-t` cap (cuts dropped + speed
    /// warped), NOT the raw trimmed span. Progress denominator and probe target.
    pub expected_output_secs: f64,
}

/// Append the duration cap, `-shortest` and credits metadata to `args`.
pub(crate) fn append_output_tail(
    args: &mut Vec<String>,
    request: &ExportRequest,
    duration: f64,
    source_duration: f64,
    speed_segments: &[SpeedSegment],
    has_generated_inputs: bool,
) -> OutputTail {
    // Filtergraph generators are infinite, so cap the output at the REAL post-edit length: the raw span bakes a frozen tail and truncates a slowed clip.
    let output_cap = output_duration_cap(&request.format, duration, speed_segments);
    if output_cap > 0.0 {
        args.extend(["-t".to_string(), format!("{output_cap:.3}")]);
    }
    let expected_output_secs = if output_cap > 0.0 {
        output_cap
    } else {
        source_duration
    };

    if duration <= 0.0 && has_generated_inputs {
        args.push("-shortest".to_string());
    }

    // CC-BY music requires credit, so bake the attribution into the output's comment metadata; GIF has no audio to credit.
    if request.format != "gif" {
        if let Some(comment) = build_credits_comment(&request.render_state.music_clips) {
            args.extend(["-metadata".to_string(), format!("comment={comment}")]);
        }
    }

    OutputTail {
        expected_output_secs,
    }
}

/// Attribution lines the music clips' licenses require (CC-BY), deduped and
/// joined for the output file's `comment` metadata so the credit travels with
/// the exported video. None when nothing needs crediting.
pub(crate) fn build_credits_comment(clips: &[AudioClip]) -> Option<String> {
    let mut seen: Vec<&str> = Vec::new();
    for clip in clips {
        if let Some(a) = clip.source.attribution() {
            if !seen.contains(&a) {
                seen.push(a);
            }
        }
    }
    if seen.is_empty() {
        return None;
    }
    Some(format!("Music: {}", seen.join("; ")))
}
