use core::time::Duration;

use capturekit_core::{
    AudioDesc, AudioDevice, AudioDeviceId, AudioDirection, AudioFormat, AudioTimeline,
    CaptureError, LostReason, Result, SampleFormat, Timestamp,
};
use windows::core::{GUID, PCWSTR};
use windows::Win32::Foundation::S_FALSE;
use windows::Win32::Media::Audio::{
    eCapture, eConsole, eRender, IAudioCaptureClient, IAudioClient, IMMDevice, IMMDeviceEnumerator,
    MMDeviceEnumerator, AUDCLNT_BUFFERFLAGS_DATA_DISCONTINUITY, AUDCLNT_BUFFERFLAGS_SILENT,
    AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_LOOPBACK, DEVICE_STATE_ACTIVE, WAVEFORMATEX,
    WAVEFORMATEXTENSIBLE,
};
use windows::Win32::System::Com::StructuredStorage::PropVariantToStringAlloc;
use windows::Win32::System::Com::{CoCreateInstance, CoTaskMemFree, CLSCTX_ALL, STGM_READ};
use windows::Win32::UI::Shell::PropertiesSystem::PROPERTYKEY;

use super::com::{ComScope, Scoped};
use crate::backend::{AudioSource, RawAudio};

pub(crate) const BACKEND: &str = "wasapi";

/// `PKEY_Device_FriendlyName`, which the metadata-only bindings do not export.
const DEVICE_FRIENDLY_NAME: PROPERTYKEY = PROPERTYKEY {
    fmtid: GUID::from_u128(0xa45c254e_df1c_4efd_8020_67d146a850e0),
    pid: 14,
};

/// `WAVE_FORMAT_EXTENSIBLE`, which the metadata-only bindings do not export.
const WAVE_FORMAT_EXTENSIBLE: u16 = 0xFFFE;

/// `WAVE_FORMAT_IEEE_FLOAT`, for a plain header that names no subformat.
const WAVE_FORMAT_IEEE_FLOAT: u16 = 3;

/// `E_NOTFOUND`, returned by `GetDevice` for an id no endpoint has.
const E_NOTFOUND: windows::core::HRESULT = windows::core::HRESULT(0x8002_3003_u32 as i32);

/// `KSDATAFORMAT_SUBTYPE_IEEE_FLOAT`, to tell a float mix format from an integer
/// one when the header says `WAVE_FORMAT_EXTENSIBLE`.
const SUBTYPE_IEEE_FLOAT: GUID = GUID::from_u128(0x00000003_0000_0010_8000_00aa00389b71);

/// The shared-mode mixer always runs, so a wait longer than a few buffers means
/// the device stopped rather than that it is quiet.
const POLL_INTERVAL: Duration = Duration::from_millis(4);

fn err(source: windows::core::Error) -> CaptureError {
    CaptureError::backend(BACKEND, source)
}

/// The device enumerator, and the apartment it was made in.
/// Declare it before anything made from it and never take it apart early: what keeps a release from landing in a closed apartment is [`Scoped`]'s field order, not the caller's.
type Enumerator = Scoped<IMMDeviceEnumerator>;

/// WASAPI is COM, so a bare capture thread has to join an apartment first.
fn enumerator() -> Result<Enumerator> {
    let scope = ComScope::mta();
    // SAFETY: COM is initialised by the scope above, and the interface id comes from the bound type.
    let device = unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) }.map_err(err)?;
    Ok(Scoped::new(device, scope))
}

fn device_id(device: &IMMDevice) -> Result<AudioDeviceId> {
    // SAFETY: a property read on a live device; the returned string is freed below.
    let raw = unsafe { device.GetId() }.map_err(err)?;
    // SAFETY: `raw` is a live NUL-terminated wide string until the free below.
    let id = unsafe { raw.to_string() }.unwrap_or_default();
    // SAFETY: frees the CoTaskMem string returned above, exactly once.
    unsafe { CoTaskMemFree(Some(raw.0.cast())) };
    Ok(AudioDeviceId(id))
}

fn device_name(device: &IMMDevice) -> String {
    // SAFETY: a property-store open on a live device.
    let Ok(store) = (unsafe { device.OpenPropertyStore(STGM_READ) }) else {
        return String::new();
    };
    // SAFETY: reads a documented key from the live store above.
    let Ok(value) = (unsafe { store.GetValue(&DEVICE_FRIENDLY_NAME) }) else {
        return String::new();
    };
    // SAFETY: `value` is a live PROPVARIANT; the allocated string is freed below.
    let Ok(raw) = (unsafe { PropVariantToStringAlloc(&value) }) else {
        return String::new();
    };
    // SAFETY: `raw` is a live NUL-terminated wide string until the free below.
    let name = unsafe { raw.to_string() }.unwrap_or_default();
    // SAFETY: frees the CoTaskMem string allocated above, exactly once.
    unsafe { CoTaskMemFree(Some(raw.0.cast())) };
    name
}

/// Read the mixer's format, which shared mode never converts away from.
/// Asking for a different one is refused in shared mode, so the honest thing is to report what the device runs at and let the caller resample.
fn mix_format(client: &IAudioClient) -> Result<(AudioFormat, *mut WAVEFORMATEX)> {
    // SAFETY: a property read on a live client; the caller frees the returned header.
    let raw = unsafe { client.GetMixFormat() }.map_err(err)?;
    // SAFETY: `GetMixFormat` returns a valid CoTaskMem header, and `format_of` reads past it only when the tag says so.
    let format = unsafe { format_of(raw) }?;
    Ok((format, raw))
}

/// Read a wave header into the crate's own vocabulary.
/// # Safety: `raw` must point at a valid `WAVEFORMATEX`, and at a full `WAVEFORMATEXTENSIBLE` when its tag says so.
unsafe fn format_of(raw: *const WAVEFORMATEX) -> Result<AudioFormat> {
    // SAFETY: the caller guarantees `raw` points at a whole WAVEFORMATEX.
    let header = unsafe { *raw };
    let sample_format = if header.wFormatTag == WAVE_FORMAT_EXTENSIBLE {
        // SAFETY: the tag says this really is a WAVEFORMATEXTENSIBLE. Read unaligned, since the struct is `packed(1)`.
        let subformat = unsafe {
            core::ptr::addr_of!((*raw.cast::<WAVEFORMATEXTENSIBLE>()).SubFormat).read_unaligned()
        };
        if subformat == SUBTYPE_IEEE_FLOAT {
            SampleFormat::F32
        } else {
            integer_format(header.wBitsPerSample)?
        }
    } else if header.wFormatTag == WAVE_FORMAT_IEEE_FLOAT {
        SampleFormat::F32
    } else {
        integer_format(header.wBitsPerSample)?
    };
    Ok(AudioFormat {
        sample_rate: header.nSamplesPerSec,
        channels: header.nChannels,
        sample_format,
    })
}

fn integer_format(bits: u16) -> Result<SampleFormat> {
    match bits {
        16 => Ok(SampleFormat::I16),
        32 => Ok(SampleFormat::I32),
        // 24-bit packed is a real WASAPI format, not three bytes of an i32; refusing beats samples shifted by a byte.
        _ => Err(CaptureError::Unsupported {
            backend: BACKEND,
            operation: "read a mix format that is not 16-bit, 32-bit or float",
        }),
    }
}

/// Every active input, plus every output that can be captured as loopback.
pub(crate) fn devices() -> Result<Vec<AudioDevice>> {
    let enumerator = enumerator()?;
    let mut devices = Vec::new();
    for (flow, direction) in [
        (eCapture, AudioDirection::Input),
        (eRender, AudioDirection::Loopback),
    ] {
        // SAFETY: a query on the live enumerator; no default device is reported as an error.
        let default_id = unsafe { enumerator.value.GetDefaultAudioEndpoint(flow, eConsole) }
            .ok()
            .and_then(|device| device_id(&device).ok());
        // SAFETY: a query on the live enumerator.
        let collection = unsafe {
            enumerator
                .value
                .EnumAudioEndpoints(flow, DEVICE_STATE_ACTIVE)
        }
        .map_err(err)?;
        // SAFETY: a count read on the live collection above.
        let count = unsafe { collection.GetCount() }.map_err(err)?;
        for index in 0..count {
            // SAFETY: `index` is below the count read above.
            let Ok(device) = (unsafe { collection.Item(index) }) else {
                continue;
            };
            let Ok(id) = device_id(&device) else { continue };
            // SAFETY: activates a documented interface on a live device.
            let Ok(client) = (unsafe { device.Activate::<IAudioClient>(CLSCTX_ALL, None) }) else {
                continue;
            };
            let Ok((format, raw)) = mix_format(&client) else {
                continue;
            };
            // SAFETY: frees the mix-format header returned above, exactly once.
            unsafe { CoTaskMemFree(Some(raw.cast())) };
            devices.push(AudioDevice {
                is_default: default_id.as_ref() == Some(&id),
                id,
                name: device_name(&device),
                direction,
                format,
            });
        }
    }
    Ok(devices)
}

/// Capture from one endpoint, as an input or as loopback.
pub(crate) struct WasapiSource {
    client: IAudioClient,
    capture: IAudioCaptureClient,
    desc: AudioDesc,
    timeline: AudioTimeline,
    /// The buffer handed to the caller: either the device's samples copied out,
    /// or silence generated to cover a gap.
    staging: Vec<u8>,
    /// When the stream started, so silence for an idle device can be measured
    /// against the clock rather than against packets that never arrive.
    origin: Timestamp,
    /// What to subtract from a reported device position to get this capture's
    /// own frame count. See [`local_position`].
    anchor: Option<u64>,
    /// LAST on purpose: fields drop in declaration order, and the apartment has
    /// to outlive the interfaces made in it. Held for the source's whole life
    /// rather than just the open, because those interfaces are used on every
    /// read.
    _com: ComScope,
    stopped: bool,
}

// SAFETY: the COM objects are created and used on one thread; the source moves to its capture thread before any call.
unsafe impl Send for WasapiSource {}

impl WasapiSource {
    pub(crate) fn open(device: Option<&AudioDeviceId>, direction: AudioDirection) -> Result<Self> {
        let enumerator = enumerator()?;
        let flow = match direction {
            AudioDirection::Input => eCapture,
            AudioDirection::Loopback => eRender,
        };
        let endpoint = match device {
            Some(id) => {
                let wide: Vec<u16> = id.0.encode_utf16().chain(core::iter::once(0)).collect();
                // SAFETY: `wide` is NUL-terminated and outlives the call.
                unsafe { enumerator.value.GetDevice(PCWSTR(wide.as_ptr())) }.map_err(|error| {
                    if error.code() == E_NOTFOUND {
                        CaptureError::NotFound {
                            kind: "audio device",
                            id: 0,
                        }
                    } else {
                        err(error)
                    }
                })?
            }
            None => {
                // SAFETY: a query on the live enumerator.
                unsafe { enumerator.value.GetDefaultAudioEndpoint(flow, eConsole) }.map_err(err)?
            }
        };

        // SAFETY: activates a documented interface on the live endpoint above.
        let client: IAudioClient = unsafe { endpoint.Activate(CLSCTX_ALL, None) }.map_err(err)?;
        let (format, raw) = mix_format(&client)?;

        // Loopback reads a render endpoint's mix, the only way to hear system playback without a virtual cable.
        let flags = match direction {
            AudioDirection::Loopback => AUDCLNT_STREAMFLAGS_LOOPBACK,
            AudioDirection::Input => 0,
        };
        // SAFETY: `raw` is the mix format from this same client, live until the free below.
        let initialised = unsafe {
            client.Initialize(
                AUDCLNT_SHAREMODE_SHARED,
                flags,
                // Shared mode treats this as a hint; a generous one-second buffer tolerates a stalled consumer without dropping samples.
                10_000_000,
                0,
                raw,
                None,
            )
        };
        // SAFETY: frees the mix-format header, after `Initialize` has copied what it needs.
        unsafe { CoTaskMemFree(Some(raw.cast())) };
        initialised.map_err(err)?;

        // SAFETY: a documented service on the initialised client above.
        let capture: IAudioCaptureClient = unsafe { client.GetService() }.map_err(err)?;
        // SAFETY: the client was initialised successfully just above.
        unsafe { client.Start() }.map_err(err)?;

        Ok(Self {
            client,
            capture,
            desc: AudioDesc {
                format,
                device: device_id(&endpoint)?,
                direction,
                backend: BACKEND,
            },
            timeline: AudioTimeline::new(format),
            staging: Vec::new(),
            origin: super::now(),
            anchor: None,
            _com: enumerator.into_scope(),
            stopped: false,
        })
    }

    /// The timestamp for the timeline's current position.
    fn pts_now(&self) -> Timestamp {
        self.origin.saturating_add(self.timeline.elapsed())
    }

    /// Emit `frames` of silence at the timeline's current position.
    fn emit_silence(&mut self, frames: u64, discontinuous: bool) -> RawAudio<'_> {
        let bytes = self.timeline.silence_bytes(frames);
        self.staging.clear();
        self.staging.resize(bytes, 0);
        let pts = self.pts_now();
        self.timeline.advance(frames);
        RawAudio {
            pts,
            bytes: &self.staging,
            silence: true,
            discontinuous,
        }
    }

    /// Sample frames the device should have produced by now but has not.
    /// An idle endpoint delivers no packets at all, so a gap measured from a received buffer can never fire; the stream clock is the only thing still moving.
    fn owed_by_clock(&self) -> u64 {
        let elapsed = super::now().saturating_since(self.origin);
        let expected = self.desc.format.frames_in_duration(elapsed);
        expected.saturating_sub(self.timeline.position())
    }
}

/// A reported device position translated into this capture's own frame count, anchored on the first packet.
/// A loopback stream's position counts from when the audio engine started, so taken literally it reads as an enormous gap and buries the audio under hours of silence.
fn local_position(anchor: &mut Option<u64>, device_position: u64, timeline: u64) -> u64 {
    let at = *anchor.get_or_insert_with(|| device_position.saturating_sub(timeline));
    device_position.saturating_sub(at)
}

/// The most silence one call will generate, so a long idle period is delivered
/// as a series of ordinary buffers rather than one enormous allocation.
const MAX_SILENCE: Duration = Duration::from_millis(100);

impl Drop for WasapiSource {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

impl AudioSource for WasapiSource {
    fn describe(&self) -> &AudioDesc {
        &self.desc
    }

    fn next_buffer(&mut self, timeout: Duration) -> Result<RawAudio<'_>> {
        let deadline = std::time::Instant::now() + timeout;
        loop {
            // SAFETY: a size query on the capture client this source owns.
            let available = unsafe { self.capture.GetNextPacketSize() }.map_err(err)?;
            if available == 0 {
                // Nothing is playing: cover the time the device ran through with real silence rather than a short track.
                let owed = self.owed_by_clock();
                let chunk = self.desc.format.frames_in_duration(MAX_SILENCE);
                if owed >= chunk {
                    return Ok(self.emit_silence(chunk, false));
                }
                if std::time::Instant::now() >= deadline {
                    // Still owed something, just under a chunk: deliver it rather than report a timeout the caller can't act on.
                    if owed > 0 {
                        return Ok(self.emit_silence(owed, false));
                    }
                    return Err(CaptureError::Timeout(timeout));
                }
                std::thread::sleep(POLL_INTERVAL);
                continue;
            }

            let mut data = core::ptr::null_mut();
            let mut frames = 0u32;
            let mut flags = 0u32;
            let mut device_position = 0u64;
            // SAFETY: every out-parameter is a live local, and the buffer is released on each path below.
            let acquired = unsafe {
                self.capture.GetBuffer(
                    &mut data,
                    &mut frames,
                    &mut flags,
                    Some(&mut device_position),
                    None,
                )
            };
            if let Err(error) = acquired {
                return Err(if error.code() == S_FALSE {
                    CaptureError::Timeout(timeout)
                } else {
                    CaptureError::Lost(LostReason::DeviceLost)
                });
            }

            // The device ran on with nothing arriving, so the owed samples are real silence at a real position, not a skip.
            let local = local_position(&mut self.anchor, device_position, self.timeline.position());
            let gap = self.timeline.gap_before(local);
            let discontinuous = flags & AUDCLNT_BUFFERFLAGS_DATA_DISCONTINUITY.0 as u32 != 0;
            if gap > 0 {
                // SAFETY: releases the buffer acquired above without consuming a frame, so it is delivered next call.
                let _ = unsafe { self.capture.ReleaseBuffer(0) };
                let chunk = self.desc.format.frames_in_duration(MAX_SILENCE).min(gap);
                return Ok(self.emit_silence(chunk, discontinuous));
            }

            let bytes = self.desc.format.bytes_for(frames as usize);
            self.staging.clear();
            if flags & AUDCLNT_BUFFERFLAGS_SILENT.0 as u32 != 0 || data.is_null() {
                // WASAPI may hand back a null pointer for silence and expects the caller to write zeroes instead of reading it.
                self.staging.resize(bytes, 0);
            } else {
                // SAFETY: `GetBuffer` succeeded, so `data` covers `frames` frames of this format until `ReleaseBuffer`.
                let source = unsafe { core::slice::from_raw_parts(data, bytes) };
                self.staging.extend_from_slice(source);
            }
            // SAFETY: releases exactly the frames acquired above, after the copy out.
            let _ = unsafe { self.capture.ReleaseBuffer(frames) };

            let pts = self.pts_now();
            self.timeline.advance(u64::from(frames));
            return Ok(RawAudio {
                pts,
                bytes: &self.staging,
                silence: false,
                discontinuous,
            });
        }
    }

    fn stop(&mut self) -> Result<()> {
        if self.stopped {
            return Ok(());
        }
        self.stopped = true;
        // SAFETY: stops the client this source owns and started.
        unsafe { self.client.Stop() }.map_err(err)
    }
}

#[cfg(test)]
mod position_tests {
    use super::local_position;

    /// A capture endpoint starts at zero, so nothing should shift.
    #[test]
    fn an_input_stream_starting_at_zero_is_left_alone() {
        let mut anchor = None;
        assert_eq!(local_position(&mut anchor, 0, 0), 0);
        assert_eq!(local_position(&mut anchor, 4_800, 0), 4_800);
    }

    /// The bug this exists to prevent: a loopback endpoint reports the render
    /// stream's position, which is hours old. Read literally it is a gap of
    /// hundreds of millions of frames, and the source fills it with silence.
    #[test]
    fn a_loopback_stream_starting_hours_in_is_rebased_to_this_capture() {
        let mut anchor = None;
        let hours_in = 48_000 * 60 * 60 * 3;
        assert_eq!(local_position(&mut anchor, hours_in, 0), 0);
        assert_eq!(local_position(&mut anchor, hours_in + 4_800, 0), 4_800);
    }

    /// Silence emitted before the first packet already moved the timeline, so the anchor lines the device up with where the timeline actually is rather than restarting it at zero.
    #[test]
    fn the_anchor_lands_on_the_timeline_the_clock_already_advanced() {
        let mut anchor = None;
        let hours_in = 48_000 * 60 * 60 * 3;
        assert_eq!(local_position(&mut anchor, hours_in, 14_400), 14_400);
        assert_eq!(
            local_position(&mut anchor, hours_in + 4_800, 14_400),
            19_200
        );
    }

    /// Only the first packet sets it; a later timeline position must not move
    /// the anchor, or every gap after it would be measured from a new zero.
    #[test]
    fn the_anchor_is_fixed_by_the_first_packet_only() {
        let mut anchor = None;
        assert_eq!(local_position(&mut anchor, 1_000_000, 0), 0);
        assert_eq!(local_position(&mut anchor, 1_004_800, 999_999), 4_800);
    }

    /// A driver that resets its counter must not underflow into a huge gap.
    #[test]
    fn a_position_that_goes_backwards_saturates_at_zero() {
        let mut anchor = None;
        assert_eq!(local_position(&mut anchor, 1_000_000, 0), 0);
        assert_eq!(local_position(&mut anchor, 500, 0), 0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::Media::Audio::WAVEFORMATEXTENSIBLE_0;

    fn header(tag: u16, bits: u16) -> WAVEFORMATEX {
        WAVEFORMATEX {
            wFormatTag: tag,
            nChannels: 2,
            nSamplesPerSec: 48_000,
            nAvgBytesPerSec: 0,
            nBlockAlign: 0,
            wBitsPerSample: bits,
            cbSize: 0,
        }
    }

    fn extensible(bits: u16, subformat: GUID) -> WAVEFORMATEXTENSIBLE {
        WAVEFORMATEXTENSIBLE {
            Format: header(WAVE_FORMAT_EXTENSIBLE, bits),
            Samples: WAVEFORMATEXTENSIBLE_0 {
                wValidBitsPerSample: bits,
            },
            dwChannelMask: 3,
            SubFormat: subformat,
        }
    }

    /// A mixer running float is the common case, but assuming it is what makes a
    /// 16-bit device come back as noise.
    #[test]
    fn an_extensible_pcm_header_is_not_read_as_float() {
        let pcm = extensible(16, GUID::from_u128(0x00000001_0000_0010_8000_00aa00389b71));
        // SAFETY: `pcm` is a live WAVEFORMATEXTENSIBLE whose tag says so.
        let format =
            unsafe { format_of(core::ptr::addr_of!(pcm).cast()) }.expect("16-bit PCM is readable");
        assert_eq!(format.sample_format, SampleFormat::I16);
        assert_eq!(format.sample_rate, 48_000);
        assert_eq!(format.channels, 2);
    }

    #[test]
    fn an_extensible_float_header_is_read_as_float() {
        let float = extensible(32, SUBTYPE_IEEE_FLOAT);
        // SAFETY: `float` is a live WAVEFORMATEXTENSIBLE whose tag says so.
        let format = unsafe { format_of(core::ptr::addr_of!(float).cast()) }.expect("float");
        assert_eq!(format.sample_format, SampleFormat::F32);
    }

    #[test]
    fn a_plain_float_header_is_read_as_float() {
        let plain = header(WAVE_FORMAT_IEEE_FLOAT, 32);
        // SAFETY: `plain` is a live WAVEFORMATEX with no extension.
        let format = unsafe { format_of(&plain) }.expect("float");
        assert_eq!(format.sample_format, SampleFormat::F32);
    }

    #[test]
    fn a_plain_pcm_header_follows_its_bit_depth() {
        assert_eq!(
            // SAFETY: a live WAVEFORMATEX built by the fixture, with no extension.
            unsafe { format_of(&header(1, 16)) }
                .expect("pcm16")
                .sample_format,
            SampleFormat::I16
        );
        assert_eq!(
            // SAFETY: a live WAVEFORMATEX built by the fixture, with no extension.
            unsafe { format_of(&header(1, 32)) }
                .expect("pcm32")
                .sample_format,
            SampleFormat::I32
        );
    }

    /// Packed 24-bit is a real WASAPI format and is not three bytes of an i32.
    /// Reading it as one shifts every sample by a byte, which is loud noise.
    #[test]
    fn a_packed_24_bit_header_is_refused_rather_than_misread() {
        // SAFETY: a live WAVEFORMATEX built by the fixture, with no extension.
        assert!(unsafe { format_of(&header(1, 24)) }.is_err());
        // SAFETY: as above.
        assert!(unsafe { format_of(&header(1, 8)) }.is_err());
    }
}
