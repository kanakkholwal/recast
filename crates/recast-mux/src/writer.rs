use crate::avc::AvcConfig;
use crate::boxes::BoxBuf;
use crate::track::{Sample, SampleTable};

/// The movie header timescale. Milliseconds, so a duration in it is readable
/// and every track timescale divides into it cleanly enough.
const MOVIE_TIMESCALE: u32 = 1000;

/// Written into `avc1`'s fixed-size compressor name field.
const COMPRESSOR: &[u8] = b"Recast";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VideoFormat {
    pub width: u16,
    pub height: u16,
    /// Ticks per second for this track's sample durations.
    pub timescale: u32,
}

/// Builds a progressive MP4: `moov` is emitted before `mdat`, so a player can
/// start on the first bytes instead of seeking to the end.
///
/// Samples are buffered because the sample tables carry offsets into `mdat`,
/// and those are only known once `moov`'s own size is. That is the same trade
/// `+faststart` makes, minus the second pass over the finished file.
#[derive(Debug)]
pub struct Mp4Writer {
    format: VideoFormat,
    config: AvcConfig,
    table: SampleTable,
    payload: Vec<u8>,
}

impl Mp4Writer {
    pub fn new(format: VideoFormat) -> Self {
        Self {
            format,
            config: AvcConfig::default(),
            table: SampleTable::default(),
            payload: Vec::new(),
        }
    }

    /// Parameter sets for `avcC`. Later calls replace earlier ones, so an
    /// encoder that repeats them on every keyframe costs nothing.
    pub fn set_avc_config(&mut self, config: AvcConfig) {
        if !config.is_empty() {
            self.config = config;
        }
    }

    /// Appends one access unit, already length-prefixed.
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
        self.table.push(Sample {
            offset: self.payload.len() as u64,
            size: data.len() as u32,
            duration,
            is_sync,
            composition_offset,
        });
        self.payload.extend_from_slice(data);
    }

    pub fn sample_count(&self) -> usize {
        self.table.samples.len()
    }

    /// The finished file. `None` when there is nothing to play: no samples, or
    /// no parameter sets to decode them with.
    pub fn finish(self) -> Option<Vec<u8>> {
        if self.table.is_empty() {
            return None;
        }
        let record = self.config.record()?;

        let ftyp = self.ftyp();
        // moov's own size depends on the chunk offsets, which depend on where
        // mdat lands, which depends on moov's size. Writing it once with a
        // placeholder settles the size, then again with the real offsets: the
        // second pass cannot change the size, because only offset VALUES move.
        let probe = self.moov(&record, 0);
        let mdat_header = mdat_header_len(self.payload.len());
        let payload_start = (ftyp.len() + probe.len() + mdat_header) as u64;
        let moov = self.moov(&record, payload_start);
        debug_assert_eq!(
            moov.len(),
            probe.len(),
            "the offset pass changed moov's size"
        );

        let mut out = Vec::with_capacity(ftyp.len() + moov.len() + mdat_header + self.payload.len());
        out.extend_from_slice(&ftyp);
        out.extend_from_slice(&moov);
        write_mdat_header(&mut out, self.payload.len());
        out.extend_from_slice(&self.payload);
        Some(out)
    }

    fn ftyp(&self) -> Vec<u8> {
        let mut buf = BoxBuf::new();
        buf.open(b"ftyp");
        buf.raw(b"isom").u32(512);
        for brand in [b"isom", b"iso2", b"avc1", b"mp41"] {
            buf.raw(brand);
        }
        buf.close();
        buf.into_bytes()
    }

    fn movie_duration(&self) -> u64 {
        let ticks = self.table.duration();
        ticks * MOVIE_TIMESCALE as u64 / self.format.timescale.max(1) as u64
    }

    fn moov(&self, record: &[u8], payload_start: u64) -> Vec<u8> {
        let mut buf = BoxBuf::new();
        buf.open(b"moov");

        buf.open_full(b"mvhd", 0, 0);
        buf.u32(0).u32(0).u32(MOVIE_TIMESCALE);
        buf.u32(self.movie_duration() as u32);
        buf.fixed16_16(1.0).u16(0x0100).zeros(10);
        buf.identity_matrix().zeros(24);
        // Next free track id. One video track today, so always 2.
        buf.u32(2);
        buf.close();

        self.trak(&mut buf, record, payload_start);
        buf.close();
        buf.into_bytes()
    }

    fn trak(&self, buf: &mut BoxBuf, record: &[u8], payload_start: u64) {
        buf.open(b"trak");

        // enabled | in movie | in preview
        buf.open_full(b"tkhd", 0, 0x00_0007);
        buf.u32(0).u32(0).u32(1).u32(0);
        buf.u32(self.movie_duration() as u32);
        buf.zeros(8).u16(0).u16(0);
        // Volume is zero for a video track.
        buf.u16(0).u16(0);
        buf.identity_matrix();
        buf.fixed16_16(self.format.width as f64);
        buf.fixed16_16(self.format.height as f64);
        buf.close();

        buf.open(b"mdia");
        buf.open_full(b"mdhd", 0, 0);
        buf.u32(0).u32(0).u32(self.format.timescale);
        buf.u32(self.table.duration() as u32);
        // Packed ISO-639-2/T "und", then pre_defined.
        buf.u16(0x55C4).u16(0);
        buf.close();

        buf.open_full(b"hdlr", 0, 0);
        buf.u32(0).raw(b"vide").zeros(12);
        buf.raw(b"VideoHandler\0");
        buf.close();

        buf.open(b"minf");
        buf.open_full(b"vmhd", 0, 1);
        buf.u16(0).zeros(6);
        buf.close();

        buf.open(b"dinf");
        buf.open_full(b"dref", 0, 0);
        buf.u32(1);
        // Flag 1 says the media is in this same file, so there is no URL.
        buf.open_full(b"url ", 0, 1);
        buf.close();
        buf.close();
        buf.close();

        buf.open(b"stbl");
        self.stsd(buf, record);
        self.table.write_stts(buf);
        self.table.write_stss(buf);
        self.table.write_ctts(buf);
        self.table.write_stsc(buf);
        self.table.write_stsz(buf);
        self.table.write_stco(buf, payload_start);
        buf.close();

        buf.close();
        buf.close();
        buf.close();
    }

    fn stsd(&self, buf: &mut BoxBuf, record: &[u8]) {
        buf.open_full(b"stsd", 0, 0);
        buf.u32(1);

        buf.open(b"avc1");
        buf.zeros(6).u16(1);
        buf.u16(0).u16(0).zeros(12);
        buf.u16(self.format.width).u16(self.format.height);
        // 72 dpi, as every muxer writes regardless of the real display size.
        buf.fixed16_16(72.0).fixed16_16(72.0);
        buf.u32(0).u16(1);
        // Pascal string in a fixed 32-byte field.
        buf.u8(COMPRESSOR.len() as u8).raw(COMPRESSOR);
        buf.zeros(31 - COMPRESSOR.len());
        buf.u16(0x0018).i16(-1);

        buf.open(b"avcC");
        buf.raw(record);
        buf.close();

        buf.close();
        buf.close();
    }
}

/// mdat needs the 64-bit header once its payload passes what a 32-bit size can
/// describe, and the header length feeds the offsets, so it is computed first.
fn mdat_header_len(payload: usize) -> usize {
    match payload as u64 + 8 > u32::MAX as u64 {
        true => 16,
        false => 8,
    }
}

fn write_mdat_header(out: &mut Vec<u8>, payload: usize) {
    if mdat_header_len(payload) == 16 {
        out.extend_from_slice(&1u32.to_be_bytes());
        out.extend_from_slice(b"mdat");
        out.extend_from_slice(&(payload as u64 + 16).to_be_bytes());
    } else {
        out.extend_from_slice(&(payload as u32 + 8).to_be_bytes());
        out.extend_from_slice(b"mdat");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::boxes::top_level_boxes;

    fn config() -> AvcConfig {
        AvcConfig {
            sps: vec![vec![0x67, 0x64, 0x00, 0x28, 0xAC, 0xD9]],
            pps: vec![vec![0x68, 0xEB, 0xE3, 0xCB]],
        }
    }

    fn writer() -> Mp4Writer {
        let mut w = Mp4Writer::new(VideoFormat {
            width: 640,
            height: 360,
            timescale: 30_000,
        });
        w.set_avc_config(config());
        w
    }

    /// Searches past `ftyp`, whose compatible-brands list contains the literal
    /// "avc1" and would otherwise be found instead of the sample entry.
    fn find(data: &[u8], kind: &[u8; 4]) -> Option<usize> {
        let ftyp = top_level_boxes(data).first().map(|(_, size)| *size)? as usize;
        data[ftyp..]
            .windows(4)
            .position(|w| w == kind)
            .map(|at| at + ftyp)
    }

    /// The whole point of the writer: a player must be able to start from the
    /// front of the file.
    #[test]
    fn moov_is_written_before_mdat() {
        let mut w = writer();
        w.push_sample(&[0, 0, 0, 2, 0x65, 1], 1000, true);
        let data = w.finish().expect("a file");
        let kinds: Vec<[u8; 4]> = top_level_boxes(&data).into_iter().map(|(k, _)| k).collect();
        assert_eq!(kinds, vec![*b"ftyp", *b"moov", *b"mdat"]);
    }

    #[test]
    fn the_boxes_cover_the_file_exactly() {
        let mut w = writer();
        for i in 0..5 {
            w.push_sample(&[0, 0, 0, 2, 0x65, i], 1000, i == 0);
        }
        let data = w.finish().expect("a file");
        let total: u64 = top_level_boxes(&data).iter().map(|(_, size)| size).sum();
        assert_eq!(total, data.len() as u64, "the boxes do not tile the file");
    }

    /// The chunk offset has to name the sample's real place in the finished
    /// file, or a player reads the wrong bytes.
    #[test]
    fn the_chunk_offset_points_at_the_first_sample() {
        let mut w = writer();
        let first = [0u8, 0, 0, 3, 0x65, 0xAA, 0xBB];
        w.push_sample(&first, 1000, true);
        w.push_sample(&[0, 0, 0, 2, 0x41, 9], 1000, false);
        let data = w.finish().expect("a file");

        let stco = find(&data, b"stco").expect("stco");
        let offset = u32::from_be_bytes(data[stco + 12..stco + 16].try_into().unwrap()) as usize;
        assert_eq!(&data[offset..offset + first.len()], &first);
    }

    #[test]
    fn the_mdat_payload_is_the_samples_in_order() {
        let mut w = writer();
        w.push_sample(&[1, 2, 3], 1000, true);
        w.push_sample(&[4, 5], 1000, false);
        let data = w.finish().expect("a file");
        let mdat = find(&data, b"mdat").expect("mdat");
        assert_eq!(&data[mdat + 4..mdat + 9], &[1, 2, 3, 4, 5]);
    }

    #[test]
    fn a_file_with_no_samples_is_not_written() {
        assert!(writer().finish().is_none());
    }

    /// Without parameter sets nothing can decode the samples, so a file would
    /// be worse than no file.
    #[test]
    fn a_file_without_parameter_sets_is_not_written() {
        let mut w = Mp4Writer::new(VideoFormat {
            width: 64,
            height: 64,
            timescale: 1000,
        });
        w.push_sample(&[0, 0, 0, 2, 0x65, 1], 100, true);
        assert!(w.finish().is_none());
    }

    #[test]
    fn the_avcc_record_is_embedded_in_the_sample_entry() {
        let mut w = writer();
        w.push_sample(&[0, 0, 0, 2, 0x65, 1], 1000, true);
        let data = w.finish().expect("a file");
        let avcc = find(&data, b"avcC").expect("avcC");
        let record = config().record().expect("a record");
        assert_eq!(&data[avcc + 4..avcc + 4 + record.len()], &record[..]);
    }

    #[test]
    fn the_track_dimensions_reach_the_sample_entry() {
        let mut w = writer();
        w.push_sample(&[0, 0, 0, 2, 0x65, 1], 1000, true);
        let data = w.finish().expect("a file");
        let avc1 = find(&data, b"avc1").expect("avc1");
        // Fields start after the four type bytes, then 24 of reserved and
        // pre_defined before the dimensions.
        let dims = avc1 + 4 + 24;
        assert_eq!(&data[dims..dims + 2], &640u16.to_be_bytes());
        assert_eq!(&data[dims + 2..dims + 4], &360u16.to_be_bytes());
    }

    /// The offsets are computed from a first pass over moov, so anything that
    /// changed its size between passes would corrupt every offset.
    #[test]
    fn the_offset_pass_does_not_change_the_header_size() {
        let mut w = writer();
        for i in 0..40 {
            w.push_sample(&[0, 0, 0, 2, 0x65, i], 1000, i % 10 == 0);
        }
        let data = w.finish().expect("a file");
        let boxes = top_level_boxes(&data);
        let total: u64 = boxes.iter().map(|(_, size)| size).sum();
        assert_eq!(total, data.len() as u64);

        // And the last sample still lands where its chunk says.
        let stco = find(&data, b"stco").expect("stco");
        let offset = u32::from_be_bytes(data[stco + 12..stco + 16].try_into().unwrap()) as usize;
        assert_eq!(&data[offset..offset + 6], &[0, 0, 0, 2, 0x65, 0]);
    }

    #[test]
    fn the_duration_is_the_sum_of_the_sample_durations() {
        let mut w = writer();
        for _ in 0..30 {
            w.push_sample(&[0, 0, 0, 2, 0x65, 1], 1000, true);
        }
        // 30 samples of 1000 ticks at 30000/s is one second.
        assert_eq!(w.movie_duration(), 1000);
    }

    #[test]
    fn a_large_payload_uses_the_sixty_four_bit_mdat_header() {
        assert_eq!(mdat_header_len(1024), 8);
        assert_eq!(mdat_header_len(u32::MAX as usize), 16);
    }
}
