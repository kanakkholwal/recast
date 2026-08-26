//! What can encode on this machine, and which of them to use.
//!
//! Platform-free on purpose: the enumeration backends (`recast-codec-mf` on
//! Windows) produce these descriptors, and the selection policy is pure so it
//! can be tested without a GPU or a driver.

#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VideoCodec {
    H264,
    Hevc,
    Av1,
}

impl VideoCodec {
    pub fn label(self) -> &'static str {
        match self {
            Self::H264 => "H.264",
            Self::Hevc => "HEVC",
            Self::Av1 => "AV1",
        }
    }
}

/// Who provides the encoder. The order of the variants is not the selection
/// order; see [`preference_rank`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Vendor {
    Apple,
    Nvidia,
    Amd,
    Intel,
    /// A hardware encoder we recognise as such but cannot attribute.
    OtherHardware,
    Software,
}

impl Vendor {
    pub fn is_hardware(self) -> bool {
        !matches!(self, Self::Software)
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Apple => "Apple VideoToolbox",
            Self::Nvidia => "NVIDIA NVENC",
            Self::Amd => "AMD AMF",
            Self::Intel => "Intel Quick Sync",
            Self::OtherHardware => "Hardware",
            Self::Software => "Software",
        }
    }

    /// Attributes an encoder from its name. Both the Media Foundation
    /// transform names and the FFmpeg codec names carry the vendor in plain
    /// text, so one matcher serves both while they coexist.
    pub fn guess(name: &str) -> Self {
        let name = name.to_ascii_lowercase();
        // Ordered by how specific the marker is: "amf" appears inside longer
        // AMD names, and "intel" inside several Intel transform names.
        for (needle, vendor) in [
            ("nvenc", Self::Nvidia),
            ("nvidia", Self::Nvidia),
            ("videotoolbox", Self::Apple),
            ("apple", Self::Apple),
            ("qsv", Self::Intel),
            ("intel", Self::Intel),
            ("amf", Self::Amd),
            ("amd", Self::Amd),
            ("radeon", Self::Amd),
        ] {
            if name.contains(needle) {
                return vendor;
            }
        }
        Self::OtherHardware
    }
}

/// One encoder this machine exposes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EncoderDescriptor {
    /// Stable identity for the backend that produced it: a Media Foundation
    /// CLSID, or an FFmpeg codec name while that path still exists.
    pub id: String,
    /// What the transform or codec calls itself, for logs and Diagnostics.
    pub name: String,
    pub vendor: Vendor,
    pub codec: VideoCodec,
    pub hardware: bool,
}

impl EncoderDescriptor {
    pub fn label(&self) -> String {
        format!("{} ({})", self.vendor.label(), self.codec.label())
    }
}

/// Lower sorts first. Hardware always beats software; among hardware the order
/// is the one the export has always used, which is the order these encoders
/// tend to rank in throughput on the machines that have them.
fn preference_rank(descriptor: &EncoderDescriptor) -> u8 {
    if !descriptor.hardware {
        return 100;
    }
    match descriptor.vendor {
        Vendor::Apple => 0,
        Vendor::Nvidia => 1,
        Vendor::Amd => 2,
        Vendor::Intel => 3,
        Vendor::OtherHardware => 4,
        // A software-vendor descriptor claiming hardware is a backend bug; rank
        // it below every real hardware encoder rather than trusting the flag.
        Vendor::Software => 99,
    }
}

/// The encoder to use for `codec`, or `None` when nothing can encode it.
///
/// Ties break on enumeration order, so a backend that lists the system's
/// preferred transform first keeps that preference.
pub fn select_preferred(
    candidates: &[EncoderDescriptor],
    codec: VideoCodec,
) -> Option<&EncoderDescriptor> {
    candidates
        .iter()
        .filter(|d| d.codec == codec)
        .min_by_key(|d| preference_rank(d))
}

/// Every candidate for `codec`, best first. The export uses this to fall down
/// the list when one encoder fails to open rather than dropping straight to
/// software.
pub fn ranked(candidates: &[EncoderDescriptor], codec: VideoCodec) -> Vec<&EncoderDescriptor> {
    let mut matching: Vec<&EncoderDescriptor> =
        candidates.iter().filter(|d| d.codec == codec).collect();
    matching.sort_by_key(|d| preference_rank(d));
    matching
}

#[cfg(test)]
mod tests {
    use super::*;

    fn descriptor(name: &str, codec: VideoCodec, hardware: bool) -> EncoderDescriptor {
        EncoderDescriptor {
            id: name.to_string(),
            name: name.to_string(),
            vendor: match hardware {
                true => Vendor::guess(name),
                false => Vendor::Software,
            },
            codec,
            hardware,
        }
    }

    #[test]
    fn a_vendor_is_read_out_of_the_encoder_name() {
        assert_eq!(Vendor::guess("h264_nvenc"), Vendor::Nvidia);
        assert_eq!(Vendor::guess("NVIDIA H.264 Encoder MFT"), Vendor::Nvidia);
        assert_eq!(Vendor::guess("h264_amf"), Vendor::Amd);
        assert_eq!(Vendor::guess("AMD H.264 Hardware MFT Encoder"), Vendor::Amd);
        assert_eq!(Vendor::guess("h264_qsv"), Vendor::Intel);
        assert_eq!(
            Vendor::guess("Intel(R) Quick Sync Video H.264 Encoder MFT"),
            Vendor::Intel
        );
        assert_eq!(Vendor::guess("h264_videotoolbox"), Vendor::Apple);
    }

    /// An unfamiliar hardware transform must not be mistaken for software: it
    /// is still far faster than libx264 and should be tried first.
    #[test]
    fn an_unrecognised_name_is_hardware_of_unknown_make() {
        assert_eq!(Vendor::guess("Acme Turbo Encoder"), Vendor::OtherHardware);
        assert!(Vendor::guess("Acme Turbo Encoder").is_hardware());
    }

    #[test]
    fn hardware_is_chosen_over_software_whatever_the_order() {
        let candidates = [
            descriptor("libx264", VideoCodec::H264, false),
            descriptor("h264_nvenc", VideoCodec::H264, true),
        ];
        let picked = select_preferred(&candidates, VideoCodec::H264).expect("a pick");
        assert_eq!(picked.id, "h264_nvenc");
    }

    #[test]
    fn the_vendor_order_is_apple_nvidia_amd_intel() {
        let candidates = [
            descriptor("h264_qsv", VideoCodec::H264, true),
            descriptor("h264_amf", VideoCodec::H264, true),
            descriptor("h264_nvenc", VideoCodec::H264, true),
            descriptor("h264_videotoolbox", VideoCodec::H264, true),
        ];
        let order: Vec<&str> = ranked(&candidates, VideoCodec::H264)
            .iter()
            .map(|d| d.id.as_str())
            .collect();
        assert_eq!(
            order,
            [
                "h264_videotoolbox",
                "h264_nvenc",
                "h264_amf",
                "h264_qsv"
            ]
        );
    }

    #[test]
    fn a_codec_nothing_can_encode_selects_nothing() {
        let candidates = [descriptor("h264_nvenc", VideoCodec::H264, true)];
        assert!(select_preferred(&candidates, VideoCodec::Av1).is_none());
        assert!(ranked(&candidates, VideoCodec::Av1).is_empty());
    }

    /// Asking for HEVC must not hand back an H.264 encoder.
    #[test]
    fn selection_never_crosses_codecs() {
        let candidates = [
            descriptor("h264_nvenc", VideoCodec::H264, true),
            descriptor("libx265", VideoCodec::Hevc, false),
        ];
        let picked = select_preferred(&candidates, VideoCodec::Hevc).expect("a pick");
        assert_eq!(picked.id, "libx265");
        assert!(!picked.hardware);
    }

    /// The backend lists the system's own preferred transform first, so two
    /// encoders from one vendor must keep that order.
    #[test]
    fn a_tie_keeps_the_order_the_backend_enumerated() {
        let candidates = [
            descriptor("NVIDIA H.264 Encoder MFT", VideoCodec::H264, true),
            descriptor("NVIDIA H.264 Encoder MFT (legacy)", VideoCodec::H264, true),
        ];
        let picked = select_preferred(&candidates, VideoCodec::H264).expect("a pick");
        assert_eq!(picked.id, "NVIDIA H.264 Encoder MFT");
    }

    /// The `hardware` flag is authoritative, not the vendor: a backend that
    /// lists a vendor encoder it could not open must not outrank one it could.
    #[test]
    fn a_vendor_encoder_marked_unavailable_ranks_below_working_hardware() {
        let mut unavailable = descriptor("h264_nvenc", VideoCodec::H264, true);
        unavailable.hardware = false;
        let candidates = [unavailable, descriptor("h264_qsv", VideoCodec::H264, true)];
        let picked = select_preferred(&candidates, VideoCodec::H264).expect("a pick");
        assert_eq!(picked.id, "h264_qsv");
    }

    /// A backend that marks a software encoder as hardware would otherwise
    /// outrank real hardware and undo the whole point of the ranking.
    #[test]
    fn a_software_vendor_claiming_hardware_still_ranks_below_real_hardware() {
        let mut liar = descriptor("libx264", VideoCodec::H264, true);
        liar.vendor = Vendor::Software;
        let candidates = [liar, descriptor("h264_qsv", VideoCodec::H264, true)];
        let picked = select_preferred(&candidates, VideoCodec::H264).expect("a pick");
        assert_eq!(picked.id, "h264_qsv");
    }
}
