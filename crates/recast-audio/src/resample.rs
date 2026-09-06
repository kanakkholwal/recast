use std::f64::consts::PI;

/// Half-width of the kernel in cycles of its own cutoff. 16 puts the stopband
/// far enough down that a full-scale sweep shows no audible image.
const HALF: f64 = 16.0;

fn sinc(x: f64) -> f64 {
    if x.abs() < 1e-9 {
        1.0
    } else {
        let p = PI * x;
        p.sin() / p
    }
}

fn blackman(t: f64) -> f64 {
    0.42 + 0.5 * (PI * t).cos() + 0.08 * (2.0 * PI * t).cos()
}

/// A windowed-sinc kernel for one rate ratio.
/// Downsampling lowers the cutoff and widens the kernel by the same factor, so the band that would alias is filtered before it is sampled rather than after.
#[derive(Debug, Clone, Copy)]
pub struct Kernel {
    cutoff: f64,
    half: f64,
}

impl Kernel {
    /// `ratio` is source frames per output frame: 2.0 halves the rate.
    pub fn new(ratio: f64) -> Self {
        let cutoff = if ratio > 1.0 { 1.0 / ratio } else { 1.0 };
        Self {
            cutoff,
            half: HALF / cutoff,
        }
    }

    /// Weight for a source sample `distance` frames from the wanted position.
    pub fn weight(&self, distance: f64) -> f64 {
        if distance.abs() >= self.half {
            return 0.0;
        }
        self.cutoff * sinc(self.cutoff * distance) * blackman(distance / self.half)
    }

    pub fn half_width(&self) -> f64 {
        self.half
    }

    /// Samples `channel` of an interleaved window at a fractional position, with `window` starting at source frame `window_start`.
    /// Taps are not renormalised per phase: at 32 taps the Blackman window holds their sum inside 2e-5 of unity, a hundred dB below audible, and a divide here is not free.
    pub fn sample(
        &self,
        window: &[f32],
        window_start: i64,
        channels: usize,
        channel: usize,
        position: f64,
    ) -> f32 {
        let first = (position - self.half).ceil() as i64;
        let last = (position + self.half).floor() as i64;
        let mut acc = 0.0f64;
        for frame in first..=last {
            let index = (frame - window_start) as usize * channels + channel;
            let Some(value) = window.get(index) else {
                continue;
            };
            acc += self.weight(frame as f64 - position) * *value as f64;
        }
        acc as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(frames: usize, hz: f64, rate: u32) -> Vec<f32> {
        (0..frames)
            .map(|i| (2.0 * PI * hz * i as f64 / rate as f64).sin() as f32)
            .collect()
    }

    /// Resamples a whole mono buffer, which the mixer does block by block.
    fn convert(input: &[f32], from: u32, to: u32) -> Vec<f32> {
        let ratio = from as f64 / to as f64;
        let kernel = Kernel::new(ratio);
        let frames = (input.len() as f64 / ratio).floor() as usize;
        (0..frames)
            .map(|i| kernel.sample(input, 0, 1, 0, i as f64 * ratio))
            .collect()
    }

    fn rms(data: &[f32]) -> f64 {
        if data.is_empty() {
            return 0.0;
        }
        (data.iter().map(|v| (*v as f64).powi(2)).sum::<f64>() / data.len() as f64).sqrt()
    }

    #[test]
    fn an_equal_rate_reproduces_the_input() {
        let input = sine(2048, 440.0, 48_000);
        let out = convert(&input, 48_000, 48_000);
        assert_eq!(out.len(), input.len());
        for (a, b) in out.iter().zip(&input) {
            assert!((a - b).abs() < 1e-3, "{a} against {b}");
        }
    }

    #[test]
    fn forty_four_one_to_forty_eight_keeps_the_tone_and_the_level() {
        let input = sine(44_100, 440.0, 44_100);
        let out = convert(&input, 44_100, 48_000);
        assert!(out.len().abs_diff(48_000) <= 1, "produced {}", out.len());
        // A steady sine holds its RMS through a correct resample, so a broken normalisation shows up before pitch does.
        let (a, b) = (rms(&input), rms(&out));
        assert!((a - b).abs() < 0.01, "rms {a} became {b}");
    }

    /// The whole reason for the widening kernel: 18 kHz sampled down to 24 kHz
    /// has no home below Nyquist, so it must be filtered out, not folded back to
    /// 6 kHz. A naive linear interpolator leaves that alias at nearly full scale.
    #[test]
    fn downsampling_filters_what_would_otherwise_alias() {
        let input = sine(48_000, 18_000.0, 48_000);
        let out = convert(&input, 48_000, 24_000);
        // Skip the kernel's edge, where the window is only partly fed.
        let interior = &out[2_000..out.len() - 2_000];
        assert!(
            rms(interior) < 0.02,
            "alias came through at rms {}",
            rms(interior)
        );
    }

    #[test]
    fn a_tone_inside_the_new_band_survives_downsampling() {
        let input = sine(48_000, 1_000.0, 48_000);
        let out = convert(&input, 48_000, 24_000);
        let interior = &out[2_000..out.len() - 2_000];
        assert!(
            (rms(interior) - rms(&input)).abs() < 0.02,
            "rms {} became {}",
            rms(&input),
            rms(interior)
        );
    }

    /// Direct current is the probe for the tap normalisation: the sum of the
    /// taps IS the gain at zero frequency, and it drifts with the fractional
    /// phase. A ratio that never repeats walks that phase across every value it
    /// can take, so a ripple has nowhere to hide.
    #[test]
    fn a_constant_stays_constant_through_a_fractional_ratio() {
        let input = vec![0.5f32; 8_000];
        let out = convert(&input, 44_100, 48_000);
        let interior = &out[100..out.len() - 100];
        let (low, high) = interior
            .iter()
            .fold((f32::MAX, f32::MIN), |(l, h), v| (l.min(*v), h.max(*v)));
        assert!(
            (high - low) < 1e-4,
            "the gain rippled between {low} and {high}"
        );
        assert!((low - 0.5).abs() < 1e-4, "and settled at {low}");
    }

    #[test]
    fn reading_past_the_window_gives_silence_not_a_panic() {
        let kernel = Kernel::new(1.0);
        let window = [0.5f32; 8];
        let value = kernel.sample(&window, 0, 1, 0, 4_000.0);
        assert_eq!(value, 0.0);
    }
}
