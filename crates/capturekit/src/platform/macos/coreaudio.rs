use core::ffi::c_void;
use core::ptr::NonNull;

use capturekit_core::{
    AudioDevice, AudioDeviceId, AudioDirection, AudioFormat, CaptureError, Result, SampleFormat,
};
use objc2_core_audio::{
    kAudioDevicePropertyDeviceUID, kAudioDevicePropertyNominalSampleRate,
    kAudioDevicePropertyStreamConfiguration, kAudioHardwarePropertyDefaultInputDevice,
    kAudioHardwarePropertyDefaultOutputDevice, kAudioHardwarePropertyDevices,
    kAudioObjectPropertyElementMain, kAudioObjectPropertyName, kAudioObjectPropertyScopeGlobal,
    kAudioObjectPropertyScopeInput, kAudioObjectPropertyScopeOutput, kAudioObjectSystemObject,
    AudioObjectGetPropertyData, AudioObjectGetPropertyDataSize, AudioObjectID,
    AudioObjectPropertyAddress,
};
use objc2_core_audio_types::{AudioBuffer, AudioBufferList};
use objc2_core_foundation::{CFRetained, CFString};

pub(crate) const BACKEND: &str = "coreaudio";

/// What a device is asked for. CoreAudio reports whatever the hardware runs at
/// and both capture paths convert, so this is the shape, not a demand.
const REQUESTED: AudioFormat = AudioFormat::STEREO_48K;

fn failed(operation: &'static str, status: i32) -> CaptureError {
    CaptureError::backend(
        BACKEND,
        std::io::Error::other(format!("{operation} failed: OSStatus {status}")),
    )
}

fn address(selector: u32, scope: u32) -> AudioObjectPropertyAddress {
    AudioObjectPropertyAddress {
        mSelector: selector,
        mScope: scope,
        mElement: kAudioObjectPropertyElementMain,
    }
}

/// Read a property whose size is known, as `T`.
/// # Safety: `T` must be the type CoreAudio documents for `selector` on `object`, or this reads the wrong byte count into the wrong shape.
unsafe fn property<T>(
    object: AudioObjectID,
    selector: u32,
    scope: u32,
    what: &'static str,
) -> Result<T> {
    let mut value = core::mem::MaybeUninit::<T>::uninit();
    let mut size =
        u32::try_from(core::mem::size_of::<T>()).map_err(|_| failed("size a property", -1))?;
    let mut addr = address(selector, scope);
    // SAFETY: `size` is this `T`'s own size, so CoreAudio cannot write past the uninitialised slot.
    let status = unsafe {
        AudioObjectGetPropertyData(
            object,
            NonNull::from(&mut addr),
            0,
            core::ptr::null(),
            NonNull::from(&mut size),
            NonNull::new_unchecked(value.as_mut_ptr().cast::<c_void>()),
        )
    };
    if status != 0 {
        return Err(failed(what, status));
    }
    // SAFETY: CoreAudio reported success, so it wrote a whole `T`.
    Ok(unsafe { value.assume_init() })
}

/// Read a variable-length property into a byte buffer.
fn property_bytes(
    object: AudioObjectID,
    selector: u32,
    scope: u32,
    what: &'static str,
) -> Result<Vec<u8>> {
    let mut addr = address(selector, scope);
    let mut size = 0u32;
    // SAFETY: asks only for the size, writing one `u32` into a live local.
    let status = unsafe {
        AudioObjectGetPropertyDataSize(
            object,
            NonNull::from(&mut addr),
            0,
            core::ptr::null(),
            NonNull::from(&mut size),
        )
    };
    if status != 0 {
        return Err(failed(what, status));
    }
    let mut bytes = vec![0u8; size as usize];
    if size == 0 {
        return Ok(bytes);
    }
    // SAFETY: `bytes` was sized by the query above and `size` caps the write to it.
    let status = unsafe {
        AudioObjectGetPropertyData(
            object,
            NonNull::from(&mut addr),
            0,
            core::ptr::null(),
            NonNull::from(&mut size),
            NonNull::new_unchecked(bytes.as_mut_ptr().cast::<c_void>()),
        )
    };
    if status != 0 {
        return Err(failed(what, status));
    }
    bytes.truncate(size as usize);
    Ok(bytes)
}

/// A `CFStringRef` property, as a Rust string.
fn property_string(object: AudioObjectID, selector: u32, what: &'static str) -> Result<String> {
    // SAFETY: a UID or name selector is documented to answer with a `CFStringRef`.
    let raw: *const CFString =
        unsafe { property(object, selector, kAudioObjectPropertyScopeGlobal, what)? };
    let Some(raw) = NonNull::new(raw.cast_mut()) else {
        return Err(failed(what, 0));
    };
    // SAFETY: a UID/name property returns a +1 CFString, which `from_raw` owns.
    let string = unsafe { CFRetained::from_raw(raw) };
    Ok(string.to_string())
}

/// Channels the device carries in one direction, which is how CoreAudio says
/// whether a device is an input, an output, or both.
fn channels_on(object: AudioObjectID, scope: u32) -> u16 {
    let Ok(bytes) = property_bytes(
        object,
        kAudioDevicePropertyStreamConfiguration,
        scope,
        "read a stream configuration",
    ) else {
        return 0;
    };
    let Some(head) = bytes.get(..core::mem::size_of::<u32>()) else {
        return 0;
    };
    // Unaligned throughout: a Vec<u8> carries no alignment, the list is pointer-aligned.
    let count = u32::from_ne_bytes(head.try_into().unwrap_or_default()) as usize;
    let base = core::mem::offset_of!(AudioBufferList, mBuffers);
    let stride = core::mem::size_of::<AudioBuffer>();
    let mut channels = 0u32;
    for index in 0..count {
        let at = base + index * stride;
        if bytes.len() < at + stride {
            break;
        }
        // SAFETY: the bound above keeps this inside the tail array CoreAudio wrote.
        let buffer = unsafe {
            bytes
                .as_ptr()
                .add(at)
                .cast::<AudioBuffer>()
                .read_unaligned()
        };
        channels = channels.saturating_add(buffer.mNumberChannels);
    }
    u16::try_from(channels).unwrap_or(u16::MAX)
}

fn nominal_rate(object: AudioObjectID) -> u32 {
    // SAFETY: the nominal sample rate is documented as a `Float64`.
    let rate: f64 = unsafe {
        property(
            object,
            kAudioDevicePropertyNominalSampleRate,
            kAudioObjectPropertyScopeGlobal,
            "read a nominal sample rate",
        )
    }
    .unwrap_or(0.0);
    if rate > 0.0 {
        rate as u32
    } else {
        REQUESTED.sample_rate
    }
}

fn default_device(selector: u32) -> Option<AudioObjectID> {
    // SAFETY: both default-device selectors answer with an `AudioObjectID`.
    let id: AudioObjectID = unsafe {
        property(
            kAudioObjectSystemObject as AudioObjectID,
            selector,
            kAudioObjectPropertyScopeGlobal,
            "read a default device",
        )
    }
    .ok()?;
    (id != 0).then_some(id)
}

/// Every audio device CoreAudio knows about; input channels make an input, output channels a loopback, since capturing an output means reading what it plays.
/// A device with both is listed once per direction, which is what lets a picker show it in both places.
pub(crate) fn devices() -> Result<Vec<AudioDevice>> {
    let ids = property_bytes(
        kAudioObjectSystemObject as AudioObjectID,
        kAudioHardwarePropertyDevices,
        kAudioObjectPropertyScopeGlobal,
        "list the audio devices",
    )?;
    let stride = core::mem::size_of::<AudioObjectID>();
    let default_input = default_device(kAudioHardwarePropertyDefaultInputDevice);
    let default_output = default_device(kAudioHardwarePropertyDefaultOutputDevice);

    let mut devices = Vec::new();
    for chunk in ids.chunks_exact(stride) {
        let object = AudioObjectID::from_ne_bytes(chunk.try_into().unwrap_or_default());
        // No UID means it cannot be reopened by name, so a picker cannot use it.
        let Ok(uid) = property_string(object, kAudioDevicePropertyDeviceUID, "read a device UID")
        else {
            log::debug!("coreaudio device {object} has no UID; skipping");
            continue;
        };
        let name = property_string(object, kAudioObjectPropertyName, "read a device name")
            .unwrap_or_else(|_| uid.clone());
        let rate = nominal_rate(object);

        for (scope, direction, default) in [
            (
                kAudioObjectPropertyScopeInput,
                AudioDirection::Input,
                default_input,
            ),
            (
                kAudioObjectPropertyScopeOutput,
                AudioDirection::Loopback,
                default_output,
            ),
        ] {
            let channels = channels_on(object, scope);
            if channels == 0 {
                continue;
            }
            devices.push(AudioDevice {
                id: AudioDeviceId(uid.clone()),
                name: name.clone(),
                direction,
                is_default: default == Some(object),
                format: AudioFormat::new(rate, channels, SampleFormat::F32),
            });
        }
    }
    Ok(devices)
}
