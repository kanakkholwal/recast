use std::f64::consts::PI;

use recast_audio::{
    integrated_lufs, Master, Mixer, SampleSource, Samples, Track, DEFAULT_TARGET_LUFS,
    MASTER_CHANNELS, MASTER_RATE,
};

fn tone(seconds: f64, hz: f64, amplitude: f32, rate: u32, channels: u16) -> Samples {
    let frames = (seconds * rate as f64) as usize;
    let mut data = Vec::with_capacity(frames * channels as usize);
    for i in 0..frames {
        let value = (2.0 * PI * hz * i as f64 / rate as f64).sin() as f32 * amplitude;
        for _ in 0..channels {
            data.push(value);
        }
    }
    Samples::new(data, rate, channels)
}

fn track(source: Samples) -> Track {
    Track::new(Box::new(source) as Box<dyn SampleSource>)
}

/// Peak of the left side over an output-time window, which is how every level
/// assertion below is phrased.
fn peak_between(mix: &[f32], from: f64, to: f64) -> f32 {
    let first = (from * MASTER_RATE as f64) as usize;
    let last = ((to * MASTER_RATE as f64) as usize).min(mix.len() / 2);
    mix[first * 2..last * 2]
        .iter()
        .fold(0.0f32, |m, v| m.max(v.abs()))
}

#[test]
fn a_track_is_silent_until_it_starts() {
    let mut mixer = Mixer::new(Master::new(3.0));
    mixer.push(track(tone(1.0, 440.0, 0.5, 48_000, 1)).at(1.0));
    let mix = mixer.render_all();
    assert_eq!(peak_between(&mix, 0.0, 0.9), 0.0);
    assert!(peak_between(&mix, 1.1, 1.9) > 0.45);
    assert_eq!(peak_between(&mix, 2.1, 3.0), 0.0);
}

#[test]
fn gain_scales_and_mute_silences() {
    let mut mixer = Mixer::new(Master::new(1.0));
    mixer.push(track(tone(1.0, 440.0, 0.8, 48_000, 1)).with_gain(0.5));
    let quieter = peak_between(&mixer.render_all(), 0.1, 0.9);
    assert!((quieter - 0.4).abs() < 0.01, "peaked at {quieter}");

    let mut muted = Mixer::new(Master::new(1.0));
    let mut only = track(tone(1.0, 440.0, 0.8, 48_000, 1));
    only.muted = true;
    muted.push(only);
    assert_eq!(peak_between(&muted.render_all(), 0.0, 1.0), 0.0);
}

/// Two sources sum. Averaging them, which is what `amix` does unless it is told
/// not to, would quietly halve a single-source export the moment a second track
/// appeared.
#[test]
fn tracks_sum_rather_than_average() {
    let mut mixer = Mixer::new(Master::new(1.0));
    mixer.push(track(tone(1.0, 440.0, 0.3, 48_000, 1)));
    mixer.push(track(tone(1.0, 440.0, 0.3, 48_000, 1)));
    let peak = peak_between(&mixer.render_all(), 0.1, 0.9);
    assert!((peak - 0.6).abs() < 0.02, "peaked at {peak}");
}

#[test]
fn a_forty_four_one_source_arrives_at_the_master_rate() {
    let mut mixer = Mixer::new(Master::new(1.0));
    mixer.push(track(tone(1.0, 1_000.0, 0.5, 44_100, 2)));
    let mix = mixer.render_all();
    assert_eq!(mix.len(), 48_000 * 2);
    let peak = peak_between(&mix, 0.1, 0.9);
    assert!((peak - 0.5).abs() < 0.02, "peaked at {peak}");
    // Zero crossings prove the pitch survived: a wrong ratio moves them even
    // when the level looks right.
    let left: Vec<f32> = mix[9_600 * 2..38_400 * 2].iter().step_by(2).copied().collect();
    let crossings = left
        .windows(2)
        .filter(|w| (w[0] <= 0.0) != (w[1] <= 0.0))
        .count();
    // 1 kHz over 0.6 s crosses zero 1200 times, whatever rate it arrived at.
    assert!(crossings.abs_diff(1_200) <= 2, "counted {crossings}");
}

#[test]
fn an_offset_skips_into_the_source() {
    let mut data = vec![0.0f32; 48_000];
    data.resize(96_000, 0.5);
    let mut clip = track(Samples::mono(data, 48_000));
    clip.placement.offset_sec = 1.0;
    let mut mixer = Mixer::new(Master::new(1.0));
    mixer.push(clip);
    assert!(peak_between(&mixer.render_all(), 0.1, 0.9) > 0.49);
}

#[test]
fn a_duration_stops_the_clip_early() {
    let mut clip = track(tone(4.0, 440.0, 0.5, 48_000, 1));
    clip.placement.duration_sec = 1.0;
    let mut mixer = Mixer::new(Master::new(3.0));
    mixer.push(clip);
    let mix = mixer.render_all();
    assert!(peak_between(&mix, 0.1, 0.9) > 0.45);
    assert_eq!(peak_between(&mix, 1.1, 3.0), 0.0);
}

#[test]
fn looping_fills_the_output_from_a_short_source() {
    let mut clip = track(tone(0.25, 1_000.0, 0.5, 48_000, 1));
    clip.looping = true;
    let mut mixer = Mixer::new(Master::new(2.0));
    mixer.push(clip);
    let mix = mixer.render_all();
    // Every quarter-second window has to carry signal, which is the point.
    for step in 0..7 {
        let from = step as f64 * 0.25 + 0.02;
        assert!(
            peak_between(&mix, from, from + 0.2) > 0.45,
            "the loop died at {from}s"
        );
    }
}

#[test]
fn fades_start_and_end_at_silence() {
    let mut master = Master::new(2.0);
    master.fade_in = 0.5;
    master.fade_out = 0.5;
    let mut mixer = Mixer::new(master);
    mixer.push(track(tone(2.0, 1_000.0, 0.5, 48_000, 1)));
    let mix = mixer.render_all();
    assert!(
        peak_between(&mix, 0.0, 0.01) < 0.02,
        "the fade in started loud"
    );
    assert!(
        peak_between(&mix, 1.99, 2.0) < 0.02,
        "the fade out ended loud"
    );
    assert!(peak_between(&mix, 0.8, 1.2) > 0.45, "the middle was faded");
    // Half way into a linear fade is half the level, not a curve.
    let quarter = peak_between(&mix, 0.24, 0.26);
    assert!((quarter - 0.25).abs() < 0.02, "a quarter in it read {quarter}");
}

/// Music dips while the key speaks and comes back afterwards. The key is DC
/// here, so the music level reads straight off the offset.
#[test]
fn a_ducked_track_dips_under_the_key_and_recovers() {
    let mut voice = vec![0.0f32; 48_000];
    voice.resize(96_000, 0.5);
    voice.resize(192_000, 0.0);
    let mut music = track(tone(4.0, 1_000.0, 0.2, 48_000, 1));
    music.ducked = true;
    let mut mixer = Mixer::new(Master::new(4.0));
    mixer.push(track(Samples::mono(voice, 48_000)));
    mixer.push(music);
    let mix = mixer.render_all();

    let before = peak_between(&mix, 0.5, 0.9);
    assert!(
        (before - 0.2).abs() < 0.01,
        "music read {before} before the key"
    );

    let under = mix[(1.8 * 48_000.0) as usize * 2..(1.95 * 48_000.0) as usize * 2]
        .iter()
        .step_by(2)
        .fold(0.0f32, |m, v| m.max((v - 0.5).abs()));
    assert!(under < 0.08, "music held {under} under the key");

    let after = peak_between(&mix, 3.5, 3.9);
    assert!(
        (after - 0.2).abs() < 0.01,
        "music read {after} after the key"
    );
}

#[test]
fn without_a_ducked_track_the_key_is_untouched() {
    let mut mixer = Mixer::new(Master::new(1.0));
    mixer.push(track(tone(1.0, 1_000.0, 0.5, 48_000, 1)));
    let peak = peak_between(&mixer.render_all(), 0.1, 0.9);
    assert!((peak - 0.5).abs() < 0.01, "peaked at {peak}");
}

#[test]
fn normalising_lifts_a_quiet_mix_onto_the_target() {
    let mut master = Master::new(4.0);
    master.normalize = true;
    let mut mixer = Mixer::new(master);
    mixer.push(track(tone(4.0, 1_000.0, 0.02, 48_000, 2)));
    let measured = integrated_lufs(&mixer.render_all()).expect("a measurement");
    assert!(
        (measured - DEFAULT_TARGET_LUFS).abs() < 0.3,
        "the mix landed at {measured} LUFS"
    );
}

/// The streaming path and the one-shot path have to agree sample for sample, or
/// the encoder hears a different mix from the one that was measured.
#[test]
fn block_rendering_matches_one_pass() {
    let build = || {
        let mut master = Master::new(2.0);
        master.fade_in = 0.2;
        master.fade_out = 0.3;
        let mut mixer = Mixer::new(master);
        mixer.push(track(tone(2.0, 300.0, 0.4, 44_100, 1)).at(0.1));
        let mut music = track(tone(0.7, 900.0, 0.25, 48_000, 2));
        music.ducked = true;
        music.looping = true;
        mixer.push(music);
        mixer
    };
    let whole = build().render_all();

    let mut blocked = build();
    let mut out: Vec<f32> = Vec::new();
    let mut block = vec![0.0f32; 1024 * MASTER_CHANNELS];
    while out.len() < whole.len() {
        blocked.render_into(&mut block);
        out.extend_from_slice(&block);
    }
    out.truncate(whole.len());

    for (index, (a, b)) in whole.iter().zip(&out).enumerate() {
        assert!(
            (a - b).abs() < 1e-6,
            "sample {index} was {a} in one pass and {b} in blocks"
        );
    }
}

#[test]
fn a_muted_master_produces_silence() {
    let mut master = Master::new(1.0);
    master.muted = true;
    let mut mixer = Mixer::new(master);
    mixer.push(track(tone(1.0, 440.0, 0.9, 48_000, 1)));
    assert_eq!(peak_between(&mixer.render_all(), 0.0, 1.0), 0.0);
}

/// A corrupt or unset duration must not turn into an allocation the size of the
/// address space. Nothing here should reach the allocator at all.
#[test]
fn an_impossible_duration_renders_nothing_rather_than_aborting() {
    for seconds in [f64::INFINITY, f64::NAN, -5.0, 1e30] {
        let mut mixer = Mixer::new(Master::new(seconds));
        mixer.push(track(tone(1.0, 440.0, 0.5, 48_000, 1)));
        let frames = mixer.total_frames();
        assert!(
            frames <= 24 * 3600 * MASTER_RATE as u64,
            "{seconds} asked for {frames} frames"
        );
    }
    // And zero is genuinely zero, not the cap.
    assert_eq!(Mixer::new(Master::new(0.0)).total_frames(), 0);
}
