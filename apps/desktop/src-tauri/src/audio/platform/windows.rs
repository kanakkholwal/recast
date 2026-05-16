use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};

use anyhow::{anyhow, Context, Result};

use crate::audio::wav::WavWriter;
use crate::audio::{AudioCaptureConfig, MicrophoneCaptureConfig};

pub struct PlatformAudioSession {
    stop_flag: Arc<AtomicBool>,
    thread_handle: JoinHandle<Result<PathBuf>>,
}

impl PlatformAudioSession {
    pub fn start(config: AudioCaptureConfig) -> Result<Self> {
        let stop_flag = Arc::new(AtomicBool::new(false));
        let flag_clone = stop_flag.clone();
        let output_path = config.output_path.clone();

        let thread_handle = thread::Builder::new()
            .name("recast-audio".into())
            .spawn(move || capture_loopback_thread(config, flag_clone))
            .context("failed to spawn audio capture thread")?;

        log::info!("audio capture started, output: {}", output_path.display());

        Ok(Self {
            stop_flag,
            thread_handle,
        })
    }

    pub fn stop(self) -> Result<PathBuf> {
        self.stop_flag.store(true, Ordering::Release);
        self.thread_handle
            .join()
            .map_err(|_| anyhow!("audio capture thread panicked"))?
    }
}

pub struct PlatformMicrophoneSession {
    stop_flag: Arc<AtomicBool>,
    thread_handle: JoinHandle<Result<PathBuf>>,
}

impl PlatformMicrophoneSession {
    pub fn start(config: MicrophoneCaptureConfig) -> Result<Self> {
        let stop_flag = Arc::new(AtomicBool::new(false));
        let flag_clone = stop_flag.clone();
        let output_path = config.output_path.clone();

        let thread_handle = thread::Builder::new()
            .name("recast-microphone".into())
            .spawn(move || capture_microphone_thread(config, flag_clone))
            .context("failed to spawn microphone capture thread")?;

        log::info!(
            "microphone capture started, output: {}",
            output_path.display()
        );

        Ok(Self {
            stop_flag,
            thread_handle,
        })
    }

    pub fn stop(self) -> Result<PathBuf> {
        self.stop_flag.store(true, Ordering::Release);
        self.thread_handle
            .join()
            .map_err(|_| anyhow!("microphone capture thread panicked"))?
    }
}

/// WASAPI microphone capture running on a dedicated thread.
/// Captures from a specific microphone device and writes PCM data as WAV.
fn capture_microphone_thread(
    config: MicrophoneCaptureConfig,
    stop_flag: Arc<AtomicBool>,
) -> Result<PathBuf> {
    use windows::core::HSTRING;
    use windows::Win32::Media::Audio::*;
    use windows::Win32::System::Com::*;

    unsafe {
        CoInitializeEx(None, COINIT_MULTITHREADED)
            .ok()
            .context("COM initialization failed")?;
    }

    struct ComGuard;
    impl Drop for ComGuard {
        fn drop(&mut self) {
            unsafe {
                CoUninitialize();
            }
        }
    }
    let _com_guard = ComGuard;

    let enumerator: IMMDeviceEnumerator = unsafe {
        CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)
            .context("failed to create MMDeviceEnumerator")?
    };

    // Open specific device by ID, or fall back to the default capture endpoint.
    let device = if let Some(ref id) = config.device_id {
        let hid = HSTRING::from(id.as_str());
        unsafe {
            enumerator
                .GetDevice(&hid)
                .with_context(|| format!("microphone device not found: {id}"))?
        }
    } else {
        unsafe {
            enumerator
                .GetDefaultAudioEndpoint(eCapture, eConsole)
                .context("no default microphone device found")?
        }
    };

    let audio_client: IAudioClient = unsafe {
        device
            .Activate(CLSCTX_ALL, None)
            .context("failed to activate IAudioClient for microphone")?
    };

    let mix_format_ptr = unsafe {
        audio_client
            .GetMixFormat()
            .context("failed to get microphone mix format")?
    };

    let mix_format = unsafe { &*mix_format_ptr };
    let sample_rate = mix_format.nSamplesPerSec;
    let channels = mix_format.nChannels;
    let bits_per_sample = mix_format.wBitsPerSample;
    let block_align = mix_format.nBlockAlign;

    log::info!(
        "WASAPI microphone format: {}Hz, {} channels, {} bits, block_align={}",
        sample_rate,
        channels,
        bits_per_sample,
        block_align
    );

    // Initialize in shared mode WITHOUT loopback flag — this captures from the mic input.
    let buffer_duration_100ns: i64 = 10_000_000; // 1 second buffer
    unsafe {
        audio_client
            .Initialize(
                AUDCLNT_SHAREMODE_SHARED,
                0u32, // No special flags — standard capture mode
                buffer_duration_100ns,
                0,
                mix_format_ptr,
                None,
            )
            .context("failed to initialize audio client for microphone capture")?;
    }

    let capture_client: IAudioCaptureClient = unsafe {
        audio_client
            .GetService()
            .context("failed to get IAudioCaptureClient for microphone")?
    };

    let wav_bits = if bits_per_sample == 32 { 32u16 } else { 16u16 };
    let mut wav_writer = WavWriter::new(&config.output_path, sample_rate, channels, wav_bits)?;

    unsafe {
        audio_client
            .Start()
            .context("failed to start microphone capture")?;
    }

    // Capture loop — same pattern as loopback but reading from mic input.
    while !stop_flag.load(Ordering::Acquire) {
        thread::sleep(std::time::Duration::from_millis(10));

        loop {
            let packet_length = unsafe { capture_client.GetNextPacketSize().unwrap_or(0) };
            if packet_length == 0 {
                break;
            }

            let mut buffer_ptr = std::ptr::null_mut();
            let mut frames_available = 0u32;
            let mut flags = 0u32;

            let hr = unsafe {
                capture_client.GetBuffer(
                    &mut buffer_ptr,
                    &mut frames_available,
                    &mut flags,
                    None,
                    None,
                )
            };

            if hr.is_err() {
                break;
            }

            // Drain the device every iteration, but only write samples when
            // the recording isn't paused — so the WAV has no paused span.
            if frames_available > 0 && !config.pause_flag.load(Ordering::Acquire) {
                let byte_count = frames_available as usize * block_align as usize;

                if flags & AUDCLNT_BUFFERFLAGS_SILENT.0 as u32 != 0 {
                    let silence = vec![0u8; byte_count];
                    let _ = wav_writer.write_samples(&silence);
                } else {
                    let data = unsafe { std::slice::from_raw_parts(buffer_ptr, byte_count) };
                    let _ = wav_writer.write_samples(data);
                }
            }

            unsafe {
                let _ = capture_client.ReleaseBuffer(frames_available);
            }
        }
    }

    unsafe {
        let _ = audio_client.Stop();
    }

    // Drain remaining packets.
    loop {
        let packet_length = unsafe { capture_client.GetNextPacketSize().unwrap_or(0) };
        if packet_length == 0 {
            break;
        }

        let mut buffer_ptr = std::ptr::null_mut();
        let mut frames_available = 0u32;
        let mut flags = 0u32;

        let hr = unsafe {
            capture_client.GetBuffer(
                &mut buffer_ptr,
                &mut frames_available,
                &mut flags,
                None,
                None,
            )
        };

        if hr.is_err() {
            break;
        }

        if frames_available > 0 {
            let byte_count = frames_available as usize * block_align as usize;
            if flags & AUDCLNT_BUFFERFLAGS_SILENT.0 as u32 != 0 {
                let silence = vec![0u8; byte_count];
                let _ = wav_writer.write_samples(&silence);
            } else {
                let data = unsafe { std::slice::from_raw_parts(buffer_ptr, byte_count) };
                let _ = wav_writer.write_samples(data);
            }
        }

        unsafe {
            let _ = capture_client.ReleaseBuffer(frames_available);
        }
    }

    unsafe {
        CoTaskMemFree(Some(mix_format_ptr as *const _ as *const _));
    }

    wav_writer.finish()?;
    log::info!(
        "microphone capture finished: {}",
        config.output_path.display()
    );
    Ok(config.output_path)
}

/// WASAPI loopback capture running on a dedicated thread.
/// Captures system audio output (what-you-hear) and writes it as WAV.
fn capture_loopback_thread(
    config: AudioCaptureConfig,
    stop_flag: Arc<AtomicBool>,
) -> Result<PathBuf> {
    use windows::Win32::Media::Audio::*;
    use windows::Win32::System::Com::*;

    // Initialize COM for this thread (WASAPI requires it).
    unsafe {
        CoInitializeEx(None, COINIT_MULTITHREADED)
            .ok()
            .context("COM initialization failed")?;
    }

    // Ensure COM is uninitialized when we leave.
    struct ComGuard;
    impl Drop for ComGuard {
        fn drop(&mut self) {
            unsafe {
                CoUninitialize();
            }
        }
    }
    let _com_guard = ComGuard;

    // Get the default audio render endpoint (speakers/headphones).
    let enumerator: IMMDeviceEnumerator = unsafe {
        CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)
            .context("failed to create MMDeviceEnumerator")?
    };

    let device = unsafe {
        enumerator
            .GetDefaultAudioEndpoint(eRender, eConsole)
            .context("no default audio render device found")?
    };

    // Activate the audio client on the device.
    let audio_client: IAudioClient = unsafe {
        device
            .Activate(CLSCTX_ALL, None)
            .context("failed to activate IAudioClient")?
    };

    // Get the mix format (native format of the audio engine).
    let mix_format_ptr = unsafe {
        audio_client
            .GetMixFormat()
            .context("failed to get mix format")?
    };

    let mix_format = unsafe { &*mix_format_ptr };
    let sample_rate = mix_format.nSamplesPerSec;
    let channels = mix_format.nChannels;
    let bits_per_sample = mix_format.wBitsPerSample;
    let block_align = mix_format.nBlockAlign;

    log::info!(
        "WASAPI loopback format: {}Hz, {} channels, {} bits, block_align={}",
        sample_rate,
        channels,
        bits_per_sample,
        block_align
    );

    // Initialize the audio client in loopback mode.
    // AUDCLNT_STREAMFLAGS_LOOPBACK captures the output mix.
    let buffer_duration_100ns: i64 = 10_000_000; // 1 second buffer in 100-ns units
    unsafe {
        audio_client
            .Initialize(
                AUDCLNT_SHAREMODE_SHARED,
                AUDCLNT_STREAMFLAGS_LOOPBACK,
                buffer_duration_100ns,
                0,
                mix_format_ptr,
                None,
            )
            .context("failed to initialize audio client in loopback mode")?;
    }

    // Get the capture client interface.
    let capture_client: IAudioCaptureClient = unsafe {
        audio_client
            .GetService()
            .context("failed to get IAudioCaptureClient")?
    };

    // Determine output WAV format.
    // WASAPI loopback typically gives us 32-bit float. We'll write it as-is.
    let wav_bits = if bits_per_sample == 32 { 32u16 } else { 16u16 };
    let mut wav_writer = WavWriter::new(&config.output_path, sample_rate, channels, wav_bits)?;

    // Start capturing.
    unsafe {
        audio_client
            .Start()
            .context("failed to start audio capture")?;
    }

    // Capture loop: pull packets from the capture buffer until stopped.
    while !stop_flag.load(Ordering::Acquire) {
        // Sleep briefly to let the buffer fill. 10ms is a good balance
        // between latency and CPU usage for loopback capture.
        thread::sleep(std::time::Duration::from_millis(10));

        loop {
            let packet_length = unsafe { capture_client.GetNextPacketSize().unwrap_or(0) };

            if packet_length == 0 {
                break;
            }

            let mut buffer_ptr = std::ptr::null_mut();
            let mut frames_available = 0u32;
            let mut flags = 0u32;

            let hr = unsafe {
                capture_client.GetBuffer(
                    &mut buffer_ptr,
                    &mut frames_available,
                    &mut flags,
                    None,
                    None,
                )
            };

            if hr.is_err() {
                break;
            }

            // Drain the device every iteration, but only write samples when
            // the recording isn't paused — so the WAV has no paused span.
            if frames_available > 0 && !config.pause_flag.load(Ordering::Acquire) {
                let byte_count = frames_available as usize * block_align as usize;

                if flags & AUDCLNT_BUFFERFLAGS_SILENT.0 as u32 != 0 {
                    // Device reported silence — write zeros.
                    let silence = vec![0u8; byte_count];
                    let _ = wav_writer.write_samples(&silence);
                } else {
                    // Copy the captured audio data.
                    let data = unsafe { std::slice::from_raw_parts(buffer_ptr, byte_count) };
                    let _ = wav_writer.write_samples(data);
                }
            }

            unsafe {
                let _ = capture_client.ReleaseBuffer(frames_available);
            }
        }
    }

    // Stop and finalize.
    unsafe {
        let _ = audio_client.Stop();
    }

    // Drain any remaining packets after stopping.
    loop {
        let packet_length = unsafe { capture_client.GetNextPacketSize().unwrap_or(0) };
        if packet_length == 0 {
            break;
        }

        let mut buffer_ptr = std::ptr::null_mut();
        let mut frames_available = 0u32;
        let mut flags = 0u32;

        let hr = unsafe {
            capture_client.GetBuffer(
                &mut buffer_ptr,
                &mut frames_available,
                &mut flags,
                None,
                None,
            )
        };

        if hr.is_err() {
            break;
        }

        if frames_available > 0 {
            let byte_count = frames_available as usize * block_align as usize;
            if flags & AUDCLNT_BUFFERFLAGS_SILENT.0 as u32 != 0 {
                let silence = vec![0u8; byte_count];
                let _ = wav_writer.write_samples(&silence);
            } else {
                let data = unsafe { std::slice::from_raw_parts(buffer_ptr, byte_count) };
                let _ = wav_writer.write_samples(data);
            }
        }

        unsafe {
            let _ = capture_client.ReleaseBuffer(frames_available);
        }
    }

    // Free the format memory allocated by WASAPI.
    unsafe {
        CoTaskMemFree(Some(mix_format_ptr as *const _ as *const _));
    }

    wav_writer.finish()?;
    log::info!("audio capture finished: {}", config.output_path.display());
    Ok(config.output_path)
}
