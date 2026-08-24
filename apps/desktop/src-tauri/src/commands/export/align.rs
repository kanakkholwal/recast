//! Filter fragments that shift a companion capture track onto the screen
//! video's timeline. Each capture device (loopback, microphone, webcam) starts
//! at its own instant, so `recording::TrackOffsets` records how far each one
//! lags video frame 0 and this turns that into FFmpeg.

/// Below this the correction is inaudible and invisible, and emitting a filter
/// would only cost a resample. ITU-R BT.1359 puts A/V detectability near 45 ms;
/// 5 ms leaves a wide margin.
const MIN_CORRECTION_MS: i64 = 5;

/// Above this the measurement is not believable (a stalled capture thread, a
/// clock jump), and applying it would wreck an otherwise fine export. Nothing
/// legitimate takes 30 s to deliver its first sample.
const MAX_CORRECTION_MS: i64 = 30_000;

/// `Some(offset)` reduced to a correction worth applying.
fn usable(offset_ms: Option<i64>) -> Option<i64> {
    let ms = offset_ms?;
    (ms.abs() >= MIN_CORRECTION_MS && ms.abs() <= MAX_CORRECTION_MS).then_some(ms)
}

/// Audio filter placing a track that starts `offset_ms` after video frame 0
/// back onto the video's timeline. Positive pads the head with silence;
/// negative drops the head the track captured before the video existed.
pub(crate) fn audio_align_filter(offset_ms: Option<i64>) -> Option<String> {
    match usable(offset_ms)? {
        ms if ms > 0 => Some(format!("adelay={ms}:all=1")),
        ms => Some(format!(
            "atrim=start={:.3},asetpts=PTS-STARTPTS",
            -ms as f64 / 1000.0
        )),
    }
}

/// Seconds to pass to FFmpeg's `-itsoffset` for the camera input. Positive
/// pushes a late camera track later; negative pulls an early one forward, whose
/// now-negative leading frames the overlay drops.
pub(crate) fn camera_input_offset_secs(offset_ms: Option<i64>) -> Option<f64> {
    Some(usable(offset_ms)? as f64 / 1000.0)
}

/// Video equivalent for the camera overlay, for graphs that filter rather than
/// shift the input. Positive delays the track; negative drops its head.
#[allow(dead_code)]
pub(crate) fn video_align_filter(offset_ms: Option<i64>) -> Option<String> {
    match usable(offset_ms)? {
        ms if ms > 0 => Some(format!("setpts=PTS+{:.3}/TB", ms as f64 / 1000.0)),
        ms => Some(format!(
            "trim=start={:.3},setpts=PTS-STARTPTS",
            -ms as f64 / 1000.0
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_measurement_means_no_correction() {
        assert_eq!(audio_align_filter(None), None);
        assert_eq!(video_align_filter(None), None);
    }

    #[test]
    fn sub_threshold_skew_is_left_alone() {
        assert_eq!(audio_align_filter(Some(4)), None);
        assert_eq!(audio_align_filter(Some(-4)), None);
        assert_eq!(audio_align_filter(Some(0)), None);
    }

    #[test]
    fn a_late_track_is_delayed_into_place() {
        assert_eq!(
            audio_align_filter(Some(320)),
            Some("adelay=320:all=1".into())
        );
    }

    #[test]
    fn an_early_track_has_its_head_trimmed() {
        assert_eq!(
            audio_align_filter(Some(-750)),
            Some("atrim=start=0.750,asetpts=PTS-STARTPTS".into())
        );
    }

    #[test]
    fn an_implausible_measurement_is_ignored_rather_than_applied() {
        assert_eq!(audio_align_filter(Some(45_000)), None);
        assert_eq!(audio_align_filter(Some(-45_000)), None);
        assert_eq!(video_align_filter(Some(120_000)), None);
    }

    #[test]
    fn video_shifts_pts_forward_for_a_late_camera() {
        assert_eq!(
            video_align_filter(Some(500)),
            Some("setpts=PTS+0.500/TB".into())
        );
    }

    #[test]
    fn video_trims_the_head_of_an_early_camera() {
        assert_eq!(
            video_align_filter(Some(-1_250)),
            Some("trim=start=1.250,setpts=PTS-STARTPTS".into())
        );
    }

    #[test]
    fn camera_input_offset_is_reported_in_seconds() {
        assert_eq!(camera_input_offset_secs(Some(320)), Some(0.32));
        assert_eq!(camera_input_offset_secs(Some(-1_500)), Some(-1.5));
        assert_eq!(camera_input_offset_secs(Some(2)), None);
        assert_eq!(camera_input_offset_secs(None), None);
    }

    #[test]
    fn the_threshold_boundary_is_inclusive() {
        assert!(audio_align_filter(Some(MIN_CORRECTION_MS)).is_some());
        assert!(audio_align_filter(Some(MAX_CORRECTION_MS)).is_some());
        assert!(audio_align_filter(Some(MAX_CORRECTION_MS + 1)).is_none());
    }
}
