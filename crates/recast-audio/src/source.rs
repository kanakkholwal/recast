/// A positioned reader over decoded audio, in the source's own rate and channel count.
/// Random access rather than streaming, because the mixer places clips on an output timeline and reads them out of order.
pub trait SampleSource: Send {
    fn sample_rate(&self) -> u32;
    fn channels(&self) -> u16;
    fn frames(&self) -> u64;

    /// Fills `out` with interleaved frames starting at `start`, which may be
    /// negative or run past the end. Anything outside the source is silence.
    fn read(&self, start: i64, out: &mut [f32]);
}

/// Decoded audio held in memory, interleaved.
#[derive(Debug, Clone)]
pub struct Samples {
    data: Vec<f32>,
    sample_rate: u32,
    channels: u16,
}

impl Samples {
    pub fn new(data: Vec<f32>, sample_rate: u32, channels: u16) -> Self {
        let channels = channels.max(1);
        Self {
            data,
            sample_rate: sample_rate.max(1),
            channels,
        }
    }

    pub fn mono(data: Vec<f32>, sample_rate: u32) -> Self {
        Self::new(data, sample_rate, 1)
    }

    pub fn data(&self) -> &[f32] {
        &self.data
    }

    /// The samples themselves, for a caller that rewrites them in place rather
    /// than building a second buffer beside a track that is already gigabytes.
    pub fn into_data(self) -> Vec<f32> {
        self.data
    }
}

impl SampleSource for Samples {
    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn channels(&self) -> u16 {
        self.channels
    }

    fn frames(&self) -> u64 {
        (self.data.len() / self.channels as usize) as u64
    }

    fn read(&self, start: i64, out: &mut [f32]) {
        let channels = self.channels as usize;
        let frames = self.frames() as i64;
        out.fill(0.0);
        for (index, frame) in (start..).take(out.len() / channels).enumerate() {
            if frame < 0 || frame >= frames {
                continue;
            }
            let from = frame as usize * channels;
            let to = index * channels;
            out[to..to + channels].copy_from_slice(&self.data[from..from + channels]);
        }
    }
}

/// Folds any channel count into stereo.
/// Mono goes to both sides at unity, not at -3 dB: a mono microphone is the common case and halving it would quietly change every existing export.
pub fn to_stereo(frame: &[f32], out: &mut [f32; 2]) {
    match frame.len() {
        0 => *out = [0.0, 0.0],
        1 => *out = [frame[0], frame[0]],
        2 => *out = [frame[0], frame[1]],
        _ => {
            // Everything past the front pair spreads evenly across both sides, keeping a 5.1 centre audible without an unverifiable matrix.
            let rest: f32 = frame[2..].iter().sum::<f32>() / (frame.len() - 2) as f32;
            *out = [frame[0] + rest * 0.5, frame[1] + rest * 0.5];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_read_inside_the_source_is_the_source() {
        let samples = Samples::new(vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0], 48_000, 2);
        let mut out = [0.0f32; 4];
        samples.read(1, &mut out);
        assert_eq!(out, [3.0, 4.0, 5.0, 6.0]);
        assert_eq!(samples.frames(), 3);
    }

    #[test]
    fn a_read_straddling_the_edges_pads_with_silence() {
        let samples = Samples::mono(vec![1.0, 2.0], 48_000);
        let mut out = [9.0f32; 4];
        samples.read(-1, &mut out);
        assert_eq!(out, [0.0, 1.0, 2.0, 0.0]);
    }

    #[test]
    fn mono_reaches_both_sides_at_unity() {
        let mut out = [0.0f32; 2];
        to_stereo(&[0.5], &mut out);
        assert_eq!(out, [0.5, 0.5]);
    }

    #[test]
    fn stereo_passes_through_unchanged() {
        let mut out = [0.0f32; 2];
        to_stereo(&[0.25, -0.75], &mut out);
        assert_eq!(out, [0.25, -0.75]);
    }

    #[test]
    fn surround_keeps_the_extra_channels_audible() {
        let mut out = [0.0f32; 2];
        to_stereo(&[0.0, 0.0, 1.0, 1.0], &mut out);
        assert_eq!(out, [0.5, 0.5]);
    }
}
