use crate::loudness::{normalizing_gain, DEFAULT_CEILING, DEFAULT_TARGET_LUFS, R128_RATE};
use crate::resample::Kernel;
use crate::source::{to_stereo, SampleSource};

/// The master rate. Pinned here rather than passed in, so an export can no
/// longer inherit whatever rate a surviving source happened to have (E4).
pub const MASTER_RATE: u32 = R128_RATE;
pub const MASTER_CHANNELS: usize = 2;

/// Widest source layout folded down by [`to_stereo`]. Anything past this reads
/// as its first eight channels.
const MAX_SOURCE_CHANNELS: usize = 8;

/// Longest mix rendered in one call. A duration arriving as infinity, or from a
/// corrupt project, would otherwise ask for an allocation that aborts the
/// process rather than failing.
const MAX_OUTPUT_SECONDS: f64 = 24.0 * 3600.0;

/// A ducked track falls to this while the key is speaking.
const DUCK_DEPTH: f32 = 0.25;
/// Roughly -34 dBFS. Below it the key counts as silence.
const DUCK_THRESHOLD: f32 = 0.02;
const DUCK_ATTACK_SEC: f64 = 0.04;
const DUCK_RELEASE_SEC: f64 = 0.4;

/// Where a source sits on the output timeline, and which part of it plays.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Placement {
    pub start_sec: f64,
    pub offset_sec: f64,
    /// 0 plays to the end of the source, or to the end of the output when looping.
    pub duration_sec: f64,
}

pub struct Track {
    pub source: Box<dyn SampleSource>,
    pub placement: Placement,
    pub gain: f32,
    pub muted: bool,
    pub fade_in: f64,
    pub fade_out: f64,
    pub looping: bool,
    /// Dips under the tracks that are not ducked, which together are the key.
    pub ducked: bool,
}

impl Track {
    pub fn new(source: Box<dyn SampleSource>) -> Self {
        Self {
            source,
            placement: Placement::default(),
            gain: 1.0,
            muted: false,
            fade_in: 0.0,
            fade_out: 0.0,
            looping: false,
            ducked: false,
        }
    }

    pub fn at(mut self, start_sec: f64) -> Self {
        self.placement.start_sec = start_sec;
        self
    }

    pub fn with_gain(mut self, gain: f32) -> Self {
        self.gain = gain;
        self
    }

    fn source_seconds(&self) -> f64 {
        self.source.frames() as f64 / self.source.sample_rate() as f64
    }

    /// How long this track plays on the output timeline.
    fn play_seconds(&self, output_seconds: f64) -> f64 {
        let span = (self.source_seconds() - self.placement.offset_sec).max(0.0);
        if self.placement.duration_sec > 0.0 {
            self.placement.duration_sec
        } else if self.looping {
            (output_seconds - self.placement.start_sec).max(0.0)
        } else {
            span
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Master {
    pub duration_sec: f64,
    pub gain: f32,
    pub muted: bool,
    pub fade_in: f64,
    pub fade_out: f64,
    /// EBU R128 over the finished mix. Only [`Mixer::render_all`] can honour it,
    /// because a gain measured over part of a mix is not the mix's gain.
    pub normalize: bool,
    pub target_lufs: f64,
    pub ceiling: f32,
}

impl Master {
    pub fn new(duration_sec: f64) -> Self {
        Self {
            duration_sec,
            gain: 1.0,
            muted: false,
            fade_in: 0.0,
            fade_out: 0.0,
            normalize: false,
            target_lufs: DEFAULT_TARGET_LUFS,
            ceiling: DEFAULT_CEILING,
        }
    }
}

pub struct Mixer {
    tracks: Vec<Track>,
    master: Master,
    position: u64,
    duck_level: f32,
    window: Vec<f32>,
    ducked_mix: Vec<f32>,
    duck_gain: Vec<f32>,
}

impl Mixer {
    pub fn new(master: Master) -> Self {
        Self {
            tracks: Vec::new(),
            master,
            position: 0,
            duck_level: 0.0,
            window: Vec::new(),
            ducked_mix: Vec::new(),
            duck_gain: Vec::new(),
        }
    }

    pub fn push(&mut self, track: Track) {
        self.tracks.push(track);
    }

    pub fn master(&self) -> &Master {
        &self.master
    }

    // NOT `clamp`: it passes NaN through, while `max` returns the other side and folds a non-finite duration to zero.
    #[allow(clippy::manual_clamp)]
    pub fn total_frames(&self) -> u64 {
        let seconds = self.master.duration_sec.max(0.0).min(MAX_OUTPUT_SECONDS);
        (seconds * MASTER_RATE as f64).round() as u64
    }

    /// Rewinds to the start, ducking envelope included.
    pub fn reset(&mut self) {
        self.position = 0;
        self.duck_level = 0.0;
    }

    /// Renders the next block, continuing where the last call stopped. `out` is
    /// interleaved stereo and is overwritten, not accumulated into.
    ///
    /// Blocks must be rendered in order: the ducking envelope has attack and
    /// release, so seeking needs [`Mixer::reset`] and a re-run.
    pub fn render_into(&mut self, out: &mut [f32]) {
        let frames = out.len() / MASTER_CHANNELS;
        let start = self.position;
        let duration = self.master.duration_sec;
        out.fill(0.0);

        let mut window = std::mem::take(&mut self.window);
        for track in self.tracks.iter().filter(|t| !t.ducked) {
            render_track(track, duration, start, frames, &mut window, out);
        }

        if self.tracks.iter().any(|t| t.ducked && !t.muted) {
            self.duck_gain.clear();
            self.duck_gain.resize(frames, 1.0);
            let attack = coefficient(DUCK_ATTACK_SEC);
            let release = coefficient(DUCK_RELEASE_SEC);
            for frame in 0..frames {
                let key = out[frame * 2].abs().max(out[frame * 2 + 1].abs());
                let target = if key > DUCK_THRESHOLD { 1.0 } else { 0.0 };
                let rate = if target > self.duck_level {
                    attack
                } else {
                    release
                };
                self.duck_level += (target - self.duck_level) * rate;
                self.duck_gain[frame] = 1.0 - (1.0 - DUCK_DEPTH) * self.duck_level;
            }

            let mut bus = std::mem::take(&mut self.ducked_mix);
            bus.clear();
            bus.resize(out.len(), 0.0);
            for track in self.tracks.iter().filter(|t| t.ducked) {
                render_track(track, duration, start, frames, &mut window, &mut bus);
            }
            for frame in 0..frames {
                let gain = self.duck_gain[frame];
                out[frame * 2] += bus[frame * 2] * gain;
                out[frame * 2 + 1] += bus[frame * 2 + 1] * gain;
            }
            self.ducked_mix = bus;
        }
        self.window = window;

        let gain = if self.master.muted {
            0.0
        } else {
            self.master.gain
        };
        for frame in 0..frames {
            let envelope = gain
                * fade_envelope(
                    (start + frame as u64) as f64 / MASTER_RATE as f64,
                    duration,
                    self.master.fade_in,
                    self.master.fade_out,
                );
            out[frame * 2] *= envelope;
            out[frame * 2 + 1] *= envelope;
        }

        self.position += frames as u64;
    }

    /// The whole mix from the start, with loudness normalisation applied when
    /// the master asks for it.
    pub fn render_all(&mut self) -> Vec<f32> {
        self.reset();
        let mut out = vec![0.0f32; self.total_frames() as usize * MASTER_CHANNELS];
        // One call rather than a block loop: the sources are random-access, so blocking buys nothing and the ducking envelope stays whole.
        self.render_into(&mut out);
        if self.master.normalize {
            let gain = normalizing_gain(&out, self.master.target_lufs, self.master.ceiling);
            if (gain - 1.0).abs() > 1e-6 {
                for sample in &mut out {
                    *sample *= gain;
                }
            }
        }
        out
    }
}

/// Adds one track's contribution to `out`, resampled to the master rate.
fn render_track(
    track: &Track,
    output_seconds: f64,
    block_start: u64,
    frames: usize,
    window: &mut Vec<f32>,
    out: &mut [f32],
) {
    if track.muted || track.gain == 0.0 || frames == 0 {
        return;
    }
    let play = track.play_seconds(output_seconds);
    if play <= 0.0 {
        return;
    }
    let source_rate = track.source.sample_rate();
    let channels = (track.source.channels() as usize).clamp(1, MAX_SOURCE_CHANNELS);
    let ratio = source_rate as f64 / MASTER_RATE as f64;
    let kernel = Kernel::new(ratio);
    let half = kernel.half_width();
    let span = (track.source_seconds() - track.placement.offset_sec).max(0.0);
    let loop_span = if track.looping && span > 0.0 {
        span
    } else {
        f64::INFINITY
    };

    let mut frame = 0usize;
    while frame < frames {
        let rel =
            (block_start + frame as u64) as f64 / MASTER_RATE as f64 - track.placement.start_sec;
        if rel < 0.0 {
            frame += 1;
            continue;
        }
        if rel >= play {
            return;
        }
        // A run is the stretch of output frames reading a contiguous piece of the source, ending where a loop or the clip does.
        let (phase, run_end) = if loop_span.is_finite() {
            let cycle = (rel / loop_span).floor();
            (
                rel - cycle * loop_span,
                ((cycle + 1.0) * loop_span).min(play),
            )
        } else {
            (rel, play)
        };
        let run = (((run_end - rel) * MASTER_RATE as f64).ceil() as usize)
            .max(1)
            .min(frames - frame);

        let first = (track.placement.offset_sec + phase) * source_rate as f64;
        let last = first + (run - 1) as f64 * ratio;
        let window_start = (first - half).floor() as i64;
        let window_end = (last + half).ceil() as i64;
        let window_frames = (window_end - window_start + 1).max(1) as usize;
        window.clear();
        window.resize(window_frames * channels, 0.0);
        track.source.read(window_start, window);

        let mut source_frame = [0.0f32; MAX_SOURCE_CHANNELS];
        let mut stereo = [0.0f32; 2];
        for step in 0..run {
            let position = first + step as f64 * ratio;
            for (channel, value) in source_frame.iter_mut().enumerate().take(channels) {
                *value = kernel.sample(window, window_start, channels, channel, position);
            }
            to_stereo(&source_frame[..channels], &mut stereo);
            let envelope = track.gain
                * fade_envelope(
                    rel + step as f64 / MASTER_RATE as f64,
                    play,
                    track.fade_in,
                    track.fade_out,
                );
            let at = (frame + step) * 2;
            out[at] += stereo[0] * envelope;
            out[at + 1] += stereo[1] * envelope;
        }
        frame += run;
    }
}

/// Per-sample smoothing coefficient for a time constant in seconds.
fn coefficient(seconds: f64) -> f32 {
    if seconds <= 0.0 {
        return 1.0;
    }
    (1.0 - (-1.0 / (seconds * MASTER_RATE as f64)).exp()) as f32
}

/// Linear in and out, the `tri` curve the FFmpeg path used, so an existing
/// project fades the way it always did. Overlapping fades multiply, which is
/// what chaining two `afade` filters did.
fn fade_envelope(at: f64, length: f64, fade_in: f64, fade_out: f64) -> f32 {
    let mut envelope = 1.0f64;
    if fade_in > 0.0 && at < fade_in {
        envelope *= (at / fade_in).clamp(0.0, 1.0);
    }
    let fade_out = fade_out.min(length);
    if fade_out > 0.0 && length > 0.0 && at > length - fade_out {
        envelope *= ((length - at) / fade_out).clamp(0.0, 1.0);
    }
    envelope as f32
}
