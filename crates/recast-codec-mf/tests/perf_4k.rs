//! What 4K60 costs the CPU when the pixels never leave the GPU.
//!
//! The budget the zero-copy path exists to meet: converting and encoding
//! 3840x2160 at 60fps must stay under a small fraction of one core, because the
//! machine is also running whatever the user is recording. Measured in real
//! time, paced to 60fps, so the number is "cost of keeping up" rather than
//! "cost of going as fast as possible".
//!
//! Build it with `--release`: a debug build measures rustc's inlining decisions.

#![cfg(windows)]

use std::time::{Duration, Instant};

use recast_codec::{select_preferred, VideoCodec};
use recast_codec_mf::{enumerate_encoders, D3dContext, EncodeConfig, H264Encoder};
use windows::Win32::Foundation::FILETIME;
use windows::Win32::System::Threading::{GetCurrentProcess, GetProcessTimes};

const WIDTH: u32 = 3840;
const HEIGHT: u32 = 2160;
const FPS: u32 = 60;
const SECONDS: u32 = 5;
/// Frames run before the clock starts. The first encode of a session pays for
/// driver setup and the encoder's own rate-control warmup, which measured as a
/// near doubling of the reported cost when it landed inside the window.
const WARMUP: usize = 60;
const BITRATE: u32 = 40_000_000;

/// Fraction of ONE core the convert-and-encode path may take.
const CPU_BUDGET: f64 = 0.08;

/// Distinct source frames, cycled so the encoder always sees motion without a
/// host upload inside the measured loop.
const SOURCES: usize = 8;

fn cpu_time() -> Duration {
    let mut created = FILETIME::default();
    let mut exited = FILETIME::default();
    let mut kernel = FILETIME::default();
    let mut user = FILETIME::default();
    // SAFETY: four out parameters, freshly declared, on our own process handle.
    unsafe {
        GetProcessTimes(
            GetCurrentProcess(),
            &mut created,
            &mut exited,
            &mut kernel,
            &mut user,
        )
        .expect("the process reports its own times");
    }
    let ticks = |t: FILETIME| (t.dwHighDateTime as u64) << 32 | t.dwLowDateTime as u64;
    Duration::from_nanos((ticks(kernel) + ticks(user)) * 100)
}

/// A diagonal gradient shifted by `step`, which gives the encoder real motion
/// without the flat-frame cost of a still or the absurd cost of noise.
fn gradient(step: usize) -> Vec<u8> {
    let mut pixels = vec![0u8; (WIDTH * HEIGHT * 4) as usize];
    let shift = (step * 40) as u32;
    for y in 0..HEIGHT {
        let row = (y * WIDTH * 4) as usize;
        for x in 0..WIDTH {
            let at = row + (x * 4) as usize;
            pixels[at] = ((x + shift) % 256) as u8;
            pixels[at + 1] = ((y + shift) % 256) as u8;
            pixels[at + 2] = ((x + y + shift) % 256) as u8;
            pixels[at + 3] = 255;
        }
    }
    pixels
}

/// Convert plus encode at 4K60, in real time, within the CPU budget.
#[test]
#[ignore = "live: needs a hardware H.264 encoder, and only means anything in --release"]
fn four_k_sixty_stays_within_the_cpu_budget() {
    let candidates = enumerate_encoders();
    let Some(descriptor) = select_preferred(&candidates, VideoCodec::H264) else {
        panic!("no H.264 encoder on this machine to measure");
    };
    let context = D3dContext::new().expect("a D3D11 device");
    let config = EncodeConfig {
        width: WIDTH,
        height: HEIGHT,
        frame_rate: (FPS, 1),
        bitrate: BITRATE,
        keyframe_interval: 0,
    };
    let mut encoder = H264Encoder::open_with_gpu(descriptor, config, &context)
        .expect("the encoder opens on the GPU");
    assert!(
        encoder.takes_textures(),
        "{} will not take textures, so there is no zero-copy path to measure",
        descriptor.label()
    );
    let converter = context
        .nv12_converter(WIDTH, HEIGHT, (FPS, 1))
        .expect("an NV12 converter");
    let frames: Vec<_> = (0..3)
        .map(|_| converter.frame().expect("an NV12 frame"))
        .collect();

    // Uploaded once, outside the loop: a per-frame upload would dominate the number.
    let sources: Vec<_> = (0..SOURCES)
        .map(|step| {
            let surface = context
                .shared_surface(WIDTH, HEIGHT)
                .expect("a source surface");
            context
                .write_bgra(&surface, &gradient(step))
                .expect("the source fills");
            surface
        })
        .collect();

    let total = WARMUP + (FPS * SECONDS) as usize;
    let period = Duration::from_nanos(1_000_000_000 / u64::from(FPS));
    let mut encoded_bytes = 0usize;
    let mut measured = None;
    let started = Instant::now();

    for index in 0..total {
        if index == WARMUP {
            measured = Some((Instant::now(), cpu_time()));
            encoded_bytes = 0;
        }
        let target = &frames[index % frames.len()];
        context
            .convert(&sources[index % SOURCES], &converter, target)
            .expect("the frame converts");
        let pts = (index as i64) * 10_000_000 / i64::from(FPS);
        for sample in encoder.encode_texture(target, pts, 0).expect("it encodes") {
            encoded_bytes += sample.data.len();
        }
        let due = started + period * (index as u32 + 1);
        if let Some(wait) = due.checked_duration_since(Instant::now()) {
            std::thread::sleep(wait);
        }
    }
    let (at, cpu_before) = measured.expect("the warmup ends before the run does");
    let wall = at.elapsed();
    let cpu = cpu_time() - cpu_before;
    for sample in encoder.finish().expect("the encoder flushes") {
        encoded_bytes += sample.data.len();
    }

    let share = cpu.as_secs_f64() / wall.as_secs_f64();
    eprintln!(
        "4K60: {} frames in {:.2}s wall, {:.3}s CPU ({:.1}% of one core), {:.1} Mbit/s",
        total - WARMUP,
        wall.as_secs_f64(),
        cpu.as_secs_f64(),
        share * 100.0,
        encoded_bytes as f64 * 8.0 / wall.as_secs_f64() / 1_000_000.0
    );
    assert!(
        wall < Duration::from_secs(u64::from(SECONDS) * 3 / 2),
        "the encode could not keep up with 60fps: {} frames took {wall:?}",
        total - WARMUP
    );
    assert!(
        share < CPU_BUDGET,
        "4K60 took {:.1}% of one core, over the {:.0}% budget",
        share * 100.0,
        CPU_BUDGET * 100.0
    );
}
