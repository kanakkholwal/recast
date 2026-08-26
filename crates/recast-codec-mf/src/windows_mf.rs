use std::ffi::c_void;
use std::sync::OnceLock;

use windows::core::GUID;
use windows::Win32::Media::MediaFoundation::{
    MFStartup, MFTEnumEx, IMFActivate, MFMediaType_Video, MFVideoFormat_H264, MFVideoFormat_HEVC,
    MFT_CATEGORY_VIDEO_ENCODER, MFT_ENUM_FLAG, MFT_ENUM_FLAG_ASYNCMFT, MFT_ENUM_FLAG_HARDWARE,
    MFT_ENUM_FLAG_SORTANDFILTER, MFT_ENUM_FLAG_SYNCMFT, MFT_ENUM_HARDWARE_URL_Attribute,
    MFT_FRIENDLY_NAME_Attribute, MFT_REGISTER_TYPE_INFO, MFT_TRANSFORM_CLSID_Attribute,
    MF_TRANSFORM_ASYNC, MF_VERSION,
};
use windows::Win32::System::Com::CoTaskMemFree;

use recast_codec::{EncoderDescriptor, Vendor, VideoCodec};

/// Media Foundation is reference counted per process. We start it once and
/// never shut it down: enumeration happens repeatedly over the app's life, and
/// a matching `MFShutdown` would only run at exit anyway.
pub(crate) fn ensure_started() -> bool {
    static STARTED: OnceLock<bool> = OnceLock::new();
    *STARTED.get_or_init(|| {
        // SAFETY: no arguments to get wrong; the call is idempotent per process.
        unsafe { MFStartup(MF_VERSION, 0).is_ok() }
    })
}

/// The activate for `id`, plus whether the transform is asynchronous. Matched
/// by re-enumerating rather than by holding activates open, so a probe never
/// keeps a hardware session reserved.
pub(crate) fn activate_for(id: &str) -> Option<(IMFActivate, bool)> {
    if !ensure_started() {
        return None;
    }
    for (codec, subtype) in [
        (VideoCodec::H264, MFVideoFormat_H264),
        (VideoCodec::Hevc, MFVideoFormat_HEVC),
    ] {
        let _ = codec;
        for activate in activates(subtype) {
            // SAFETY: reading a GUID attribute every registered MFT carries.
            let Ok(clsid) = (unsafe { activate.GetGUID(&MFT_TRANSFORM_CLSID_Attribute) }) else {
                continue;
            };
            if format!("{clsid:?}") == id {
                // SAFETY: a missing attribute is the synchronous case.
                let asynchronous = unsafe { activate.GetUINT32(&MF_TRANSFORM_ASYNC) }
                    .map(|v| v != 0)
                    .unwrap_or(false);
                return Some((activate, asynchronous));
            }
        }
    }
    None
}

pub fn enumerate_encoders() -> Vec<EncoderDescriptor> {
    if !ensure_started() {
        return Vec::new();
    }
    let mut found = Vec::new();
    for (codec, subtype) in [
        (VideoCodec::H264, MFVideoFormat_H264),
        (VideoCodec::Hevc, MFVideoFormat_HEVC),
    ] {
        found.extend(enumerate_codec(codec, subtype));
    }
    found
}

fn enumerate_codec(codec: VideoCodec, subtype: GUID) -> Vec<EncoderDescriptor> {
    activates(subtype)
        .iter()
        .filter_map(|activate| describe(activate, codec))
        .collect()
}

/// Every video-encoder transform that can output `subtype`.
fn activates(subtype: GUID) -> Vec<IMFActivate> {
    encoder_activates(MFT_CATEGORY_VIDEO_ENCODER, MFMediaType_Video, subtype)
}

/// Every encoder transform in `category` that can output `major`/`subtype`, in
/// the order the system ranks them.
pub(crate) fn encoder_activates(
    category: GUID,
    major: GUID,
    subtype: GUID,
) -> Vec<IMFActivate> {
    let output = MFT_REGISTER_TYPE_INFO {
        guidMajorType: major,
        guidSubtype: subtype,
    };
    // SORTANDFILTER is what makes the system's own preference the array order,
    // which the selection policy then keeps for ties.
    let flags = MFT_ENUM_FLAG(
        MFT_ENUM_FLAG_HARDWARE.0
            | MFT_ENUM_FLAG_SYNCMFT.0
            | MFT_ENUM_FLAG_ASYNCMFT.0
            | MFT_ENUM_FLAG_SORTANDFILTER.0,
    );

    let mut activates: *mut Option<IMFActivate> = std::ptr::null_mut();
    let mut count: u32 = 0;
    // SAFETY: `activates` and `count` are the out-params the API documents, and
    // the array it allocates is freed below.
    let enumerated = unsafe {
        MFTEnumEx(
            category,
            flags,
            None,
            Some(&output),
            &mut activates,
            &mut count,
        )
    };
    if enumerated.is_err() || activates.is_null() {
        return Vec::new();
    }

    // SAFETY: the API filled `count` entries at `activates`.
    let slots = unsafe { std::slice::from_raw_parts_mut(activates, count as usize) };
    let mut found = Vec::with_capacity(slots.len());
    for slot in slots.iter_mut() {
        // Taking the interface out moves ownership here, so the release happens
        // when it drops rather than being leaked with the array.
        if let Some(activate) = slot.take() {
            found.push(activate);
        }
    }
    // SAFETY: MFTEnumEx allocated this array with CoTaskMemAlloc, and every
    // interface in it has been taken out above.
    unsafe { CoTaskMemFree(Some(activates as *const c_void)) };
    found
}

fn describe(activate: &IMFActivate, codec: VideoCodec) -> Option<EncoderDescriptor> {
    let name = allocated_string(activate, &MFT_FRIENDLY_NAME_Attribute)?;
    // Only hardware transforms carry the hardware URL, so its presence is the
    // system's own answer rather than a guess from the name.
    let hardware = allocated_string(activate, &MFT_ENUM_HARDWARE_URL_Attribute).is_some();
    // SAFETY: reading a GUID attribute that every registered MFT carries.
    let clsid = unsafe { activate.GetGUID(&MFT_TRANSFORM_CLSID_Attribute) }.ok()?;
    Some(EncoderDescriptor {
        id: format!("{clsid:?}"),
        vendor: match hardware {
            true => Vendor::guess(&name),
            false => Vendor::Software,
        },
        name,
        codec,
        hardware,
    })
}

fn allocated_string(activate: &IMFActivate, key: &GUID) -> Option<String> {
    let mut value = windows::core::PWSTR::null();
    let mut length = 0u32;
    // SAFETY: the API writes a CoTaskMem string we free below; a missing
    // attribute returns an error and leaves `value` null.
    unsafe {
        activate.GetAllocatedString(key, &mut value, &mut length).ok()?;
        let text = value.to_string().ok();
        CoTaskMemFree(Some(value.0 as *const c_void));
        text
    }
}
