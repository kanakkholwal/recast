pub const DEFAULT_SAMPLE_RATE: u32 = 48_000;

const FRAME_SEC: f64 = 0.04;
const SEARCH_SEC: f64 = 0.004;
const MIN_FRAME: usize = 32;
const EPS: f32 = 1e-6;

/// Correlation is the hot loop; every second sample tracks the peak just as
/// well.
const CORR_STRIDE: usize = 2;

/// Resamples by `rate` with linear interpolation, so `rate` 2 halves the
/// length. Shifts pitch, which is why it is only the fallback for fragments too
/// short to stretch.
pub fn resample_linear(input: &[f32], rate: f64) -> Vec<f32> {
    // `is_finite` first, so a NaN rate is rejected rather than compared.
    if !rate.is_finite() || rate <= 0.0 || (rate - 1.0).abs() < EPS as f64 {
        return input.to_vec();
    }
    let out_length = ((input.len() as f64 / rate).floor() as usize).max(0);
    let mut out = Vec::with_capacity(out_length);
    for i in 0..out_length {
        let position = i as f64 * rate;
        let low = position.floor() as usize;
        let high = (low + 1).min(input.len().saturating_sub(1));
        let fraction = (position - low as f64) as f32;
        let (a, b) = (
            input.get(low).copied().unwrap_or(0.0),
            input.get(high).copied().unwrap_or(0.0),
        );
        out.push(a * (1.0 - fraction) + b * fraction);
    }
    out
}

fn hann(length: usize) -> Vec<f32> {
    (0..length)
        .map(|i| 0.5 - 0.5 * (2.0 * std::f64::consts::PI * i as f64 / length as f64).cos() as f32)
        .collect()
}

/// Offset within `radius` of `ideal` whose overlap region best continues the
/// window at `reference`.
///
/// This is the WSOLA search, and it is what keeps successive frames phase
/// aligned; a naive overlap-add doubles transients and sounds metallic.
fn best_offset(
    input: &[f32],
    ideal: isize,
    reference: usize,
    overlap: usize,
    radius: isize,
    max_start: usize,
) -> usize {
    let mut best_position = ideal.clamp(0, max_start as isize) as usize;
    let mut best_score = f64::NEG_INFINITY;
    for delta in -radius..=radius {
        let position = ideal + delta;
        if position < 0 || position > max_start as isize {
            continue;
        }
        let position = position as usize;
        // Accumulated in f64 to match the TypeScript, which computes every
        // arithmetic step at double precision however the samples are stored.
        // In f32 the sums drift enough to pick a different offset near a tie.
        let mut dot = 0.0f64;
        let mut energy = 0.0f64;
        let mut i = 0;
        while i < overlap {
            let a = input.get(position + i).copied().unwrap_or(0.0) as f64;
            dot += a * input.get(reference + i).copied().unwrap_or(0.0) as f64;
            energy += a * a;
            i += CORR_STRIDE;
        }
        // Normalising by the candidate's own energy stops the search always
        // picking the loudest window rather than the best-aligned one.
        let score = dot / (energy + EPS as f64).sqrt();
        if score > best_score {
            best_score = score;
            best_position = position;
        }
    }
    best_position
}

/// Changes playback speed without changing pitch, by WSOLA overlap-add. `rate`
/// 2 halves the duration.
///
/// Mirrors `time-stretch.ts` step for step, because the preview and the export
/// have to warp audio identically or a speed ramp drifts against the picture.
pub fn time_stretch(input: &[f32], rate: f64, sample_rate: u32) -> Vec<f32> {
    if !rate.is_finite() || rate <= 0.0 || (rate - 1.0).abs() < EPS as f64 {
        return input.to_vec();
    }
    let out_length = (input.len() as f64 / rate).floor() as usize;
    if out_length == 0 {
        return Vec::new();
    }

    // The frame has to fit the input twice over for the search to have anywhere
    // to go; below that a fragment is short enough that resampling is inaudible.
    let mut frame = ((FRAME_SEC * sample_rate as f64).round() as usize).min(input.len() / 4 * 2);
    frame -= frame % 2;
    if frame < MIN_FRAME {
        return resample_linear(input, rate);
    }

    let syn_hop = frame / 2;
    let ana_hop = syn_hop as f64 * rate;
    let radius = ((SEARCH_SEC * sample_rate as f64).round() as isize).max(1);
    let max_start = input.len() - frame;
    let window = hann(frame);

    let mut out = vec![0.0f32; out_length];
    let mut norm = vec![0.0f32; out_length];
    let mut previous = 0usize;

    let mut s = 0usize;
    while s * syn_hop < out_length {
        let syn_position = s * syn_hop;
        let ideal = (s as f64 * ana_hop).round() as isize;
        let chosen = match s {
            0 => ideal.clamp(0, max_start as isize) as usize,
            _ => best_offset(
                input,
                ideal,
                (previous + syn_hop).min(max_start),
                syn_hop,
                radius,
                max_start,
            ),
        };
        let span = frame.min(out_length - syn_position);
        for i in 0..span {
            let w = window[i];
            out[syn_position + i] += input.get(chosen + i).copied().unwrap_or(0.0) * w;
            norm[syn_position + i] += w;
        }
        previous = chosen;
        s += 1;
    }

    // Hann at a half hop sums to unity in the steady state but tapers at both
    // edges; dividing by the actual window sum keeps the gain flat end to end.
    for (sample, weight) in out.iter_mut().zip(norm) {
        if weight > EPS {
            *sample /= weight;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(samples: usize, hz: f64, sample_rate: u32) -> Vec<f32> {
        (0..samples)
            .map(|i| (2.0 * std::f64::consts::PI * hz * i as f64 / sample_rate as f64).sin() as f32)
            .collect()
    }

    /// Zero crossings per second stand in for pitch: a stretch must not move
    /// them, which is the whole difference from resampling.
    fn crossings(data: &[f32]) -> usize {
        data.windows(2)
            .filter(|w| (w[0] <= 0.0) != (w[1] <= 0.0))
            .count()
    }

    fn peak(data: &[f32]) -> f32 {
        data.iter().fold(0.0f32, |m, v| m.max(v.abs()))
    }

    #[test]
    fn a_rate_of_one_is_the_input_untouched() {
        let input = sine(4800, 440.0, DEFAULT_SAMPLE_RATE);
        assert_eq!(time_stretch(&input, 1.0, DEFAULT_SAMPLE_RATE), input);
    }

    #[test]
    fn doubling_the_rate_halves_the_length() {
        let input = sine(48_000, 440.0, DEFAULT_SAMPLE_RATE);
        let out = time_stretch(&input, 2.0, DEFAULT_SAMPLE_RATE);
        assert_eq!(out.len(), 24_000);
    }

    #[test]
    fn halving_the_rate_doubles_the_length() {
        let input = sine(24_000, 440.0, DEFAULT_SAMPLE_RATE);
        let out = time_stretch(&input, 0.5, DEFAULT_SAMPLE_RATE);
        assert_eq!(out.len(), 48_000);
    }

    /// The point of WSOLA: the tone comes out at the pitch it went in at.
    /// Resampling the same clip would double the crossing rate.
    #[test]
    fn stretching_keeps_the_pitch_where_resampling_moves_it() {
        let sample_rate = DEFAULT_SAMPLE_RATE;
        let input = sine(48_000, 440.0, sample_rate);
        let stretched = time_stretch(&input, 2.0, sample_rate);
        let resampled = resample_linear(&input, 2.0);

        let per_second =
            |data: &[f32]| crossings(data) as f64 * sample_rate as f64 / data.len() as f64;
        let source = per_second(&input);
        assert!(
            (per_second(&stretched) - source).abs() < source * 0.1,
            "stretched pitch moved: {} against {source}",
            per_second(&stretched)
        );
        assert!(
            per_second(&resampled) > source * 1.5,
            "the resampled control did not shift pitch, so this proves nothing"
        );
    }

    /// The normalisation exists so the ends do not fade. Only the first and
    /// last window taper, so the check has to sit INSIDE one of them: a wider
    /// span includes fully covered samples and the peak hides the fade.
    #[test]
    fn the_level_stays_flat_into_the_very_first_and_last_window() {
        let input = sine(48_000, 220.0, DEFAULT_SAMPLE_RATE);
        let out = time_stretch(&input, 1.5, DEFAULT_SAMPLE_RATE);
        let middle = peak(&out[out.len() / 2..out.len() / 2 + 2000]);
        let head = peak(&out[10..300]);
        let tail = peak(&out[out.len() - 300..out.len() - 10]);
        assert!(
            head > middle * 0.5,
            "the start faded: {head} against {middle}"
        );
        assert!(
            tail > middle * 0.5,
            "the end faded: {tail} against {middle}"
        );
        assert!(middle < 1.2, "the middle clipped at {middle}");
    }

    /// What the search buys: overlapping two windows at whatever phase the
    /// ideal offset lands on makes them cancel, and a steady tone comes out
    /// with a beating envelope. Aligning them keeps the level steady.
    #[test]
    fn the_search_keeps_a_steady_tone_steady() {
        let input = sine(48_000, 440.0, DEFAULT_SAMPLE_RATE);
        let out = time_stretch(&input, 1.5, DEFAULT_SAMPLE_RATE);
        // Skip the tapered first and last window.
        let body = &out[2000..out.len() - 2000];
        let envelope: Vec<f32> = body.chunks(480).map(peak).collect();
        let high = envelope.iter().fold(0.0f32, |m, v| m.max(*v));
        let low = envelope.iter().fold(f32::INFINITY, |m, v| m.min(*v));
        // Measured: the search holds this at 1.00, and taking the ideal offset
        // instead drops it to 0.93.
        assert!(
            low > high * 0.99,
            "the envelope beats between {low} and {high}"
        );
    }

    /// Short fragments cannot host the search window, and must fall back rather
    /// than return silence.
    #[test]
    fn a_fragment_too_short_to_stretch_is_resampled_instead() {
        // 60 samples give a 30-sample frame, just under the floor.
        let input = sine(60, 440.0, DEFAULT_SAMPLE_RATE);
        let out = time_stretch(&input, 2.0, DEFAULT_SAMPLE_RATE);
        assert_eq!(out, resample_linear(&input, 2.0));
        assert!(peak(&out) > 0.1, "the fallback produced silence");
    }

    #[test]
    fn a_nonsense_rate_leaves_the_input_alone() {
        let input = sine(1000, 440.0, DEFAULT_SAMPLE_RATE);
        assert_eq!(time_stretch(&input, 0.0, DEFAULT_SAMPLE_RATE), input);
        assert_eq!(time_stretch(&input, -1.0, DEFAULT_SAMPLE_RATE), input);
        assert_eq!(time_stretch(&input, f64::NAN, DEFAULT_SAMPLE_RATE), input);
    }

    #[test]
    fn an_empty_input_stretches_to_nothing() {
        assert!(time_stretch(&[], 2.0, DEFAULT_SAMPLE_RATE).is_empty());
        assert!(resample_linear(&[], 2.0).is_empty());
    }

    #[test]
    fn resampling_interpolates_rather_than_dropping_samples() {
        let input = [0.0, 1.0, 2.0, 3.0];
        let out = resample_linear(&input, 0.5);
        assert_eq!(out.len(), 8);
        assert!(
            (out[1] - 0.5).abs() < 1e-6,
            "no interpolation at {}",
            out[1]
        );
    }
}
