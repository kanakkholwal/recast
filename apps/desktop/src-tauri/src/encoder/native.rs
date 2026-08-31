//! FFmpeg-free recording writer: a capturekit GPU handle becomes NV12, is encoded by Media Foundation and muxed, never touching host memory.
//! Timing is variable-rate, so a still desktop costs one long sample and a stutter is recorded rather than papered over.

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;
use std::sync::atomic::Ordering;

use anyhow::{anyhow, Context, Result};
use capturekit::GpuHandle;
use recast_codec::{select_preferred, VideoCodec};
use recast_codec_mf::{
    enumerate_encoders, D3dContext, EncodeConfig, H264Encoder, Nv12Converter, Nv12Frame,
    SharedSurface, SyncFence,
};
use recast_mux::avc::annex_b_to_avcc;
use recast_mux::fragment::FragmentedWriter;
use recast_mux::writer::VideoFormat;

use crate::capture::CapturedFrame;
use crate::recording::pipeline::{FrameSink, PipelineStats};

/// Sample durations are in these ticks per second.
/// 90 kHz is the MPEG convention and divides every common frame rate closely enough that per-sample rounding never accumulates into drift.
const TIMESCALE: u32 = 90_000;

/// Media Foundation timestamps are 100 ns units.
const MF_TICKS_PER_SEC: i64 = 10_000_000;

/// The longest a still desktop may go without a sample.
/// A player seeking into a gap has nothing to show, and a sample that never ends cannot be seeked past, so a keepalive repeat bounds both.
pub const KEEPALIVE: std::time::Duration = std::time::Duration::from_millis(500);

/// The longest a recording may go without a keyframe, which is what a scrub costs, since a seek decodes from the keyframe before it.
/// Measured in TIME, not frames: this writer is variable rate, and a still desktop produces far fewer frames per second than it was opened at.
const KEYFRAME_INTERVAL_US: u64 = 500_000;

/// Encodes capture frames straight from the GPU into a fragmented MP4.
pub struct NativeRecorder {
    context: D3dContext,
    encoder: H264Encoder,
    converter: Nv12Converter,
    writer: FragmentedWriter,
    out: BufWriter<File>,
    /// Round-robined: a hardware encoder is asynchronous and still holds the
    /// previous frame when the next conversion starts.
    frames: Vec<Nv12Frame>,
    next_frame: usize,
    /// The producer's surface and both fences, opened once. `OpenSharedResource1` is a kernel handle open; repeating it 60 times a second is a syscall the zero-copy path exists to avoid.
    imported: Option<Imported>,
    /// The last frame released back to the producer, which is also what tells a new picture from a keepalive: the capture hands the same handle back when the display has not changed.
    released: u64,
    /// The slot holding the picture last converted, which a repeat copies from.
    last_converted: Option<usize>,
    /// When the last keyframe was asked for, so the next is due by time.
    last_keyframe_us: Option<u64>,
    /// The sample whose duration is not known yet. VFR means a sample lasts
    /// until the NEXT frame arrives, so the writer is always one behind.
    pending: Option<PendingSample>,
    wrote_init: bool,
    /// Whether the encoder has handed over its parameter sets. They arrive with the first encoded sample, NOT when the encoder opens, and the MP4 header cannot be built without them.
    have_config: bool,
    samples: u64,
}

/// The producer's shared objects, as this device sees them.
struct Imported {
    texture_handle: isize,
    texture: SharedSurface,
    ready: SyncFence,
    release: SyncFence,
}

struct PendingSample {
    data: Vec<u8>,
    is_sync: bool,
    /// The encoder's own stamp for this sample, which its duration runs from.
    pts_us: u64,
}

impl NativeRecorder {
    pub fn open(path: &Path, width: u32, height: u32, fps: u32, bitrate: u32) -> Result<Self> {
        // NV12 carries chroma at half resolution and has nowhere to put an odd row.
        let (width, height) = (width & !1, height & !1);
        let candidates = enumerate_encoders();
        let descriptor = select_preferred(&candidates, VideoCodec::H264)
            .ok_or_else(|| anyhow!("no H.264 encoder is available on this machine"))?;
        let context = D3dContext::new().map_err(|e| anyhow!("open a D3D11 device: {e:?}"))?;
        let config = EncodeConfig {
            width,
            height,
            frame_rate: (fps.max(1), 1),
            bitrate,
            // Zero: keyframes are asked for by TIME, not by frame count.
            keyframe_interval: 0,
        };
        let encoder = H264Encoder::open_with_gpu(descriptor, config, &context)
            .map_err(|e| anyhow!("open the {} encoder: {e:?}", descriptor.label()))?;
        if !encoder.takes_textures() {
            return Err(anyhow!(
                "the {} encoder will not take textures, so the capture would have to \
                 be read back to host memory",
                descriptor.label()
            ));
        }
        let converter = context
            .nv12_converter(width, height, (fps.max(1), 1))
            .map_err(|e| anyhow!("create the NV12 converter: {e:?}"))?;
        // Three: one being encoded, one queued, one being written.
        let frames = (0..3)
            .map(|_| converter.frame())
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| anyhow!("allocate NV12 frames: {e:?}"))?;

        let writer = FragmentedWriter::new(VideoFormat {
            width: u16::try_from(width).context("width does not fit an MP4 track")?,
            height: u16::try_from(height).context("height does not fit an MP4 track")?,
            timescale: TIMESCALE,
        });
        let out = BufWriter::new(
            File::create(path).with_context(|| format!("create {}", path.display()))?,
        );
        Ok(Self {
            context,
            encoder,
            converter,
            writer,
            out,
            frames,
            next_frame: 0,
            imported: None,
            released: 0,
            last_converted: None,
            last_keyframe_us: None,
            pending: None,
            wrote_init: false,
            have_config: false,
            samples: 0,
        })
    }

    /// Encodes one captured frame stamped at `pts_us` on the recording clock.
    /// The previous sample's duration is only known now, which is what makes this variable rate and keeps the writer one frame behind; an already-taken handle is a keepalive.
    pub fn push(&mut self, handle: &GpuHandle, pts_us: u64) -> Result<()> {
        if handle.ready_at <= self.released {
            return self.repeat(pts_us);
        }
        self.import(handle)?;
        let index = self.take_slot();
        let imported = self
            .imported
            .as_ref()
            .expect("import stores the surface or fails");
        // Nothing orders the two devices; converting before this reads zeroes.
        self.context
            .wait_for(&imported.ready, handle.ready_at)
            .map_err(|e| anyhow!("wait for the captured frame: {e:?}"))?;
        self.context
            .convert(&imported.texture, &self.converter, &self.frames[index])
            .map_err(|e| anyhow!("convert the frame to NV12: {e:?}"))?;
        // One surface is reused: nothing may overwrite it until the conversion runs.
        self.context
            .signal(&imported.release, handle.ready_at)
            .map_err(|e| anyhow!("release the captured frame: {e:?}"))?;
        self.released = handle.ready_at;
        self.last_converted = Some(index);
        self.encode_slot(index, pts_us)
    }

    /// Emit the last picture again, to bound how long a still desktop's sample can run without a new one.
    /// Copies within our own frames rather than reading the producer's surface again: the picture has not changed, and the capture has already been told it may overwrite that surface.
    fn repeat(&mut self, pts_us: u64) -> Result<()> {
        let source = self
            .last_converted
            .ok_or_else(|| anyhow!("a repeat was asked for before any frame was converted"))?;
        let index = self.take_slot();
        self.context
            .copy_frame(&self.frames[source], &self.frames[index])
            .map_err(|e| anyhow!("copy the frame to repeat: {e:?}"))?;
        self.last_converted = Some(index);
        self.encode_slot(index, pts_us)
    }

    /// Whether this frame owes a keyframe, by time rather than by frame count.
    const fn keyframe_due(&self, pts_us: u64) -> bool {
        match self.last_keyframe_us {
            None => true,
            Some(last) => pts_us.saturating_sub(last) >= KEYFRAME_INTERVAL_US,
        }
    }

    /// The next NV12 slot to fill. Round-robin because a hardware encoder is
    /// asynchronous and still holds the previous frame.
    fn take_slot(&mut self) -> usize {
        let index = self.next_frame;
        self.next_frame = (self.next_frame + 1) % self.frames.len();
        index
    }

    /// Encode the picture in `index` and file whatever samples come back.
    fn encode_slot(&mut self, index: usize, pts_us: u64) -> Result<()> {
        if self.keyframe_due(pts_us) {
            self.encoder.request_keyframe();
            self.last_keyframe_us = Some(pts_us);
        }
        let encoded = self
            .encoder
            .encode_texture(&self.frames[index], us_to_mf(pts_us), 0)
            .map_err(|e| anyhow!("encode a frame: {e:?}"))?;
        for sample in encoded {
            // The sample's OWN stamp: an async MFT answers a push with an earlier frame.
            self.hold(&sample.data, sample.is_sync, mf_to_us(sample.timestamp));
        }
        self.drain()
    }

    /// Open the producer's shared objects, reusing them while it hands over the same texture. capturekit reuses one surface for a capture's whole life, so in practice this opens once.
    fn import(&mut self, handle: &GpuHandle) -> Result<()> {
        if self
            .imported
            .as_ref()
            .is_some_and(|open| open.texture_handle == handle.texture)
        {
            return Ok(());
        }
        let texture = self
            .context
            .open_shared_texture(handle.texture, handle.width, handle.height)
            .map_err(|e| anyhow!("open the captured texture: {e:?}"))?;
        let ready = self
            .context
            .open_shared_fence(handle.fence)
            .map_err(|e| anyhow!("open the capture fence: {e:?}"))?;
        let release = self
            .context
            .open_shared_fence(handle.release)
            .map_err(|e| anyhow!("open the release fence: {e:?}"))?;
        self.imported = Some(Imported {
            texture_handle: handle.texture,
            texture,
            ready,
            release,
        });
        Ok(())
    }

    /// Take a newly encoded sample, flushing the one before it now that its
    /// duration is known.
    fn hold(&mut self, annex_b: &[u8], is_sync: bool, pts_us: u64) {
        let converted = annex_b_to_avcc(annex_b);
        if !converted.config.is_empty() {
            self.writer.set_avc_config(converted.config);
            self.have_config = true;
        }
        if converted.sample.is_empty() {
            return;
        }
        if let Some(previous) = self.pending.take() {
            let duration = duration_ticks(previous.pts_us, pts_us);
            self.writer
                .push_sample(&previous.data, duration, previous.is_sync);
            self.samples += 1;
        }
        self.pending = Some(PendingSample {
            data: converted.sample,
            is_sync: is_sync || converted.is_sync,
            pts_us,
        });
    }

    fn drain(&mut self) -> Result<()> {
        if !self.have_config {
            // Nothing can be written before the parameter sets, which take a few frames.
            return Ok(());
        }
        if !self.wrote_init {
            let init = self
                .writer
                .initialization_segment()
                .map_err(|e| anyhow!("build the MP4 header: {e:?}"))?;
            self.out.write_all(&init).context("write the MP4 header")?;
            self.wrote_init = true;
        }
        if let Some(fragment) = self
            .writer
            .fragment()
            .map_err(|e| anyhow!("build an MP4 fragment: {e:?}"))?
        {
            self.out.write_all(&fragment).context("write a fragment")?;
        }
        Ok(())
    }

    /// Flush the encoder and finish the file.
    pub fn finish(mut self) -> Result<u64> {
        let tail = self
            .encoder
            .finish()
            .map_err(|e| anyhow!("flush the encoder: {e:?}"))?;
        for sample in tail {
            self.hold(&sample.data, sample.is_sync, mf_to_us(sample.timestamp));
        }
        if let Some(last) = self.pending.take() {
            // Nothing follows it, so its length is a choice; zero would be unseekable.
            let duration = duration_ticks(0, KEEPALIVE.as_micros() as u64);
            self.writer.push_sample(&last.data, duration, last.is_sync);
            self.samples += 1;
        }
        self.drain()?;
        if !self.wrote_init {
            return Err(anyhow!(
                "the encoder produced no parameter sets, so the recording has no                  header and nothing can play it"
            ));
        }
        self.out.flush().context("flush the recording")?;
        Ok(self.samples)
    }
}

/// Target bitrate for screen content at this size and rate.
/// 0.08 bits per pixel per frame keeps 4K60 text legible at ~40 Mbit/s. Clamped so a tiny window still gets a usable floor and a huge one cannot fill a disk.
pub const fn target_bitrate(width: u32, height: u32, fps: u32) -> u32 {
    let fps = if fps == 0 { 1 } else { fps };
    let pixels = width as u64 * height as u64 * fps as u64;
    let bits = pixels * 8 / 100;
    if bits < 4_000_000 {
        4_000_000
    } else if bits > 120_000_000 {
        120_000_000
    } else {
        bits as u32
    }
}

/// Whether this machine can encode textures without a readback.
/// Probed before a recording commits to the native path, because the fallback is a whole different writer and switching mid-recording is not a thing.
pub fn available() -> bool {
    select_preferred(&enumerate_encoders(), VideoCodec::H264).is_some()
}

/// The capture loop's sink for the zero-copy path, opening the encoder on FIRST FRAME, on the capture thread.
/// The D3D device, video processor and Media Foundation transform are then all created and used by one thread, so none has to cross a boundary.
pub struct NativeSink {
    path: std::path::PathBuf,
    fps: u32,
    recorder: Option<NativeRecorder>,
    /// Fixed by the first frame: an encoder cannot change size mid-stream.
    size: Option<(u32, u32)>,
    /// Counts what actually reached the file, so a native recording reports its
    /// throughput the way the FFmpeg one does.
    stats: PipelineStats,
}

impl NativeSink {
    pub const fn new(path: std::path::PathBuf, fps: u32, stats: PipelineStats) -> Self {
        Self {
            path,
            fps,
            recorder: None,
            size: None,
            stats,
        }
    }
}

// SAFETY: built with `recorder` None, before the one thread that makes and drops every COM object in it.
unsafe impl Send for NativeSink {}

impl FrameSink for NativeSink {
    fn accept(
        &mut self,
        frame: &CapturedFrame,
        pts_us: u64,
        width: u32,
        height: u32,
    ) -> Result<()> {
        let CapturedFrame::Gpu(handle) = frame else {
            return Err(anyhow!(
                "the native encoder was given a frame in host memory, which is \
                 the copy it exists to avoid"
            ));
        };
        match self.size {
            Some(size) if size != (width, height) => {
                return Err(anyhow!(
                    "the capture changed from {}x{} to {width}x{height} mid-recording",
                    size.0,
                    size.1
                ));
            }
            Some(_) => {}
            None => {
                let bitrate = target_bitrate(width, height, self.fps);
                self.recorder = Some(NativeRecorder::open(
                    &self.path, width, height, self.fps, bitrate,
                )?);
                self.size = Some((width, height));
            }
        }
        self.recorder
            .as_mut()
            .expect("the first frame opens the recorder")
            .push(handle, pts_us)?;
        self.stats.encoded_frames.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    fn finish(&mut self) -> Result<()> {
        // Taken, not borrowed: a second call must not retry on a closed file.
        let Some(recorder) = self.recorder.take() else {
            return Ok(());
        };
        let samples = recorder.finish()?;
        log::info!(
            "native recorder wrote {samples} samples to {}",
            self.path.display()
        );
        Ok(())
    }
}

/// Microseconds to Media Foundation's 100 ns units.
const fn us_to_mf(us: u64) -> i64 {
    (us as i64).saturating_mul(MF_TICKS_PER_SEC / 1_000_000)
}

/// Media Foundation's 100 ns units back to microseconds.
const fn mf_to_us(ticks: i64) -> u64 {
    if ticks <= 0 {
        0
    } else {
        (ticks / (MF_TICKS_PER_SEC / 1_000_000)) as u64
    }
}

/// Sample duration in `TIMESCALE` ticks, from the gap between two frames.
/// Rounds to nearest so a 60fps stream alternates 1500/1500 rather than truncating every sample and losing a frame's worth of time per minute.
pub const fn duration_ticks(from_us: u64, to_us: u64) -> u32 {
    let delta = to_us.saturating_sub(from_us);
    let ticks = (delta * TIMESCALE as u64 + 500_000) / 1_000_000;
    if ticks > u32::MAX as u64 {
        u32::MAX
    } else {
        ticks as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sixty_hertz_gap_is_one_sixtieth_of_the_timescale() {
        assert_eq!(duration_ticks(0, 16_667), 1500);
    }

    #[test]
    fn a_long_still_gap_keeps_its_real_length() {
        assert_eq!(duration_ticks(0, 500_000), 45_000);
    }

    /// Truncating instead of rounding loses a tick per frame, which at 60fps is
    /// a frame of drift every 25 seconds.
    #[test]
    fn a_gap_that_does_not_divide_evenly_rounds_rather_than_truncates() {
        assert_eq!(duration_ticks(0, 16_666), 1500);
        assert_eq!(duration_ticks(0, 8_333), 750);
    }

    /// The encoder echoes back the timestamp it was given, so the two must be
    /// exact inverses or every sample lands slightly off its own frame.
    #[test]
    fn a_timestamp_survives_the_round_trip_through_media_foundation_units() {
        for us in [0u64, 1, 16_667, 500_000, 3_600_000_000] {
            assert_eq!(mf_to_us(us_to_mf(us)), us);
        }
    }

    /// A flushed sample with no timestamp reads as zero, not as a huge negative
    /// microsecond count that would swallow the whole track.
    #[test]
    fn an_absent_timestamp_reads_as_the_start_not_as_a_negative() {
        assert_eq!(mf_to_us(-1), 0);
    }

    #[test]
    fn frames_out_of_order_cannot_produce_a_negative_duration() {
        assert_eq!(duration_ticks(1_000, 0), 0);
    }
}

/// The writer end to end: capture texture in, MP4 out, probed back. Only reading the file proves the durations reached the container rather than being flattened.
/// The oracle is ffprobe, not `recast_mux::Mp4Reader`, which parses `moov` only and reports no samples at all for a fragmented file.
#[cfg(test)]
mod live_tests {
    use super::*;
    use capturekit::{capturer, DisplayId, Pacing, Target};

    /// Deliberately uneven gaps, in microseconds. A fixed-rate writer would give
    /// all of them the same duration.
    const GAPS_US: [u64; 7] = [16_667, 16_667, 250_000, 16_667, 33_333, 500_000, 16_667];

    const TIMEOUT: std::time::Duration = std::time::Duration::from_millis(500);

    fn primary() -> Option<DisplayId> {
        let displays = capturekit::displays().ok()?;
        displays
            .iter()
            .find(|d| d.is_primary)
            .or(displays.first())
            .map(|d| d.id)
    }

    /// Presentation time of every video packet, in seconds.
    fn packet_times(probe: &std::path::Path, file: &std::path::Path) -> Vec<f64> {
        let out = std::process::Command::new(probe)
            .args([
                "-v",
                "error",
                "-select_streams",
                "v:0",
                "-show_entries",
                "packet=pts_time",
                "-of",
                "csv=p=0",
            ])
            .arg(file)
            .stdin(std::process::Stdio::null())
            .output()
            .expect("ffprobe runs");
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .filter_map(|line| line.trim().trim_end_matches(',').parse().ok())
            .collect()
    }

    /// Needs a desktop that is changing: under `Passthrough` an idle display
    /// produces no frames at all, which is not what this measures.
    #[test]
    #[ignore = "live: needs a real display with moving content and an H.264 encoder"]
    fn the_gaps_between_frames_reach_the_file_as_sample_durations() {
        let Some(display) = primary() else { return };
        // The bundled resolver answers a bare name outside an installed app.
        let probe = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("binaries")
            .join("ffprobe-x86_64-pc-windows-msvc.exe");
        assert!(
            probe.exists(),
            "no ffprobe at {} to check the file with",
            probe.display()
        );
        let dir = std::env::temp_dir().join("recast-native-vfr");
        std::fs::create_dir_all(&dir).expect("a scratch directory");
        let path = dir.join("vfr.mp4");

        let mut capture = capturer(Target::Display(display))
            .gpu_handles(true)
            .pacing(Pacing::Passthrough)
            .build()
            .expect("the display opens");
        let described = capture.describe();
        let bitrate = target_bitrate(described.width, described.height, 60);
        let mut recorder =
            NativeRecorder::open(&path, described.width, described.height, 60, bitrate)
                .expect("the native encoder opens");

        // Real pixels, scripted stamps: the durations tested are not the desktop's.
        let mut pts_us = 0u64;
        let mut wanted = vec![0.0f64];
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
        for gap in GAPS_US {
            let frame = loop {
                assert!(
                    std::time::Instant::now() < deadline,
                    "the display stopped producing frames after {} pushes",
                    wanted.len()
                );
                if let Ok(frame) = capture.next_frame(TIMEOUT) {
                    break frame;
                }
            };
            let handle = *frame.gpu_handle().expect("gpu handles were asked for");
            drop(frame);
            recorder.push(&handle, pts_us).expect("the frame encodes");
            pts_us += gap;
            wanted.push(pts_us as f64 / 1_000_000.0);
        }
        let written = recorder.finish().expect("the file closes");

        let times = packet_times(&probe, &path);
        assert!(
            times.len() >= GAPS_US.len(),
            "the writer reported {written} samples, ffprobe found {} in {}",
            times.len(),
            path.display()
        );
        // The last stamp's duration is invented by `finish`, so it is not compared.
        for (index, expected) in wanted.iter().take(GAPS_US.len()).enumerate() {
            let actual = times[index];
            assert!(
                (actual - expected).abs() < 0.002,
                "packet {index} is at {actual}s, wanted {expected}s (all: {times:?})"
            );
        }
        let distinct = times.len();
        assert!(distinct > 1, "one packet is not a rate at all");

        let _ = std::fs::remove_file(&path);
    }
}
