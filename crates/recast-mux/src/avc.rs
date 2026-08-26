/// The parameter sets an `avcC` record needs, plus the profile bytes read out
/// of the SPS.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AvcConfig {
    pub sps: Vec<Vec<u8>>,
    pub pps: Vec<Vec<u8>>,
}

impl AvcConfig {
    pub fn is_empty(&self) -> bool {
        self.sps.is_empty() || self.pps.is_empty()
    }

    /// The `AVCDecoderConfigurationRecord` payload, without the box header.
    /// `None` until at least one SPS and one PPS have been seen, because a
    /// decoder cannot start without both.
    pub fn record(&self) -> Option<Vec<u8>> {
        let sps = self.sps.first()?;
        if self.pps.is_empty() || sps.len() < 4 {
            return None;
        }
        let mut out = Vec::new();
        out.push(1);
        // profile_idc, constraint flags, level_idc, straight out of the SPS.
        out.extend_from_slice(&sps[1..4]);
        // Six reserved bits set, then lengthSizeMinusOne = 3 (4-byte lengths).
        out.push(0xFF);
        // Three reserved bits set, then the SPS count.
        out.push(0xE0 | (self.sps.len().min(31) as u8));
        for unit in self.sps.iter().take(31) {
            out.extend_from_slice(&(unit.len() as u16).to_be_bytes());
            out.extend_from_slice(unit);
        }
        out.push(self.pps.len().min(255) as u8);
        for unit in self.pps.iter().take(255) {
            out.extend_from_slice(&(unit.len() as u16).to_be_bytes());
            out.extend_from_slice(unit);
        }
        Some(out)
    }
}

/// NAL unit types we act on. Everything else is copied through untouched.
const NAL_SPS: u8 = 7;
const NAL_PPS: u8 = 8;
const NAL_IDR: u8 = 5;

/// Splits an Annex B stream into NAL units, dropping the start codes.
///
/// Accepts both the three- and four-byte start codes, which encoders mix within
/// one stream: the four-byte form leads an access unit and the three-byte form
/// separates NALs inside it.
pub fn split_annex_b(data: &[u8]) -> Vec<&[u8]> {
    let mut units = Vec::new();
    let mut starts = Vec::new();
    let mut i = 0;
    while i + 3 <= data.len() {
        if data[i] == 0 && data[i + 1] == 0 && data[i + 2] == 1 {
            starts.push(i + 3);
            i += 3;
        } else {
            i += 1;
        }
    }
    for (n, &start) in starts.iter().enumerate() {
        let end = match starts.get(n + 1) {
            // Back off the next start code, and the extra leading zero when it
            // is the four-byte form.
            Some(&next) => {
                let mut end = next - 3;
                if end > start && data[end - 1] == 0 {
                    end -= 1;
                }
                end
            }
            None => data.len(),
        };
        if end > start {
            units.push(&data[start..end]);
        }
    }
    units
}

/// Groups an Annex B stream into access units, one per coded picture.
///
/// A unit ends at the NAL that starts the next picture: the VCL types (1..=5)
/// are the pictures themselves, and any parameter sets or SEI in front of one
/// belong to it. Good enough for streams with one slice per picture, which is
/// everything our encoders produce.
pub fn split_access_units(data: &[u8]) -> Vec<Vec<u8>> {
    let mut units: Vec<Vec<u8>> = Vec::new();
    let mut seen_vcl = false;
    for nal in split_annex_b(data) {
        let Some(&header) = nal.first() else { continue };
        let kind = header & 0x1F;
        let is_vcl = (1..=5).contains(&kind);
        // Any NAL after a picture has been seen begins the next one.
        if units.is_empty() || seen_vcl {
            units.push(Vec::new());
            seen_vcl = false;
        }
        if let Some(current) = units.last_mut() {
            current.extend_from_slice(&[0, 0, 0, 1]);
            current.extend_from_slice(nal);
        }
        seen_vcl |= is_vcl;
    }
    units
}

/// One converted access unit: length-prefixed NAL units for the sample, plus
/// whatever parameter sets it carried.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Converted {
    pub sample: Vec<u8>,
    pub config: AvcConfig,
    pub is_sync: bool,
}

/// Converts an Annex B access unit into the length-prefixed form MP4 stores.
///
/// Parameter sets are pulled OUT of the sample: they belong in `avcC`, and
/// leaving them inline makes some decoders re-initialise on every keyframe.
pub fn annex_b_to_avcc(data: &[u8]) -> Converted {
    let mut out = Converted::default();
    for unit in split_annex_b(data) {
        let Some(&header) = unit.first() else { continue };
        match header & 0x1F {
            NAL_SPS => out.config.sps.push(unit.to_vec()),
            NAL_PPS => out.config.pps.push(unit.to_vec()),
            kind => {
                if kind == NAL_IDR {
                    out.is_sync = true;
                }
                out.sample
                    .extend_from_slice(&(unit.len() as u32).to_be_bytes());
                out.sample.extend_from_slice(unit);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn annex_b(units: &[&[u8]]) -> Vec<u8> {
        let mut out = Vec::new();
        for unit in units {
            out.extend_from_slice(&[0, 0, 0, 1]);
            out.extend_from_slice(unit);
        }
        out
    }

    #[test]
    fn a_stream_splits_on_four_byte_start_codes() {
        let data = annex_b(&[&[0x67, 1, 2], &[0x68, 3], &[0x65, 4, 5, 6]]);
        let units = split_annex_b(&data);
        assert_eq!(units.len(), 3);
        assert_eq!(units[0], &[0x67, 1, 2]);
        assert_eq!(units[2], &[0x65, 4, 5, 6]);
    }

    /// Encoders mix the two start-code forms inside one access unit, so a
    /// splitter that only knows one of them merges NALs together.
    #[test]
    fn three_and_four_byte_start_codes_both_split() {
        let mut data = vec![0, 0, 0, 1, 0x67, 9];
        data.extend_from_slice(&[0, 0, 1, 0x68, 8]);
        let units = split_annex_b(&data);
        assert_eq!(units.len(), 2);
        assert_eq!(units[0], &[0x67, 9]);
        assert_eq!(units[1], &[0x68, 8]);
    }

    #[test]
    fn each_nal_is_prefixed_with_its_own_length() {
        let data = annex_b(&[&[0x41, 1, 2, 3], &[0x41, 4]]);
        let converted = annex_b_to_avcc(&data);
        assert_eq!(
            converted.sample,
            vec![0, 0, 0, 4, 0x41, 1, 2, 3, 0, 0, 0, 2, 0x41, 4]
        );
    }

    /// Parameter sets go in `avcC`, not in the sample: leaving them inline
    /// makes decoders re-initialise on every keyframe.
    #[test]
    fn parameter_sets_are_lifted_out_of_the_sample() {
        let data = annex_b(&[&[0x67, 0x64, 0x00, 0x28, 1], &[0x68, 2], &[0x65, 3]]);
        let converted = annex_b_to_avcc(&data);
        assert_eq!(converted.config.sps.len(), 1);
        assert_eq!(converted.config.pps.len(), 1);
        assert_eq!(converted.sample, vec![0, 0, 0, 2, 0x65, 3]);
    }

    #[test]
    fn an_idr_unit_marks_the_sample_as_a_sync_point() {
        let idr = annex_b(&[&[0x65, 1]]);
        let inter = annex_b(&[&[0x41, 1]]);
        assert!(annex_b_to_avcc(&idr).is_sync);
        assert!(!annex_b_to_avcc(&inter).is_sync);
    }

    #[test]
    fn the_record_carries_the_profile_bytes_from_the_sps() {
        let config = AvcConfig {
            sps: vec![vec![0x67, 0x64, 0x00, 0x28, 0xAC]],
            pps: vec![vec![0x68, 0xEE]],
        };
        let record = config.record().expect("a record");
        assert_eq!(record[0], 1, "configurationVersion");
        assert_eq!(&record[1..4], &[0x64, 0x00, 0x28], "profile/compat/level");
        assert_eq!(record[4], 0xFF, "four-byte NAL lengths");
        assert_eq!(record[5], 0xE1, "one SPS");
    }

    #[test]
    fn a_record_needs_both_parameter_sets() {
        let sps_only = AvcConfig {
            sps: vec![vec![0x67, 0x64, 0x00, 0x28]],
            pps: Vec::new(),
        };
        assert!(sps_only.record().is_none());
        assert!(AvcConfig::default().record().is_none());
    }

    /// A truncated SPS cannot supply the profile bytes, and slicing it would
    /// panic.
    #[test]
    fn a_short_sps_is_refused_rather_than_slicing_out_of_range() {
        let config = AvcConfig {
            sps: vec![vec![0x67, 0x64]],
            pps: vec![vec![0x68]],
        };
        assert!(config.record().is_none());
    }

    #[test]
    fn each_coded_picture_becomes_one_access_unit() {
        let data = annex_b(&[
            &[0x67, 1],
            &[0x68, 2],
            &[0x65, 3],
            &[0x41, 4],
            &[0x41, 5],
        ]);
        let units = split_access_units(&data);
        assert_eq!(units.len(), 3, "expected one unit per picture");
        // The parameter sets ride with the picture they precede.
        assert_eq!(split_annex_b(&units[0]).len(), 3);
        assert_eq!(split_annex_b(&units[1]).len(), 1);
    }

    #[test]
    fn a_stream_with_no_start_codes_yields_nothing() {
        assert!(split_annex_b(&[1, 2, 3, 4]).is_empty());
        assert!(annex_b_to_avcc(&[1, 2, 3, 4]).sample.is_empty());
    }
}
