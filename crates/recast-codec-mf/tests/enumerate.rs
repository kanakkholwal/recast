use recast_codec_mf::enumerate_encoders;
// Only the Windows tests rank or select; off Windows the list is empty.
#[cfg(windows)]
use recast_codec::{ranked, select_preferred, Vendor, VideoCodec};

/// Windows has shipped a software H.264 encoder since Windows 8, so an empty
/// list means the enumeration itself is broken, not that the machine is bare.
#[test]
#[cfg(windows)]
fn windows_always_exposes_at_least_one_h264_encoder() {
    let found = enumerate_encoders();
    let h264 = ranked(&found, VideoCodec::H264);
    for encoder in &h264 {
        eprintln!(
            "{} | {:?} | hardware={} | {}",
            encoder.name, encoder.vendor, encoder.hardware, encoder.id
        );
    }
    assert!(!h264.is_empty(), "no H.264 encoder was enumerated");
}

#[test]
#[cfg(windows)]
fn every_descriptor_is_named_and_identified() {
    for encoder in enumerate_encoders() {
        assert!(!encoder.name.trim().is_empty(), "{encoder:?} has no name");
        assert!(!encoder.id.trim().is_empty(), "{encoder:?} has no clsid");
        // A transform flagged software must not be attributed to a GPU vendor,
        // or the ranking would put it above real hardware.
        if !encoder.hardware {
            assert_eq!(encoder.vendor, Vendor::Software, "{encoder:?}");
        }
    }
}

/// Enumeration has to be repeatable: the old FFmpeg probe cached its answer
/// precisely because it was slow, and that cache is what pinned exports to
/// software for a whole session.
#[test]
#[cfg(windows)]
fn enumeration_is_stable_across_calls() {
    let first = enumerate_encoders();
    let second = enumerate_encoders();
    assert_eq!(first, second);
}

#[test]
#[cfg(windows)]
fn the_pick_for_h264_is_hardware_when_the_machine_has_any() {
    let found = enumerate_encoders();
    let Some(picked) = select_preferred(&found, VideoCodec::H264) else {
        panic!("nothing selected for H.264");
    };
    let any_hardware = found
        .iter()
        .any(|e| e.codec == VideoCodec::H264 && e.hardware);
    assert_eq!(
        picked.hardware, any_hardware,
        "picked {picked:?} while hardware availability was {any_hardware}"
    );
}

#[test]
#[cfg(not(windows))]
fn there_is_no_media_foundation_off_windows() {
    assert!(enumerate_encoders().is_empty());
}
