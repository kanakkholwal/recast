/// The rate the R128 filter coefficients below are designed for. The spec gives
/// them at 48 kHz only, and the master node is pinned there, so redesigning them
/// per rate would be dead code.
pub const R128_RATE: u32 = 48_000;

/// Social-platform target. YouTube, Spotify and friends all normalise to about
/// here, so exporting louder only buys a gain reduction on their side.
pub const DEFAULT_TARGET_LUFS: f64 = -14.0;

/// Sample-peak ceiling applied after the loudness gain, linear (-1 dBFS).
pub const DEFAULT_CEILING: f32 = 0.891_25;

const BLOCK_SEC: f64 = 0.4;
const STEP_SEC: f64 = 0.1;
const ABSOLUTE_GATE: f64 = -70.0;
const RELATIVE_GATE: f64 = -10.0;
const OFFSET: f64 = -0.691;

#[derive(Debug, Clone, Copy)]
struct Biquad {
    b: [f64; 3],
    a: [f64; 2],
}

#[derive(Debug, Clone, Copy, Default)]
struct BiquadState {
    x: [f64; 2],
    y: [f64; 2],
}

impl Biquad {
    fn step(&self, state: &mut BiquadState, input: f64) -> f64 {
        let out = self.b[0] * input + self.b[1] * state.x[0] + self.b[2] * state.x[1]
            - self.a[0] * state.y[0]
            - self.a[1] * state.y[1];
        state.x = [input, state.x[0]];
        state.y = [out, state.y[0]];
        out
    }
}

/// Stage 1 of K-weighting: the head shelf.
const SHELF: Biquad = Biquad {
    b: [
        1.535_124_859_586_97,
        -2.691_696_189_406_38,
        1.198_392_810_852_85,
    ],
    a: [-1.690_659_293_182_41, 0.732_480_774_215_85],
};

/// Stage 2: the RLB high pass.
const HIGHPASS: Biquad = Biquad {
    b: [1.0, -2.0, 1.0],
    a: [-1.990_047_454_833_98, 0.990_072_250_366_21],
};

/// Integrated loudness of interleaved stereo at [`R128_RATE`], in LUFS.
///
/// `None` when nothing survives gating, which means silence: there is no
/// loudness to correct and the caller must not invent a gain for it.
pub fn integrated_lufs(stereo: &[f32]) -> Option<f64> {
    let frames = stereo.len() / 2;
    let block = (BLOCK_SEC * R128_RATE as f64) as usize;
    if frames < block {
        return None;
    }

    // K-weight both channels once, then square: the block loop is a moving sum
    // over these, so filtering per block would redo three quarters of the work.
    let mut squared = vec![0.0f64; frames * 2];
    for channel in 0..2 {
        let mut shelf = BiquadState::default();
        let mut high = BiquadState::default();
        for frame in 0..frames {
            let input = stereo[frame * 2 + channel] as f64;
            let value = HIGHPASS.step(&mut high, SHELF.step(&mut shelf, input));
            squared[frame * 2 + channel] = value * value;
        }
    }

    let step = (STEP_SEC * R128_RATE as f64) as usize;
    let mut blocks = Vec::new();
    let mut start = 0;
    while start + block <= frames {
        let mut power = 0.0;
        for channel in 0..2 {
            let sum: f64 = (start..start + block)
                .map(|f| squared[f * 2 + channel])
                .sum();
            power += sum / block as f64;
        }
        blocks.push(power);
        start += step;
    }

    let loud = |power: f64| OFFSET + 10.0 * power.max(f64::MIN_POSITIVE).log10();
    let above_absolute: Vec<f64> = blocks
        .into_iter()
        .filter(|p| loud(*p) > ABSOLUTE_GATE)
        .collect();
    if above_absolute.is_empty() {
        return None;
    }
    let mean = above_absolute.iter().sum::<f64>() / above_absolute.len() as f64;
    let threshold = loud(mean) + RELATIVE_GATE;
    let kept: Vec<f64> = above_absolute
        .into_iter()
        .filter(|p| loud(*p) > threshold)
        .collect();
    if kept.is_empty() {
        return None;
    }
    Some(loud(kept.iter().sum::<f64>() / kept.len() as f64))
}

/// Gain that puts `stereo` at `target` without letting a sample past `ceiling`.
/// 1.0 when there is nothing to measure.
///
/// One measured pass over the finished mix, not the streaming estimate that
/// single-pass `loudnorm` makes: export is offline, so the whole mix is already
/// in hand and guessing buys nothing.
pub fn normalizing_gain(stereo: &[f32], target: f64, ceiling: f32) -> f32 {
    let Some(measured) = integrated_lufs(stereo) else {
        return 1.0;
    };
    let gain = 10f64.powf((target - measured) / 20.0) as f32;
    let peak = stereo.iter().fold(0.0f32, |m, v| m.max(v.abs()));
    if peak <= 0.0 {
        return gain;
    }
    // Attenuating to the ceiling is the honest move: raising the mix and then
    // clipping it would meet the target on paper and distort in fact.
    gain.min(ceiling / peak)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    fn tone(seconds: f64, hz: f64, amplitude: f32) -> Vec<f32> {
        let frames = (seconds * R128_RATE as f64) as usize;
        let mut out = Vec::with_capacity(frames * 2);
        for i in 0..frames {
            let value = (2.0 * PI * hz * i as f64 / R128_RATE as f64).sin() as f32 * amplitude;
            out.push(value);
            out.push(value);
        }
        out
    }

    fn peak(data: &[f32]) -> f32 {
        data.iter().fold(0.0f32, |m, v| m.max(v.abs()))
    }

    /// EBU Tech 3341 case 1: a 1 kHz sine at -23 dBFS on both channels measures
    /// -23.0 LUFS. A wrong shelf, a wrong offset and a missed channel sum each
    /// move this by a few LU, and they partly cancel, so it needs the anchor
    /// rather than an eyeball.
    #[test]
    fn the_reference_tone_measures_where_the_spec_says() {
        let amplitude = 10f32.powf(-23.0 / 20.0);
        let measured = integrated_lufs(&tone(3.0, 1_000.0, amplitude)).expect("a measurement");
        assert!(
            (measured - -23.0).abs() < 0.1,
            "reference tone measured {measured} LUFS"
        );
    }

    #[test]
    fn halving_the_amplitude_costs_six_lu() {
        let loud = integrated_lufs(&tone(3.0, 1_000.0, 0.2)).unwrap();
        let quiet = integrated_lufs(&tone(3.0, 1_000.0, 0.1)).unwrap();
        assert!((loud - quiet - 6.02).abs() < 0.05, "{loud} against {quiet}");
    }

    /// Gating is the whole difference from a plain average: a long silent tail
    /// must not drag the reading down.
    #[test]
    fn silence_is_gated_out_rather_than_averaged_in() {
        let mut speech = tone(3.0, 1_000.0, 0.1);
        let alone = integrated_lufs(&speech).unwrap();
        speech.resize(speech.len() + R128_RATE as usize * 2 * 10, 0.0);
        let with_tail = integrated_lufs(&speech).unwrap();
        // Ungated, ten seconds of silence after three of tone reads about 6 LU
        // lower, so this margin is wide on purpose and still decisive.
        assert!(
            (alone - with_tail).abs() < 0.5,
            "a silent tail moved the reading from {alone} to {with_tail}"
        );
    }

    /// Both channels are summed, not doubled from one. Every other test here
    /// feeds identical sides, where those two are the same arithmetic.
    #[test]
    fn a_tone_on_one_side_only_is_three_lu_quieter() {
        let amplitude = 10f32.powf(-23.0 / 20.0);
        let mut left = tone(3.0, 1_000.0, amplitude);
        for frame in 0..left.len() / 2 {
            left[frame * 2 + 1] = 0.0;
        }
        let measured = integrated_lufs(&left).expect("a measurement");
        assert!(
            (measured - -26.01).abs() < 0.1,
            "one side alone measured {measured} LUFS"
        );
    }

    /// The relative gate, which the absolute gate cannot stand in for: room tone
    /// at -50 dBFS is far above -70 LUFS, so only the -10 LU window keeps it out
    /// of the average.
    #[test]
    fn quiet_room_tone_is_gated_out_of_a_loud_programme() {
        let mut programme = tone(3.0, 1_000.0, 0.5);
        let alone = integrated_lufs(&programme).unwrap();
        programme.extend_from_slice(&tone(10.0, 1_000.0, 0.003));
        let with_room = integrated_lufs(&programme).unwrap();
        assert!(
            (alone - with_room).abs() < 0.5,
            "room tone moved the reading from {alone} to {with_room}"
        );
    }

    #[test]
    fn silence_has_no_measurement_and_so_no_gain() {
        let silence = vec![0.0f32; R128_RATE as usize * 2];
        assert!(integrated_lufs(&silence).is_none());
        assert_eq!(
            normalizing_gain(&silence, DEFAULT_TARGET_LUFS, DEFAULT_CEILING),
            1.0
        );
    }

    #[test]
    fn a_clip_shorter_than_one_block_has_no_measurement() {
        assert!(integrated_lufs(&tone(0.2, 1_000.0, 0.5)).is_none());
    }

    #[test]
    fn the_gain_lands_the_mix_on_the_target() {
        let quiet = tone(4.0, 1_000.0, 0.02);
        let gain = normalizing_gain(&quiet, DEFAULT_TARGET_LUFS, DEFAULT_CEILING);
        let lifted: Vec<f32> = quiet.iter().map(|v| v * gain).collect();
        let measured = integrated_lufs(&lifted).unwrap();
        assert!(
            (measured - DEFAULT_TARGET_LUFS).abs() < 0.2,
            "landed at {measured} LUFS"
        );
    }

    /// A sparse mix measures quiet and peaks high, so the loudness target asks
    /// for a lift the samples have no room for. The ceiling has to win.
    ///
    /// A steady loud tone does NOT test this: it is already near the target, so
    /// the gain it asks for is well under the ceiling either way.
    #[test]
    fn the_ceiling_outranks_the_target() {
        let mut bursts = tone(4.0, 1_000.0, 0.9);
        // 10 ms of tone every 400 ms: loud peaks, quiet programme. The period
        // matches the gating block, so every block holds the same energy.
        let (period, burst) = (R128_RATE as usize * 2 / 5, R128_RATE as usize / 100);
        for frame in 0..bursts.len() / 2 {
            if frame % period >= burst {
                bursts[frame * 2] = 0.0;
                bursts[frame * 2 + 1] = 0.0;
            }
        }
        let measured = integrated_lufs(&bursts).expect("a measurement");
        let wanted = 10f64.powf((DEFAULT_TARGET_LUFS - measured) / 20.0) as f32;
        let allowed = DEFAULT_CEILING / peak(&bursts);
        assert!(
            wanted > allowed,
            "the fixture asks for {wanted} and is allowed {allowed}, so the ceiling never binds"
        );

        let gain = normalizing_gain(&bursts, DEFAULT_TARGET_LUFS, DEFAULT_CEILING);
        assert!(
            (gain - allowed).abs() < 1e-6,
            "the ceiling did not bind: gain {gain} against a wanted {wanted}"
        );
    }
}
