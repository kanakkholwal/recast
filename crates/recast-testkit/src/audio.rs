pub const CLICK_MS: f64 = 1.0;

/// One full-scale impulse on every whole second, so a decoded track's timing can
/// be checked against wall clock without trusting the container's own metadata.
pub fn click_track(sample_rate: u32, channels: u16, duration_secs: f64) -> Vec<i16> {
    let frames = (duration_secs * sample_rate as f64).round() as usize;
    let click_frames = ((CLICK_MS / 1000.0) * sample_rate as f64).round().max(1.0) as usize;
    let mut out = vec![0i16; frames * channels as usize];

    let mut second = 0usize;
    loop {
        let start = second * sample_rate as usize;
        if start >= frames {
            break;
        }
        for frame in start..(start + click_frames).min(frames) {
            for channel in 0..channels as usize {
                out[frame * channels as usize + channel] = i16::MAX;
            }
        }
        second += 1;
    }
    out
}

/// Seconds at which a click begins. Groups adjacent loud samples so one impulse
/// is reported once rather than per sample.
pub fn detect_clicks(samples: &[i16], sample_rate: u32, channels: u16) -> Vec<f64> {
    let threshold = i16::MAX / 2;
    let gap_frames = (sample_rate as usize / 4).max(1);
    let channels = channels.max(1) as usize;

    let mut out = Vec::new();
    let mut last_frame: Option<usize> = None;
    for (frame, chunk) in samples.chunks_exact(channels).enumerate() {
        if !chunk.iter().any(|s| s.abs() >= threshold) {
            continue;
        }
        if last_frame.is_some_and(|last| frame - last < gap_frames) {
            continue;
        }
        last_frame = Some(frame);
        out.push(frame as f64 / sample_rate as f64);
    }
    out
}

/// Largest absolute difference between where a click landed and where it was
/// expected, in seconds. `None` when the counts differ.
pub fn worst_click_drift(detected: &[f64], expected: &[f64]) -> Option<f64> {
    if detected.len() != expected.len() {
        return None;
    }
    detected
        .iter()
        .zip(expected)
        .map(|(a, b)| (a - b).abs())
        .fold(None, |worst: Option<f64>, d| {
            Some(worst.map_or(d, |w| w.max(d)))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_click_track_has_one_click_per_second() {
        let track = click_track(48_000, 2, 5.0);
        let clicks = detect_clicks(&track, 48_000, 2);
        assert_eq!(clicks.len(), 5);
    }

    #[test]
    fn clicks_land_exactly_on_the_second() {
        let track = click_track(48_000, 1, 4.0);
        let clicks = detect_clicks(&track, 48_000, 1);
        for (index, at) in clicks.iter().enumerate() {
            assert!((at - index as f64).abs() < 1e-9, "click {index} at {at}");
        }
    }

    #[test]
    fn drift_is_the_worst_case_not_the_average() {
        let drift = worst_click_drift(&[0.0, 1.002, 2.0], &[0.0, 1.0, 2.0]).unwrap();
        assert!((drift - 0.002).abs() < 1e-9);
    }

    #[test]
    fn a_missing_click_is_reported_rather_than_averaged_away() {
        assert_eq!(worst_click_drift(&[0.0, 1.0], &[0.0, 1.0, 2.0]), None);
    }

    #[test]
    fn a_shifted_track_reports_its_shift() {
        let mut track = vec![0i16; 48_000];
        track.extend(click_track(48_000, 1, 2.0));
        let clicks = detect_clicks(&track, 48_000, 1);
        assert!((clicks[0] - 1.0).abs() < 1e-6, "got {}", clicks[0]);
    }
}
