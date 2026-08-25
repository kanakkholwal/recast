use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use windows::core::HSTRING;
use windows::Win32::Media::Audio::*;
use windows::Win32::System::Com::*;

use crate::audio::wav::{measured_sample_rate, SampleFormat, WavFormat, WavWriter};
use crate::audio::{AudioCaptureConfig, MicrophoneCaptureConfig};
use crate::recording::clock::TrackStart;

/// `WAVEFORMATEX::wFormatTag` for a format described by `WAVEFORMATEXTENSIBLE`.
const WAVE_FORMAT_EXTENSIBLE: u16 = 0xFFFE;
const WAVE_FORMAT_IEEE_FLOAT: u16 = 3;
/// Bytes of `WAVEFORMATEXTENSIBLE` past `WAVEFORMATEX`; below this the extended
/// fields (`SubFormat` included) are not actually present.
const EXTENSIBLE_EXTRA_BYTES: u16 = 22;

/// Device poll interval. Long enough that the thread is nearly idle, short
/// enough that the 1 s WASAPI buffer cannot overrun.
const POLL_INTERVAL: Duration = Duration::from_millis(10);
const BUFFER_DURATION_100NS: i64 = 10_000_000;

/// COM must be initialised per thread for WASAPI, and uninitialised on the way
/// out even when the capture returns early.
struct ComGuard;

impl ComGuard {
    fn enter() -> Result<Self> {
        unsafe {
            CoInitializeEx(None, COINIT_MULTITHREADED)
                .ok()
                .context("COM initialization failed")?;
        }
        Ok(Self)
    }
}

impl Drop for ComGuard {
    fn drop(&mut self) {
        unsafe { CoUninitialize() };
    }
}

/// Owns the `WAVEFORMATEX` WASAPI allocated for us. `GetMixFormat` returns
/// CoTaskMem the caller must free; this used to be freed only on the success
/// path, so every early return leaked it for the process lifetime.
struct MixFormat(*mut WAVEFORMATEX);

impl MixFormat {
    fn of(client: &IAudioClient) -> Result<Self> {
        let ptr = unsafe { client.GetMixFormat().context("failed to get mix format")? };
        if ptr.is_null() {
            return Err(anyhow!("device reported a null mix format"));
        }
        Ok(Self(ptr))
    }

    fn as_ptr(&self) -> *const WAVEFORMATEX {
        self.0
    }

    fn raw(&self) -> &WAVEFORMATEX {
        // Non-null by construction and owned for `self`'s lifetime.
        unsafe { &*self.0 }
    }

    /// The byte layout the device will actually deliver.
    ///
    /// The depth comes from `nBlockAlign`, not `wBitsPerSample`: a 24-bit
    /// endpoint delivers 24-in-32 containers, and describing those as 24-bit
    /// makes every consumer read 3-byte samples out of a 4-byte stride, which
    /// plays back fast and distorted.
    fn wav_format(&self) -> WavFormat {
        let raw = self.raw();
        let channels = raw.nChannels.max(1);
        let container_bits = if raw.nBlockAlign > 0 {
            (raw.nBlockAlign / channels) * 8
        } else {
            raw.wBitsPerSample
        };
        WavFormat::new(
            raw.nSamplesPerSec,
            channels,
            container_bits,
            self.sample_format(),
        )
    }

    /// Integer or float, read from the format rather than guessed. WASAPI
    /// shared mode commonly reports `WAVE_FORMAT_EXTENSIBLE`, whose real type
    /// lives in `SubFormat`; assuming "32 bits means float" mislabels 32-bit
    /// integer endpoints, which is heard as full-scale noise.
    fn sample_format(&self) -> SampleFormat {
        let raw = self.raw();
        let tag = if raw.wFormatTag == WAVE_FORMAT_EXTENSIBLE {
            if raw.cbSize < EXTENSIBLE_EXTRA_BYTES {
                // Claims EXTENSIBLE without carrying the extended fields.
                return SampleFormat::Int;
            }
            // The KSDATAFORMAT_SUBTYPE_* GUIDs are the format tag widened into
            // a GUID, so `data1` IS the tag.
            let ext = unsafe { &*(self.0 as *const WAVEFORMATEXTENSIBLE) };
            ext.SubFormat.data1 as u16
        } else {
            raw.wFormatTag
        };
        if tag == WAVE_FORMAT_IEEE_FLOAT {
            SampleFormat::Float
        } else {
            SampleFormat::Int
        }
    }
}

impl Drop for MixFormat {
    fn drop(&mut self) {
        unsafe { CoTaskMemFree(Some(self.0 as *const _)) };
    }
}

// The raw pointer is what blocks the auto-impl; `MixFormat` never leaves the
// thread that created it.
unsafe impl Send for MixFormat {}

/// Which endpoint to record, and how.
enum Endpoint {
    /// The default render device, captured as loopback (what you hear).
    SystemLoopback,
    /// A capture device by id, or the default when `None`.
    Microphone(Option<String>),
}

impl Endpoint {
    fn open(&self, enumerator: &IMMDeviceEnumerator) -> Result<IMMDevice> {
        unsafe {
            match self {
                Self::SystemLoopback => enumerator
                    .GetDefaultAudioEndpoint(eRender, eConsole)
                    .context("no default audio render device found"),
                Self::Microphone(Some(id)) => enumerator
                    .GetDevice(&HSTRING::from(id.as_str()))
                    .with_context(|| format!("microphone device not found: {id}")),
                Self::Microphone(None) => enumerator
                    .GetDefaultAudioEndpoint(eCapture, eConsole)
                    .context("no default microphone device found"),
            }
        }
    }

    fn stream_flags(&self) -> u32 {
        match self {
            Self::SystemLoopback => AUDCLNT_STREAMFLAGS_LOOPBACK,
            Self::Microphone(_) => 0,
        }
    }

    fn label(&self) -> &'static str {
        match self {
            Self::SystemLoopback => "system-audio loopback",
            Self::Microphone(_) => "microphone",
        }
    }

    fn thread_name(&self) -> &'static str {
        match self {
            Self::SystemLoopback => "recast-audio",
            Self::Microphone(_) => "recast-microphone",
        }
    }
}

/// A running WASAPI capture thread writing PCM to a WAV.
struct CaptureThread {
    stop_flag: Arc<AtomicBool>,
    /// `Option` so `Drop` can take it; see the `Drop` impl.
    handle: Option<JoinHandle<Result<PathBuf>>>,
}

impl CaptureThread {
    fn spawn(
        endpoint: Endpoint,
        output_path: PathBuf,
        pause_flag: Arc<AtomicBool>,
        start: TrackStart,
    ) -> Result<Self> {
        let stop_flag = Arc::new(AtomicBool::new(false));
        let stop_for_thread = stop_flag.clone();
        let path_for_thread = output_path.clone();
        let (label, thread_name) = (endpoint.label(), endpoint.thread_name());
        let handle = thread::Builder::new()
            .name(thread_name.to_string())
            .spawn(move || {
                capture(
                    endpoint,
                    path_for_thread,
                    pause_flag,
                    stop_for_thread,
                    start,
                )
            })
            .with_context(|| format!("failed to spawn {label} capture thread"))?;

        log::info!("{label} capture started, output: {}", output_path.display());
        Ok(Self {
            stop_flag,
            handle: Some(handle),
        })
    }

    fn stop(mut self) -> Result<PathBuf> {
        self.stop_flag.store(true, Ordering::Release);
        let handle = self
            .handle
            .take()
            .ok_or_else(|| anyhow!("audio session already stopped"))?;
        handle
            .join()
            .map_err(|_| anyhow!("audio capture thread panicked"))?
    }
}

impl Drop for CaptureThread {
    fn drop(&mut self) {
        // Only fires when the session is dropped WITHOUT a clean `stop()` — a
        // panic or early return between start and the caller's stop. Without
        // this the WASAPI thread loops forever holding IAudioClient.
        if let Some(handle) = self.handle.take() {
            self.stop_flag.store(true, Ordering::Release);
            let _ = handle.join();
        }
    }
}

/// Drain every packet the device currently holds into `writer`.
///
/// `writing` gates only the WAV write, never the drain: the device buffer has
/// to be released even while paused, or WASAPI overruns and the audio after a
/// resume comes back corrupt.
fn drain_packets(
    capture_client: &IAudioCaptureClient,
    writer: &mut WavWriter,
    format: WavFormat,
    writing: bool,
    start: &TrackStart,
) -> u64 {
    let block_align = format.block_align() as usize;
    let mut written = 0u64;
    loop {
        let pending = unsafe { capture_client.GetNextPacketSize().unwrap_or(0) };
        if pending == 0 {
            return written;
        }

        let mut buffer_ptr = std::ptr::null_mut();
        let mut frames = 0u32;
        let mut flags = 0u32;
        let acquired = unsafe {
            capture_client.GetBuffer(&mut buffer_ptr, &mut frames, &mut flags, None, None)
        };
        if acquired.is_err() {
            return written;
        }

        if frames > 0 && writing {
            start.mark();
            let byte_count = frames as usize * block_align;
            let silent = flags & AUDCLNT_BUFFERFLAGS_SILENT.0 as u32 != 0;
            let wrote = if silent || buffer_ptr.is_null() {
                writer.write_samples(&vec![0u8; byte_count])
            } else {
                let data = unsafe { std::slice::from_raw_parts(buffer_ptr, byte_count) };
                writer.write_samples(data)
            };
            match wrote {
                Ok(()) => written += frames as u64,
                Err(e) => log::warn!("WAV write failed (dropping packet): {e}"),
            }
        }

        unsafe {
            let _ = capture_client.ReleaseBuffer(frames);
        }
    }
}

/// The one WASAPI capture loop. Loopback and microphone differ only in which
/// endpoint they open and a single stream flag, so they share this body; they
/// used to be near-identical copies that had to be fixed twice.
fn capture(
    endpoint: Endpoint,
    output_path: PathBuf,
    pause_flag: Arc<AtomicBool>,
    stop_flag: Arc<AtomicBool>,
    start: TrackStart,
) -> Result<PathBuf> {
    let label = endpoint.label();
    let _com = ComGuard::enter()?;

    let enumerator: IMMDeviceEnumerator = unsafe {
        CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)
            .context("failed to create MMDeviceEnumerator")?
    };
    let device = endpoint.open(&enumerator)?;
    let audio_client: IAudioClient = unsafe {
        device
            .Activate(CLSCTX_ALL, None)
            .with_context(|| format!("failed to activate IAudioClient for {label}"))?
    };

    let mix = MixFormat::of(&audio_client)?;
    let format = mix.wav_format();
    log::info!(
        "WASAPI {label} format: {}Hz, {} ch, {} bits {:?}, block_align={}",
        format.sample_rate,
        format.channels,
        format.bits_per_sample,
        format.format,
        format.block_align()
    );

    unsafe {
        audio_client
            .Initialize(
                AUDCLNT_SHAREMODE_SHARED,
                endpoint.stream_flags(),
                BUFFER_DURATION_100NS,
                0,
                mix.as_ptr(),
                None,
            )
            .with_context(|| format!("failed to initialize audio client for {label}"))?;
    }

    let capture_client: IAudioCaptureClient = unsafe {
        audio_client
            .GetService()
            .with_context(|| format!("failed to get IAudioCaptureClient for {label}"))?
    };

    let mut writer = WavWriter::new(&output_path, format)?;
    unsafe {
        audio_client
            .Start()
            .with_context(|| format!("failed to start {label} capture"))?;
    }

    // Wall-clock span the written samples actually cover, so the delivered rate
    // can be compared against the declared one. Paused spans are excluded
    // because no samples are written across them.
    let mut first_write: Option<Instant> = None;
    let mut last_write: Option<Instant> = None;
    let mut paused_total = Duration::ZERO;
    let mut paused_since: Option<Instant> = None;

    while !stop_flag.load(Ordering::Acquire) {
        thread::sleep(POLL_INTERVAL);
        let writing = !pause_flag.load(Ordering::Acquire);
        if writing {
            if let Some(since) = paused_since.take() {
                paused_total += since.elapsed();
            }
        } else if paused_since.is_none() {
            paused_since = Some(Instant::now());
        }
        if drain_packets(&capture_client, &mut writer, format, writing, &start) > 0 {
            let now = Instant::now();
            first_write.get_or_insert(now);
            last_write = Some(now);
        }
    }

    unsafe {
        let _ = audio_client.Stop();
    }
    // Whatever the device buffered between the last poll and the stop.
    if drain_packets(&capture_client, &mut writer, format, true, &start) > 0 {
        last_write = Some(Instant::now());
    }
    if let Some(since) = paused_since.take() {
        paused_total += since.elapsed();
    }

    let frames = writer.frames_written();
    // Re-declare the rate when the device's own clock ran off the nominal one.
    // The frame pacer holds the picture to wall clock exactly, so an uncorrected
    // audio crystal drifts against it for the whole take instead of cancelling.
    if let (Some(first), Some(last)) = (first_write, last_write) {
        let span = last
            .saturating_duration_since(first)
            .saturating_sub(paused_total);
        if let Some(rate) = measured_sample_rate(frames, span, format.sample_rate) {
            log::warn!(
                "{label} device clock drift: declared {}Hz, delivered {rate}Hz                  ({frames} frames over {:.3}s) — re-declaring so the track stays                  locked to the picture",
                format.sample_rate,
                span.as_secs_f64()
            );
            writer.set_sample_rate(rate);
        }
    }
    let final_rate = writer.sample_rate();
    writer.finish()?;
    log::info!(
        "{label} capture finished: {} ({:.2}s, {frames} frames @ {final_rate}Hz)",
        output_path.display(),
        frames as f64 / final_rate as f64
    );
    Ok(output_path)
}

pub struct PlatformAudioSession(CaptureThread);

impl PlatformAudioSession {
    pub fn start(config: AudioCaptureConfig) -> Result<Self> {
        CaptureThread::spawn(
            Endpoint::SystemLoopback,
            config.output_path,
            config.pause_flag,
            config.start,
        )
        .map(Self)
    }

    /// WASAPI loopback always captures the real output mix, so this is always
    /// true on Windows. (Silence in the WAV means nothing was playing, which is
    /// still a genuine system-audio track.)
    pub fn is_capturing(&self) -> bool {
        true
    }

    pub fn stop(self) -> Result<PathBuf> {
        self.0.stop()
    }
}

pub struct PlatformMicrophoneSession(CaptureThread);

impl PlatformMicrophoneSession {
    pub fn start(config: MicrophoneCaptureConfig) -> Result<Self> {
        CaptureThread::spawn(
            Endpoint::Microphone(config.device_id),
            config.output_path,
            config.pause_flag,
            config.start,
        )
        .map(Self)
    }

    pub fn stop(self) -> Result<PathBuf> {
        self.0.stop()
    }
}
