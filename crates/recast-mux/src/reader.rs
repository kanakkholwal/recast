use std::ops::Range;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReadError {
    /// The file is not an MP4, or the part that says so is missing.
    NotMp4,
    /// A box declares a size that runs past its parent.
    Truncated(&'static str),
    /// A box we need is absent.
    Missing(&'static str),
}

impl std::fmt::Display for ReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotMp4 => write!(f, "not an MP4"),
            Self::Truncated(what) => write!(f, "{what} runs past the end of its parent"),
            Self::Missing(what) => write!(f, "{what} is missing"),
        }
    }
}

impl std::error::Error for ReadError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackKind {
    Video,
    Audio,
    Other,
}

/// One sample located in the file. The payload is not copied: a reader over a
/// two-hour recording would otherwise materialise the whole thing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SampleRef {
    pub offset: u64,
    pub size: u32,
    /// Decode time, in the track's own timescale.
    pub decode_time: u64,
    pub duration: u32,
    /// Presentation minus decode, from `ctts`. Zero when the track has none.
    pub composition_offset: i32,
    pub is_sync: bool,
}

impl SampleRef {
    pub fn presentation_time(&self) -> i64 {
        self.decode_time as i64 + self.composition_offset as i64
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Track {
    pub id: u32,
    pub kind: TrackKind,
    pub timescale: u32,
    pub duration: u64,
    /// The sample entry's four-character code: `avc1`, `mp4a` and so on.
    pub format: [u8; 4],
    pub width: u16,
    pub height: u16,
    pub sample_rate: u32,
    pub channels: u16,
    /// `avcC` for video, the `AudioSpecificConfig` out of `esds` for audio.
    pub decoder_config: Vec<u8>,
    pub samples: Vec<SampleRef>,
}

impl Track {
    pub fn seconds(&self) -> f64 {
        if self.timescale == 0 {
            return 0.0;
        }
        self.duration as f64 / self.timescale as f64
    }
}

/// Reads the structure of an MP4 without decoding anything.
#[derive(Debug)]
pub struct Mp4Reader<'a> {
    data: &'a [u8],
    tracks: Vec<Track>,
}

impl<'a> Mp4Reader<'a> {
    pub fn new(data: &'a [u8]) -> Result<Self, ReadError> {
        let top = boxes(data, 0..data.len())?;
        if !top.iter().any(|(kind, _)| kind == b"ftyp") {
            return Err(ReadError::NotMp4);
        }
        let moov = top
            .iter()
            .find(|(kind, _)| kind == b"moov")
            .ok_or(ReadError::Missing("moov"))?
            .1
            .clone();

        let mut tracks = Vec::new();
        for (kind, range) in boxes(data, moov)? {
            if &kind == b"trak" {
                tracks.push(read_track(data, range)?);
            }
        }
        Ok(Self { data, tracks })
    }

    pub fn tracks(&self) -> &[Track] {
        &self.tracks
    }

    pub fn video(&self) -> Option<&Track> {
        self.tracks.iter().find(|t| t.kind == TrackKind::Video)
    }

    pub fn audio(&self) -> Option<&Track> {
        self.tracks.iter().find(|t| t.kind == TrackKind::Audio)
    }

    /// The bytes one sample points at, or `None` when it points outside the file.
    pub fn sample_data(&self, sample: &SampleRef) -> Option<&'a [u8]> {
        let start = usize::try_from(sample.offset).ok()?;
        let end = start.checked_add(sample.size as usize)?;
        self.data.get(start..end)
    }
}

/// A box's four-character code and the range its body occupies.
type Child = ([u8; 4], Range<usize>);

/// The direct children of the box body at `range`.
fn boxes(data: &[u8], range: Range<usize>) -> Result<Vec<Child>, ReadError> {
    let mut out = Vec::new();
    let mut at = range.start;
    while at + 8 <= range.end {
        let size = u32be(data, at) as u64;
        let Some(kind) = data
            .get(at + 4..at + 8)
            .and_then(|b| <[u8; 4]>::try_from(b).ok())
        else {
            return Err(ReadError::Truncated("box header"));
        };
        // 1 means the real size is a 64-bit field after the type; 0 means the box runs to the end of its parent.
        let (header, size) = match size {
            1 => {
                if at + 16 > range.end {
                    return Err(ReadError::Truncated("largesize"));
                }
                (16usize, u64be(data, at + 8))
            }
            0 => (8usize, (range.end - at) as u64),
            other => (8usize, other),
        };
        let size = usize::try_from(size).map_err(|_| ReadError::Truncated("box"))?;
        if size < header || at + size > range.end {
            return Err(ReadError::Truncated("box"));
        }
        out.push((kind, at + header..at + size));
        at += size;
    }
    Ok(out)
}

fn child(
    data: &[u8],
    range: Range<usize>,
    kind: &'static [u8; 4],
) -> Result<Range<usize>, ReadError> {
    boxes(data, range)?
        .into_iter()
        .find(|(found, _)| found == kind)
        .map(|(_, at)| at)
        // Safe to unwrap: every caller passes a four-character ASCII code.
        .ok_or(ReadError::Missing(
            std::str::from_utf8(kind).unwrap_or("box"),
        ))
}

fn read_track(data: &[u8], trak: Range<usize>) -> Result<Track, ReadError> {
    let tkhd = child(data, trak.clone(), b"tkhd")?;
    // Bounds-safe helpers throughout: this parses files off disk, so a truncated download must error, never panic.
    let (id, display) = read_tkhd(data, &tkhd);

    let mdia = child(data, trak.clone(), b"mdia")?;
    let mdhd = child(data, mdia.clone(), b"mdhd")?;
    let (timescale, duration) = read_mdhd(data, &mdhd);

    let hdlr = child(data, mdia.clone(), b"hdlr")?;
    let kind = match data.get(hdlr.start + 8..hdlr.start + 12) {
        Some(b"vide") => TrackKind::Video,
        Some(b"soun") => TrackKind::Audio,
        _ => TrackKind::Other,
    };

    let stbl = child(data, child(data, mdia, b"minf")?, b"stbl")?;
    let entry = sample_entry(data, &stbl)?;

    Ok(Track {
        id,
        kind,
        timescale,
        duration,
        format: entry.format,
        // The CODED size from the sample entry, falling back to the track header's display size; a decoder wants the coded one.
        width: if entry.width > 0 {
            entry.width
        } else {
            display.0
        },
        height: if entry.height > 0 {
            entry.height
        } else {
            display.1
        },
        sample_rate: entry.sample_rate,
        channels: entry.channels,
        decoder_config: entry.decoder_config,
        samples: read_samples(data, &stbl)?,
    })
}

/// Track id and the display size, which is 16.16 fixed point in the last eight
/// bytes of the header whichever version it is.
fn read_tkhd(data: &[u8], tkhd: &Range<usize>) -> (u32, (u16, u16)) {
    let version = data.get(tkhd.start).copied().unwrap_or(0);
    // The header grew when the times went 64-bit, so every field after them moves with it.
    let id_at = if version == 1 {
        tkhd.start + 20
    } else {
        tkhd.start + 12
    };
    (
        u32be(data, id_at),
        (u16be(data, tkhd.end - 8), u16be(data, tkhd.end - 4)),
    )
}

/// Timescale and duration. Version 1 stores the duration in 64 bits, which is
/// what a recording long enough to overflow 32 uses.
fn read_mdhd(data: &[u8], mdhd: &Range<usize>) -> (u32, u64) {
    match data.get(mdhd.start).copied().unwrap_or(0) {
        1 => (u32be(data, mdhd.start + 20), u64be(data, mdhd.start + 24)),
        _ => (
            u32be(data, mdhd.start + 12),
            u32be(data, mdhd.start + 16) as u64,
        ),
    }
}

#[derive(Default)]
struct SampleEntry {
    format: [u8; 4],
    width: u16,
    height: u16,
    sample_rate: u32,
    channels: u16,
    decoder_config: Vec<u8>,
}

fn sample_entry(data: &[u8], stbl: &Range<usize>) -> Result<SampleEntry, ReadError> {
    let stsd = child(data, stbl.clone(), b"stsd")?;
    // Four bytes of version and flags, then the entry count.
    let (kind, entry) = boxes(data, stsd.start + 8..stsd.end)?
        .into_iter()
        .next()
        .ok_or(ReadError::Missing("sample entry"))?;

    let mut out = SampleEntry {
        format: kind,
        ..Default::default()
    };
    // Both entry kinds start with six reserved bytes and a data-reference index.
    let body = entry.start + 8;
    let extensions = match &kind {
        b"avc1" | b"avc3" | b"hvc1" | b"hev1" => {
            out.width = u16be(data, body + 16);
            out.height = u16be(data, body + 18);
            body + 70
        }
        b"mp4a" => {
            out.channels = u16be(data, body + 8);
            // 16.16 fixed point, and the fraction is always zero in practice.
            out.sample_rate = u16be(data, body + 16) as u32;
            body + 20
        }
        _ => return Ok(out),
    };
    if extensions > entry.end {
        return Err(ReadError::Truncated("sample entry"));
    }

    for (kind, range) in boxes(data, extensions..entry.end)? {
        match &kind {
            b"avcC" | b"hvcC" => out.decoder_config = data.get(range).unwrap_or(&[]).to_vec(),
            b"esds" => out.decoder_config = audio_specific_config(data.get(range).unwrap_or(&[])),
            _ => {}
        }
    }
    Ok(out)
}

/// Digs the `AudioSpecificConfig` out of an `esds`, which nests it three
/// descriptors deep behind base-128 lengths.
fn audio_specific_config(esds: &[u8]) -> Vec<u8> {
    // Four bytes of version and flags, then the ES_Descriptor.
    let mut at = 4;
    let mut wanted = 0x03u8;
    loop {
        let Some(&tag) = esds.get(at) else {
            return Vec::new();
        };
        let Some((length, header)) = descriptor_length(&esds[at + 1..]) else {
            return Vec::new();
        };
        let body = at + 1 + header;
        let end = (body + length).min(esds.len());
        if tag != wanted {
            return Vec::new();
        }
        match tag {
            // ES_ID and flags, then the DecoderConfigDescriptor; a stream dependency or URL would add fields MP4 audio never writes.
            0x03 => {
                at = body + 3;
                wanted = 0x04;
            }
            // Object type, stream type, buffer size and bitrates, then the DecoderSpecificInfo.
            0x04 => {
                at = body + 13;
                wanted = 0x05;
            }
            0x05 => return esds[body..end].to_vec(),
            _ => return Vec::new(),
        }
        if at >= esds.len() {
            return Vec::new();
        }
    }
}

/// Descriptor lengths are base 128, seven bits a byte, high bit meaning more.
fn descriptor_length(data: &[u8]) -> Option<(usize, usize)> {
    let mut length = 0usize;
    for (index, byte) in data.iter().take(4).enumerate() {
        length = (length << 7) | (byte & 0x7f) as usize;
        if byte & 0x80 == 0 {
            return Some((length, index + 1));
        }
    }
    None
}

fn read_samples(data: &[u8], stbl: &Range<usize>) -> Result<Vec<SampleRef>, ReadError> {
    let children = boxes(data, stbl.clone())?;
    let find = |kind: &[u8; 4]| {
        children
            .iter()
            .find(|(found, _)| found == kind)
            .map(|(_, at)| at.clone())
    };

    let sizes = read_stsz(data, find(b"stsz").ok_or(ReadError::Missing("stsz"))?);
    let offsets = match find(b"stco") {
        Some(stco) => read_u32_table(data, stco)
            .into_iter()
            .map(u64::from)
            .collect(),
        None => read_co64(
            data,
            find(b"co64").ok_or(ReadError::Missing("chunk offsets"))?,
        ),
    };
    let runs = read_stsc(data, find(b"stsc").ok_or(ReadError::Missing("stsc"))?);
    let times = read_stts(
        data,
        find(b"stts").ok_or(ReadError::Missing("stts"))?,
        sizes.len(),
    );
    let composition = find(b"ctts")
        .map(|ctts| read_ctts(data, ctts, sizes.len()))
        .unwrap_or_else(|| vec![0; sizes.len()]);
    // No `stss` means every sample is a sync sample, which is how an audio track says so.
    let sync: Option<Vec<u32>> = find(b"stss").map(|stss| read_u32_table(data, stss));

    let mut out = Vec::with_capacity(sizes.len());
    let mut index = 0usize;
    let mut decode_time = 0u64;
    for (chunk, offset) in offsets.iter().enumerate() {
        let per_chunk = samples_in_chunk(&runs, chunk as u32 + 1);
        let mut at = *offset;
        for _ in 0..per_chunk {
            let Some(size) = sizes.get(index) else {
                break;
            };
            let duration = times.get(index).copied().unwrap_or(0);
            out.push(SampleRef {
                offset: at,
                size: *size,
                decode_time,
                duration,
                composition_offset: composition.get(index).copied().unwrap_or(0),
                // `stss` numbers samples from one.
                is_sync: match &sync {
                    Some(list) => list.binary_search(&(index as u32 + 1)).is_ok(),
                    None => true,
                },
            });
            at += *size as u64;
            decode_time += duration as u64;
            index += 1;
        }
    }
    Ok(out)
}

/// How many samples chunk number `chunk` holds, from the run-length `stsc`.
fn samples_in_chunk(runs: &[(u32, u32)], chunk: u32) -> u32 {
    runs.iter()
        .rev()
        .find(|(first, _)| *first <= chunk)
        .map(|(_, count)| *count)
        .unwrap_or(0)
}

fn read_stsz(data: &[u8], stsz: Range<usize>) -> Vec<u32> {
    let uniform = u32be(data, stsz.start + 4);
    let count = u32be(data, stsz.start + 8) as usize;
    if uniform != 0 {
        return vec![uniform; count];
    }
    (0..count)
        .map(|i| u32be(data, stsz.start + 12 + i * 4))
        .collect()
}

fn read_u32_table(data: &[u8], table: Range<usize>) -> Vec<u32> {
    let count = u32be(data, table.start + 4) as usize;
    (0..count)
        .map(|i| u32be(data, table.start + 8 + i * 4))
        .collect()
}

fn read_co64(data: &[u8], co64: Range<usize>) -> Vec<u64> {
    let count = u32be(data, co64.start + 4) as usize;
    (0..count)
        .map(|i| u64be(data, co64.start + 8 + i * 8))
        .collect()
}

/// `(first chunk, samples per chunk)` pairs, exactly as stored.
fn read_stsc(data: &[u8], stsc: Range<usize>) -> Vec<(u32, u32)> {
    let count = u32be(data, stsc.start + 4) as usize;
    (0..count)
        .map(|i| {
            let at = stsc.start + 8 + i * 12;
            (u32be(data, at), u32be(data, at + 4))
        })
        .collect()
}

fn read_stts(data: &[u8], stts: Range<usize>, samples: usize) -> Vec<u32> {
    let count = u32be(data, stts.start + 4) as usize;
    let mut out = Vec::with_capacity(samples);
    for i in 0..count {
        let at = stts.start + 8 + i * 8;
        let run = u32be(data, at) as usize;
        let delta = u32be(data, at + 4);
        out.extend(std::iter::repeat_n(
            delta,
            run.min(samples.saturating_sub(out.len())),
        ));
    }
    out.resize(samples, 0);
    out
}

fn read_ctts(data: &[u8], ctts: Range<usize>, samples: usize) -> Vec<i32> {
    let count = u32be(data, ctts.start + 4) as usize;
    let mut out = Vec::with_capacity(samples);
    for i in 0..count {
        let at = ctts.start + 8 + i * 8;
        let run = u32be(data, at) as usize;
        // Version 1 made the offset signed, which is how a stream with B frames
        // keeps its first presentation time at zero. Version 0 stores the same
        // bits unsigned and never uses the top one, so one cast reads both.
        let offset = u32be(data, at + 4) as i32;
        out.extend(std::iter::repeat_n(
            offset,
            run.min(samples.saturating_sub(out.len())),
        ));
    }
    out.resize(samples, 0);
    out
}

fn u16be(data: &[u8], at: usize) -> u16 {
    data.get(at..at + 2)
        .map(|b| u16::from_be_bytes([b[0], b[1]]))
        .unwrap_or(0)
}

fn u32be(data: &[u8], at: usize) -> u32 {
    data.get(at..at + 4)
        .map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
        .unwrap_or(0)
}

fn u64be(data: &[u8], at: usize) -> u64 {
    data.get(at..at + 8)
        .map(|b| u64::from_be_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]))
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descriptor_lengths_decode_base_128() {
        assert_eq!(descriptor_length(&[0x05]), Some((5, 1)));
        assert_eq!(descriptor_length(&[0x80, 0x05]), Some((5, 2)));
        assert_eq!(descriptor_length(&[0x81, 0x00]), Some((128, 2)));
        assert_eq!(descriptor_length(&[0x80, 0x80, 0x80, 0x80]), None);
    }

    #[test]
    fn a_chunk_takes_its_count_from_the_last_run_that_started() {
        let runs = [(1u32, 4u32), (5, 2)];
        assert_eq!(samples_in_chunk(&runs, 1), 4);
        assert_eq!(samples_in_chunk(&runs, 4), 4);
        assert_eq!(samples_in_chunk(&runs, 5), 2);
        assert_eq!(samples_in_chunk(&runs, 9), 2);
        assert_eq!(samples_in_chunk(&runs, 0), 0);
    }

    /// Files past 4 GiB carry 64-bit chunk offsets. Neither our writer nor
    /// ffmpeg will produce one at a size a test can hold, so the table is fed
    /// directly rather than through a fixture that would have to be 4 GiB.
    #[test]
    fn sixty_four_bit_chunk_offsets_read_as_offsets_not_as_pairs() {
        let mut co64 = vec![0u8; 8];
        co64[4..8].copy_from_slice(&2u32.to_be_bytes());
        co64.extend_from_slice(&0x1_0000_0000u64.to_be_bytes());
        co64.extend_from_slice(&0x1_0000_1234u64.to_be_bytes());
        let offsets = read_co64(&co64, 0..co64.len());
        assert_eq!(offsets, vec![0x1_0000_0000, 0x1_0000_1234]);
    }

    #[test]
    fn a_uniform_sample_size_expands_to_one_per_sample() {
        let mut stsz = vec![0u8; 4];
        stsz.extend_from_slice(&1024u32.to_be_bytes());
        stsz.extend_from_slice(&3u32.to_be_bytes());
        assert_eq!(read_stsz(&stsz, 0..stsz.len()), vec![1024, 1024, 1024]);
    }

    /// A recording long enough to overflow a 32-bit duration writes the version
    /// 1 header, where every field after the times has moved by eight bytes.
    /// Reading it as version 0 gives a timescale taken from the middle of a
    /// timestamp, which looks like a plausible number.
    #[test]
    fn a_version_one_media_header_reads_its_wider_fields() {
        let mut v0 = vec![0u8, 0, 0, 0];
        v0.extend_from_slice(&1u32.to_be_bytes()); // creation
        v0.extend_from_slice(&2u32.to_be_bytes()); // modification
        v0.extend_from_slice(&600u32.to_be_bytes()); // timescale
        v0.extend_from_slice(&1_200u32.to_be_bytes()); // duration
        assert_eq!(read_mdhd(&v0, &(0..v0.len())), (600, 1_200));

        let mut v1 = vec![1u8, 0, 0, 0];
        v1.extend_from_slice(&1u64.to_be_bytes());
        v1.extend_from_slice(&2u64.to_be_bytes());
        v1.extend_from_slice(&48_000u32.to_be_bytes());
        v1.extend_from_slice(&0x1_0000_0000u64.to_be_bytes());
        assert_eq!(read_mdhd(&v1, &(0..v1.len())), (48_000, 0x1_0000_0000));
    }

    #[test]
    fn a_version_one_track_header_finds_its_id() {
        let mut v1 = vec![1u8, 0, 0, 0];
        v1.extend_from_slice(&1u64.to_be_bytes());
        v1.extend_from_slice(&2u64.to_be_bytes());
        v1.extend_from_slice(&7u32.to_be_bytes()); // track id
        v1.resize(v1.len() + 40, 0);
        v1.extend_from_slice(&1_920u16.to_be_bytes());
        v1.extend_from_slice(&0u16.to_be_bytes());
        v1.extend_from_slice(&1_080u16.to_be_bytes());
        v1.extend_from_slice(&0u16.to_be_bytes());
        assert_eq!(read_tkhd(&v1, &(0..v1.len())), (7, (1_920, 1_080)));
    }

    /// Files past 4 GiB carry the size in a 64-bit field after the type, with 1
    /// in the 32-bit one. Reading that 1 as the size walks off into the payload.
    #[test]
    fn a_largesize_box_is_read_from_its_wider_field() {
        let mut data = Vec::new();
        data.extend_from_slice(&16u32.to_be_bytes());
        data.extend_from_slice(b"ftyp");
        data.extend_from_slice(b"isom    ");
        data.extend_from_slice(&1u32.to_be_bytes());
        data.extend_from_slice(b"mdat");
        data.extend_from_slice(&40u64.to_be_bytes());
        data.resize(16 + 40, 0xab);

        let found = boxes(&data, 0..data.len()).unwrap();
        assert_eq!(found.len(), 2);
        assert_eq!(&found[1].0, b"mdat");
        // The body starts after the sixteen-byte header and runs to the end.
        assert_eq!(found[1].1, 32..data.len());
    }

    /// A largesize header that does not fit in its parent is a truncated file,
    /// not a box of the remaining bytes.
    #[test]
    fn a_largesize_header_that_does_not_fit_is_refused() {
        let mut data = Vec::new();
        data.extend_from_slice(&1u32.to_be_bytes());
        data.extend_from_slice(b"mdat");
        data.extend_from_slice(&[0u8; 4]);
        assert_eq!(
            boxes(&data, 0..data.len()).unwrap_err(),
            ReadError::Truncated("largesize")
        );
    }

    /// Non-square pixels make the display size differ from the coded size, and
    /// a decoder needs the coded one.
    #[test]
    fn the_coded_size_comes_from_the_sample_entry_not_the_track_header() {
        // Six reserved bytes, the data reference index, and the predefined
        // block, then the dimensions, then out to where the extensions start.
        let mut body = vec![0u8; 24];
        body.extend_from_slice(&1_440u16.to_be_bytes());
        body.extend_from_slice(&1_080u16.to_be_bytes());
        body.resize(78, 0);
        let mut entry = ((body.len() + 8) as u32).to_be_bytes().to_vec();
        entry.extend_from_slice(b"avc1");
        entry.extend_from_slice(&body);

        let mut stsd_body = vec![0u8, 0, 0, 0];
        stsd_body.extend_from_slice(&1u32.to_be_bytes());
        stsd_body.extend_from_slice(&entry);
        let mut stbl = ((stsd_body.len() + 8) as u32).to_be_bytes().to_vec();
        stbl.extend_from_slice(b"stsd");
        stbl.extend_from_slice(&stsd_body);

        let read = sample_entry(&stbl, &(0..stbl.len())).expect("an entry");
        assert_eq!(&read.format, b"avc1");
        assert_eq!((read.width, read.height), (1_440, 1_080));
    }

    #[test]
    fn a_file_without_ftyp_is_not_an_mp4() {
        let mut data = Vec::new();
        data.extend_from_slice(&8u32.to_be_bytes());
        data.extend_from_slice(b"free");
        assert_eq!(Mp4Reader::new(&data).unwrap_err(), ReadError::NotMp4);
    }

    /// A box claiming to be longer than its parent is the shape a truncated
    /// download takes, and walking into it would read whatever follows.
    #[test]
    fn a_box_running_past_its_parent_is_refused() {
        let mut data = Vec::new();
        data.extend_from_slice(&16u32.to_be_bytes());
        data.extend_from_slice(b"ftyp");
        data.extend_from_slice(b"isom\0\0\0\0");
        data.extend_from_slice(&9_000u32.to_be_bytes());
        data.extend_from_slice(b"moov");
        assert_eq!(
            Mp4Reader::new(&data).unwrap_err(),
            ReadError::Truncated("box")
        );
    }

    #[test]
    fn a_zero_sized_box_runs_to_the_end_of_its_parent() {
        let mut data = Vec::new();
        data.extend_from_slice(&16u32.to_be_bytes());
        data.extend_from_slice(b"ftyp");
        data.extend_from_slice(b"isom\0\0\0\0");
        data.extend_from_slice(&0u32.to_be_bytes());
        data.extend_from_slice(b"moov");
        data.extend_from_slice(&[0u8; 8]);
        let found = boxes(&data, 0..data.len()).unwrap();
        assert_eq!(found.len(), 2);
        assert_eq!(found[1].1, 24..data.len());
    }
}
