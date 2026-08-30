use crate::avc::AvcConfig;
use crate::boxes::BoxBuf;
use crate::writer::{AudioFormat, Mp4Writer, VideoFormat};

/// Track ids, fixed because a fragmented file declares them once in `moov` and
/// every fragment refers back to them.
const VIDEO_TRACK: u32 = 1;
const AUDIO_TRACK: u32 = 2;

/// `sample_depends_on = 2` says this sample depends on nothing, which is what
/// makes it a seek point.
const SYNC_FLAGS: u32 = 2 << 24;
/// Depends on others, and the non-sync bit set.
const NON_SYNC_FLAGS: u32 = (1 << 24) | (1 << 16);

/// A fragment is described with 32-bit fields: the `mdat` header here has no
/// 64-bit form, and `trun`'s data offset is signed. Past this a fragment would
/// wrap rather than fail, and the file would be quietly corrupt.
const MAX_FRAGMENT: usize = i32::MAX as usize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FragmentError {
    /// The parameter sets have not arrived, so `moov` cannot describe the track.
    NoConfig,
    /// One fragment held more than [`MAX_FRAGMENT`] bytes. Call `fragment` more
    /// often; the cadence is the caller's to choose.
    TooLarge(usize),
}

impl std::fmt::Display for FragmentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoConfig => write!(f, "no parameter sets yet"),
            Self::TooLarge(bytes) => {
                write!(
                    f,
                    "a fragment of {bytes} bytes cannot be described in 32 bits"
                )
            }
        }
    }
}

impl std::error::Error for FragmentError {}

/// The `mdat` size field, or `None` when it would not fit.
fn mdat_size(payload: usize) -> Option<u32> {
    payload
        .checked_add(8)
        .filter(|n| *n <= MAX_FRAGMENT)?
        .try_into()
        .ok()
}

/// A `trun` data offset, which is signed and relative to the start of `moof`.
fn data_offset(base: usize, within: usize) -> Option<i32> {
    base.checked_add(within)
        .filter(|n| *n <= MAX_FRAGMENT)?
        .try_into()
        .ok()
}

struct Pending {
    data: Vec<u8>,
    duration: u32,
    is_sync: bool,
    composition_offset: i32,
}

/// Writes fragmented MP4: an initialisation segment, then `moof` + `mdat`
/// fragments.
///
/// The point is durability. A plain MP4 only becomes playable when `finish`
/// writes the header, so an export killed at 90% leaves nothing. Every fragment
/// here is complete on its own, so a file cut short plays up to the last one.
#[derive(Default)]
pub struct FragmentedWriter {
    video_format: VideoFormat,
    audio_format: Option<AudioFormat>,
    avc: AvcConfig,
    sequence: u32,
    video_time: u64,
    audio_time: u64,
    video: Vec<Pending>,
    audio: Vec<Pending>,
}

impl FragmentedWriter {
    pub fn new(video_format: VideoFormat) -> Self {
        Self {
            video_format,
            ..Default::default()
        }
    }

    pub fn set_avc_config(&mut self, config: AvcConfig) {
        self.avc = config;
    }

    pub fn set_audio_format(&mut self, format: AudioFormat) {
        self.audio_format = Some(format);
    }

    /// `ftyp` and a `moov` whose sample tables are empty, which is what says the
    /// samples arrive in fragments. Errors until the parameter sets are known.
    ///
    /// Written once, before any fragment, and never rewritten: that is the whole
    /// difference from the progressive writer.
    pub fn initialization_segment(&self) -> Result<Vec<u8>, FragmentError> {
        let mut header = Mp4Writer::new(self.video_format);
        header.set_avc_config(self.avc.clone());
        if let Some(format) = &self.audio_format {
            header.set_audio_format(format.clone());
        }
        header
            .initialization_segment()
            .ok_or(FragmentError::NoConfig)
    }

    pub fn push_sample(&mut self, data: &[u8], duration: u32, is_sync: bool) {
        self.push_sample_with_offset(data, duration, is_sync, 0);
    }

    pub fn push_sample_with_offset(
        &mut self,
        data: &[u8],
        duration: u32,
        is_sync: bool,
        composition_offset: i32,
    ) {
        self.video.push(Pending {
            data: data.to_vec(),
            duration,
            is_sync,
            composition_offset,
        });
    }

    pub fn push_audio_sample(&mut self, data: &[u8], duration: u32) {
        self.audio.push(Pending {
            data: data.to_vec(),
            duration,
            is_sync: true,
            composition_offset: 0,
        });
    }

    /// Samples waiting for the next [`Self::fragment`] call.
    pub fn pending(&self) -> usize {
        self.video.len() + self.audio.len()
    }

    /// Emits everything pushed so far as one fragment and clears the buffer.
    /// `None` when nothing is waiting.
    ///
    /// The caller decides the cadence. A fragment per second is the usual trade:
    /// shorter means more `moof` overhead, longer means more work lost to a
    /// crash, and one must stay under 4 GiB because its `mdat` header is 32 bit.
    pub fn fragment(&mut self) -> Result<Option<Vec<u8>>, FragmentError> {
        if self.video.is_empty() && self.audio.is_empty() {
            return Ok(None);
        }
        self.sequence += 1;

        let video = std::mem::take(&mut self.video);
        let audio = std::mem::take(&mut self.audio);
        let video_bytes: usize = video.iter().map(|s| s.data.len()).sum();

        let mut buf = BoxBuf::new();
        buf.open(b"moof");
        buf.open_full(b"mfhd", 0, 0);
        buf.u32(self.sequence);
        buf.close();

        // Where each track's data offset was written, filled in once `moof`'s size is known; the same two-pass shape as `moov`.
        let mut patches = Vec::new();
        if !video.is_empty() {
            let at = write_traf(&mut buf, VIDEO_TRACK, self.video_time, &video);
            patches.push((at, 0usize));
            self.video_time += video.iter().map(|s| s.duration as u64).sum::<u64>();
        }
        if !audio.is_empty() {
            let at = write_traf(&mut buf, AUDIO_TRACK, self.audio_time, &audio);
            patches.push((at, video_bytes));
            self.audio_time += audio.iter().map(|s| s.duration as u64).sum::<u64>();
        }
        buf.close();

        let mut out = buf.into_bytes();
        // `default-base-is-moof` makes offsets relative to `moof`'s first byte, so the base is its size plus the `mdat` header.
        let base = out.len() + 8;
        let payload: usize = video_bytes + audio.iter().map(|s| s.data.len()).sum::<usize>();
        let size = mdat_size(payload).ok_or(FragmentError::TooLarge(payload))?;
        for (at, within) in patches {
            let offset = data_offset(base, within).ok_or(FragmentError::TooLarge(base + within))?;
            let slot = out
                .get_mut(at..at + 4)
                .ok_or(FragmentError::TooLarge(payload))?;
            slot.copy_from_slice(&offset.to_be_bytes());
        }

        out.extend_from_slice(&size.to_be_bytes());
        out.extend_from_slice(b"mdat");
        for sample in video.iter().chain(audio.iter()) {
            out.extend_from_slice(&sample.data);
        }
        Ok(Some(out))
    }
}

/// Writes one track's fragment header and returns where its data offset sits.
fn write_traf(buf: &mut BoxBuf, track: u32, decode_time: u64, samples: &[Pending]) -> usize {
    buf.open(b"traf");

    // Without `default-base-is-moof`, offsets are relative to the FILE start, which a standalone fragment cannot know.
    buf.open_full(b"tfhd", 0, 0x02_0000);
    buf.u32(track);
    buf.close();

    // Version 1: a long recording overflows a 32-bit decode time, and the crossing fragment would land at the file start.
    buf.open_full(b"tfdt", 1, 0);
    buf.u64(decode_time);
    buf.close();

    // Version 1 so the composition offset is signed, which keeps a stream with B frames starting at zero.
    let flags = 0x0001 | 0x0100 | 0x0200 | 0x0400 | 0x0800;
    buf.open_full(b"trun", 1, flags);
    buf.u32(samples.len() as u32);
    let offset_at = buf.len();
    buf.u32(0);
    for sample in samples {
        buf.u32(sample.duration);
        buf.u32(sample.data.len() as u32);
        buf.u32(match sample.is_sync {
            true => SYNC_FLAGS,
            false => NON_SYNC_FLAGS,
        });
        buf.i32(sample.composition_offset);
    }
    buf.close();

    buf.close();
    offset_at
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> AvcConfig {
        AvcConfig {
            sps: vec![vec![0x67, 0x42, 0xc0, 0x1e]],
            pps: vec![vec![0x68, 0xce, 0x3c, 0x80]],
        }
    }

    fn writer() -> FragmentedWriter {
        let mut w = FragmentedWriter::new(VideoFormat {
            width: 320,
            height: 240,
            timescale: 30,
        });
        w.set_avc_config(config());
        w
    }

    fn find(data: &[u8], kind: &[u8; 4]) -> Option<usize> {
        data.windows(4).position(|w| w == kind)
    }

    /// Unwraps both layers: the tests below are about box contents, and a
    /// writer that refuses is a failure worth reading in the panic message.
    fn frag(w: &mut FragmentedWriter) -> Vec<u8> {
        w.fragment()
            .expect("the writer accepted it")
            .expect("a fragment")
    }

    /// The size fields are checked at the boundary rather than by building a
    /// two-gigabyte fragment, which no test should allocate.
    #[test]
    fn a_fragment_that_would_not_fit_in_its_size_field_is_refused() {
        assert_eq!(mdat_size(0), Some(8));
        assert_eq!(mdat_size(100), Some(108));
        assert_eq!(mdat_size(MAX_FRAGMENT - 8), Some(MAX_FRAGMENT as u32));
        assert_eq!(mdat_size(MAX_FRAGMENT - 7), None);
        assert_eq!(mdat_size(usize::MAX), None);
    }

    /// `trun` holds the data offset SIGNED, so the ceiling is half what an
    /// unsigned field would give and a wrap would land it before the fragment.
    #[test]
    fn a_data_offset_past_the_signed_ceiling_is_refused() {
        assert_eq!(data_offset(0, 0), Some(0));
        assert_eq!(data_offset(64, 500), Some(564));
        assert_eq!(data_offset(MAX_FRAGMENT, 0), Some(i32::MAX));
        assert_eq!(data_offset(MAX_FRAGMENT, 1), None);
        assert_eq!(data_offset(usize::MAX, 1), None);
    }

    fn u32_at(data: &[u8], at: usize) -> u32 {
        u32::from_be_bytes(data[at..at + 4].try_into().unwrap())
    }

    #[test]
    fn the_initialisation_segment_declares_fragments_and_carries_no_samples() {
        let w = writer();
        let init = w.initialization_segment().expect("an init segment");
        assert!(find(&init, b"moov").is_some());
        // `mvex` is what tells a reader the sample tables are empty on purpose.
        assert!(
            find(&init, b"mvex").is_some(),
            "no mvex in the init segment"
        );
        assert!(
            find(&init, b"trex").is_some(),
            "no trex in the init segment"
        );
        assert!(
            find(&init, b"mdat").is_none(),
            "the init segment carried media"
        );
    }

    #[test]
    fn nothing_pushed_is_no_fragment() {
        let mut w = writer();
        assert!(w.fragment().expect("no error").is_none());
    }

    #[test]
    fn a_fragment_carries_its_samples_in_order() {
        let mut w = writer();
        w.push_sample(&[1, 1, 1], 1, true);
        w.push_sample(&[2, 2], 1, false);
        let data = frag(&mut w);
        let mdat = find(&data, b"mdat").expect("an mdat");
        assert_eq!(&data[mdat + 4..], &[1, 1, 1, 2, 2]);
    }

    /// The offset is what points a player at the bytes. With
    /// `default-base-is-moof` it counts from the first byte of the fragment, so
    /// following it has to land exactly on the first sample.
    #[test]
    fn the_data_offset_points_at_the_first_sample() {
        let mut w = writer();
        w.push_sample(&[9, 9, 9, 9], 1, true);
        let data = frag(&mut w);
        let trun = find(&data, b"trun").expect("a trun");
        // type, version and flags, sample count, then the offset.
        let offset = u32_at(&data, trun + 4 + 4 + 4) as usize;
        assert_eq!(&data[offset..offset + 4], &[9, 9, 9, 9]);
    }

    #[test]
    fn a_second_track_starts_after_the_first_ones_bytes() {
        let mut w = writer();
        w.set_audio_format(AudioFormat {
            sample_rate: 48_000,
            channels: 2,
            config: vec![0x11, 0x90],
        });
        w.push_sample(&[1, 2, 3, 4, 5, 6], 1, true);
        w.push_audio_sample(&[7, 8], 1024);
        let data = frag(&mut w);

        let first = find(&data, b"trun").expect("a video trun");
        let second = first + 4 + find(&data[first + 4..], b"trun").expect("an audio trun");
        let video_at = u32_at(&data, first + 12) as usize;
        let audio_at = u32_at(&data, second + 12) as usize;
        assert_eq!(&data[video_at..video_at + 6], &[1, 2, 3, 4, 5, 6]);
        assert_eq!(&data[audio_at..audio_at + 2], &[7, 8]);
    }

    /// Decode time carries across fragments. Restarting it at zero puts every
    /// fragment at the start of the timeline, which plays as the last one only.
    #[test]
    fn decode_time_continues_from_one_fragment_to_the_next() {
        let mut w = writer();
        w.push_sample(&[1], 10, true);
        w.push_sample(&[2], 10, false);
        let first = frag(&mut w);
        w.push_sample(&[3], 10, false);
        let second = frag(&mut w);

        let at = |data: &[u8]| {
            let tfdt = find(data, b"tfdt").expect("a tfdt");
            u64::from_be_bytes(data[tfdt + 8..tfdt + 16].try_into().unwrap())
        };
        assert_eq!(at(&first), 0);
        assert_eq!(at(&second), 20);
    }

    #[test]
    fn fragments_are_numbered_from_one_and_upwards() {
        let mut w = writer();
        let number = |data: &[u8]| {
            let mfhd = find(data, b"mfhd").expect("an mfhd");
            u32_at(data, mfhd + 8)
        };
        w.push_sample(&[1], 1, true);
        assert_eq!(number(&frag(&mut w)), 1);
        w.push_sample(&[2], 1, false);
        assert_eq!(number(&frag(&mut w)), 2);
    }

    #[test]
    fn a_sync_sample_is_flagged_differently_from_the_rest() {
        let mut w = writer();
        w.push_sample(&[1], 1, true);
        w.push_sample(&[2], 1, false);
        let data = frag(&mut w);
        let trun = find(&data, b"trun").expect("a trun");
        // Past the header and the offset, each sample is four words.
        let first = trun + 16;
        assert_eq!(u32_at(&data, first + 8), SYNC_FLAGS);
        assert_eq!(u32_at(&data, first + 24), NON_SYNC_FLAGS);
    }

    #[test]
    fn a_fragment_empties_the_buffer() {
        let mut w = writer();
        w.push_sample(&[1], 1, true);
        assert_eq!(w.pending(), 1);
        let _ = w.fragment();
        assert_eq!(w.pending(), 0);
        assert!(w.fragment().expect("no error").is_none());
    }
}
