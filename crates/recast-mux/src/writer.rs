use crate::avc::AvcConfig;
use crate::boxes::BoxBuf;
use crate::track::{Sample, SampleTable};

/// The movie header timescale. Milliseconds, so a duration in it is readable.
const MOVIE_TIMESCALE: u32 = 1000;

/// Written into the video sample entry's fixed-size compressor name field.
const COMPRESSOR: &[u8] = b"Recast";

/// How much of one track goes into a chunk before switching to the other.
/// Sample-by-sample interleaving is also correct but makes a chunk per sample,
/// and the offset table then costs more than the interleave saves.
const INTERLEAVE_SECONDS: f64 = 0.5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VideoFormat {
    pub width: u16,
    pub height: u16,
    /// Ticks per second for this track's sample durations.
    pub timescale: u32,
}

/// An AAC track. `config` is the `AudioSpecificConfig` the encoder reported;
/// without it no decoder can start, so the writer drops the track rather than
/// emit one that plays as silence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioFormat {
    pub sample_rate: u32,
    pub channels: u16,
    pub config: Vec<u8>,
}

#[derive(Debug, Default)]
struct TrackBuffer {
    table: SampleTable,
    payloads: Vec<Vec<u8>>,
}

impl TrackBuffer {
    fn push(&mut self, data: &[u8], duration: u32, is_sync: bool, composition_offset: i32) {
        self.table.push(Sample {
            // Filled in during layout: a sample's place in `mdat` is not known
            // until both tracks have been interleaved.
            offset: 0,
            size: data.len() as u32,
            duration,
            is_sync,
            composition_offset,
        });
        self.payloads.push(data.to_vec());
    }

    fn is_empty(&self) -> bool {
        self.table.is_empty()
    }
}

/// Builds a progressive MP4: `moov` is emitted before `mdat`, so a player can
/// start on the first bytes instead of seeking to the end.
///
/// Samples are buffered because the sample tables carry offsets into `mdat`,
/// and those are only known once `moov`'s own size is. That is the same trade
/// `+faststart` makes, minus the second pass over the finished file.
#[derive(Debug)]
pub struct Mp4Writer {
    video_format: VideoFormat,
    avc: AvcConfig,
    video: TrackBuffer,
    audio_format: Option<AudioFormat>,
    audio: TrackBuffer,
}

impl Mp4Writer {
    pub fn new(format: VideoFormat) -> Self {
        Self {
            video_format: format,
            avc: AvcConfig::default(),
            video: TrackBuffer::default(),
            audio_format: None,
            audio: TrackBuffer::default(),
        }
    }

    /// Parameter sets for `avcC`. Later calls replace earlier ones, so an
    /// encoder that repeats them on every keyframe costs nothing.
    pub fn set_avc_config(&mut self, config: AvcConfig) {
        if !config.is_empty() {
            self.avc = config;
        }
    }

    /// Adds an audio track. Without this the file is video only.
    pub fn set_audio_format(&mut self, format: AudioFormat) {
        self.audio_format = Some(format);
    }

    /// Appends one video access unit, already length-prefixed.
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
        self.video.push(data, duration, is_sync, composition_offset);
    }

    /// Appends one raw AAC frame. `duration` is in samples, which for AAC-LC is
    /// 1024 per frame. Every audio frame is a sync point.
    pub fn push_audio_sample(&mut self, data: &[u8], duration: u32) {
        self.audio.push(data, duration, true, 0);
    }

    pub fn sample_count(&self) -> usize {
        self.video.table.samples.len()
    }

    pub fn audio_sample_count(&self) -> usize {
        self.audio.table.samples.len()
    }

    /// The finished file. `None` when there is nothing to play: no video
    /// samples, or no parameter sets to decode them with.
    pub fn finish(mut self) -> Option<Vec<u8>> {
        if self.video.is_empty() {
            return None;
        }
        let record = self.avc.record()?;
        // An audio track we cannot describe is dropped rather than written: a
        // silent track looks like a mix bug, which is harder to chase than a
        // missing one.
        if self.audio_format.as_ref().is_some_and(|f| f.config.is_empty()) {
            self.audio_format = None;
            self.audio = TrackBuffer::default();
        }

        let payload = self.interleave();
        let ftyp = self.ftyp();
        // moov's own size depends on the chunk offsets, which depend on where
        // mdat lands, which depends on moov's size. Writing it once with a
        // placeholder settles the size, then again with the real offsets: the
        // second pass cannot change the size, because only offset VALUES move.
        let probe = self.moov(&record, 0);
        let mdat_header = mdat_header_len(payload.len());
        let payload_start = (ftyp.len() + probe.len() + mdat_header) as u64;
        let moov = self.moov(&record, payload_start);
        debug_assert_eq!(moov.len(), probe.len(), "the offset pass changed moov's size");

        let mut out = Vec::with_capacity(ftyp.len() + moov.len() + mdat_header + payload.len());
        out.extend_from_slice(&ftyp);
        out.extend_from_slice(&moov);
        write_mdat_header(&mut out, payload.len());
        out.extend_from_slice(&payload);
        Some(out)
    }

    /// Lays both tracks into one payload in rough time order, filling in each
    /// sample's offset as it goes.
    fn interleave(&mut self) -> Vec<u8> {
        let mut payload = Vec::new();
        let (mut video_at, mut audio_at) = (0usize, 0usize);
        let (mut video_time, mut audio_time) = (0.0f64, 0.0f64);
        let video_rate = self.video_format.timescale.max(1) as f64;
        let audio_rate = self
            .audio_format
            .as_ref()
            .map(|f| f.sample_rate.max(1) as f64)
            .unwrap_or(1.0);

        while video_at < self.video.payloads.len() || audio_at < self.audio.payloads.len() {
            // Whichever track is further behind goes next, so a player reading
            // forward always has both streams to hand.
            let take_video = audio_at >= self.audio.payloads.len()
                || (video_at < self.video.payloads.len() && video_time <= audio_time);
            if take_video {
                let boundary = video_time + INTERLEAVE_SECONDS;
                while video_at < self.video.payloads.len() && video_time < boundary {
                    self.video.table.samples[video_at].offset = payload.len() as u64;
                    payload.extend_from_slice(&self.video.payloads[video_at]);
                    video_time += self.video.table.samples[video_at].duration as f64 / video_rate;
                    video_at += 1;
                }
            } else {
                let boundary = audio_time + INTERLEAVE_SECONDS;
                while audio_at < self.audio.payloads.len() && audio_time < boundary {
                    self.audio.table.samples[audio_at].offset = payload.len() as u64;
                    payload.extend_from_slice(&self.audio.payloads[audio_at]);
                    audio_time += self.audio.table.samples[audio_at].duration as f64 / audio_rate;
                    audio_at += 1;
                }
            }
        }
        payload
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
        let video = self.video.table.duration() * MOVIE_TIMESCALE as u64
            / self.video_format.timescale.max(1) as u64;
        let audio = match &self.audio_format {
            Some(format) => {
                self.audio.table.duration() * MOVIE_TIMESCALE as u64
                    / format.sample_rate.max(1) as u64
            }
            None => 0,
        };
        video.max(audio)
    }

    fn moov(&self, record: &[u8], payload_start: u64) -> Vec<u8> {
        let mut buf = BoxBuf::new();
        buf.open(b"moov");

        buf.open_full(b"mvhd", 0, 0);
        buf.u32(0).u32(0).u32(MOVIE_TIMESCALE);
        buf.u32(self.movie_duration() as u32);
        buf.fixed16_16(1.0).u16(0x0100).zeros(10);
        buf.identity_matrix().zeros(24);
        buf.u32(self.next_track_id());
        buf.close();

        self.video_trak(&mut buf, record, payload_start);
        if self.audio_format.is_some() {
            self.audio_trak(&mut buf, payload_start);
        }
        buf.close();
        buf.into_bytes()
    }

    fn next_track_id(&self) -> u32 {
        match self.audio_format.is_some() {
            true => 3,
            false => 2,
        }
    }

    /// The parts of `trak` that do not depend on the media kind.
    fn track_header(&self, buf: &mut BoxBuf, id: u32, video: bool) {
        // enabled | in movie | in preview
        buf.open_full(b"tkhd", 0, 0x00_0007);
        buf.u32(0).u32(0).u32(id).u32(0);
        buf.u32(self.movie_duration() as u32);
        buf.zeros(8).u16(0).u16(0);
        // Volume is full for audio and zero for video.
        buf.u16(match video {
            true => 0,
            false => 0x0100,
        })
        .u16(0);
        buf.identity_matrix();
        match video {
            true => {
                buf.fixed16_16(self.video_format.width as f64);
                buf.fixed16_16(self.video_format.height as f64);
            }
            false => {
                buf.u32(0).u32(0);
            }
        }
        buf.close();
    }

    fn media_header(&self, buf: &mut BoxBuf, timescale: u32, duration: u64, handler: &[u8; 4]) {
        buf.open_full(b"mdhd", 0, 0);
        buf.u32(0).u32(0).u32(timescale);
        buf.u32(duration as u32);
        // Packed ISO-639-2/T "und", then pre_defined.
        buf.u16(0x55C4).u16(0);
        buf.close();

        buf.open_full(b"hdlr", 0, 0);
        buf.u32(0).raw(handler).zeros(12);
        buf.raw(match handler {
            b"soun" => b"SoundHandler\0".as_slice(),
            _ => b"VideoHandler\0".as_slice(),
        });
        buf.close();
    }

    /// `dinf` says the media is in this same file, so there is no URL.
    fn data_information(&self, buf: &mut BoxBuf) {
        buf.open(b"dinf");
        buf.open_full(b"dref", 0, 0);
        buf.u32(1);
        buf.open_full(b"url ", 0, 1);
        buf.close();
        buf.close();
        buf.close();
    }

    fn video_trak(&self, buf: &mut BoxBuf, record: &[u8], payload_start: u64) {
        buf.open(b"trak");
        self.track_header(buf, 1, true);

        buf.open(b"mdia");
        self.media_header(
            buf,
            self.video_format.timescale,
            self.video.table.duration(),
            b"vide",
        );

        buf.open(b"minf");
        buf.open_full(b"vmhd", 0, 1);
        buf.u16(0).zeros(6);
        buf.close();
        self.data_information(buf);

        buf.open(b"stbl");
        self.avc_sample_entry(buf, record);
        self.video.table.write_stts(buf);
        self.video.table.write_stss(buf);
        self.video.table.write_ctts(buf);
        self.video.table.write_stsc(buf);
        self.video.table.write_stsz(buf);
        self.video.table.write_stco(buf, payload_start);
        buf.close();

        buf.close();
        buf.close();
        buf.close();
    }

    fn audio_trak(&self, buf: &mut BoxBuf, payload_start: u64) {
        let Some(format) = &self.audio_format else {
            return;
        };
        buf.open(b"trak");
        self.track_header(buf, 2, false);

        buf.open(b"mdia");
        self.media_header(buf, format.sample_rate, self.audio.table.duration(), b"soun");

        buf.open(b"minf");
        buf.open_full(b"smhd", 0, 0);
        buf.u16(0).u16(0);
        buf.close();
        self.data_information(buf);

        buf.open(b"stbl");
        self.aac_sample_entry(buf, format);
        self.audio.table.write_stts(buf);
        // Every AAC frame is a sync point, so `stss` is omitted by design.
        self.audio.table.write_stsc(buf);
        self.audio.table.write_stsz(buf);
        self.audio.table.write_stco(buf, payload_start);
        buf.close();

        buf.close();
        buf.close();
        buf.close();
    }

    fn avc_sample_entry(&self, buf: &mut BoxBuf, record: &[u8]) {
        buf.open_full(b"stsd", 0, 0);
        buf.u32(1);

        buf.open(b"avc1");
        buf.zeros(6).u16(1);
        buf.u16(0).u16(0).zeros(12);
        buf.u16(self.video_format.width).u16(self.video_format.height);
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

    fn aac_sample_entry(&self, buf: &mut BoxBuf, format: &AudioFormat) {
        buf.open_full(b"stsd", 0, 0);
        buf.u32(1);

        buf.open(b"mp4a");
        buf.zeros(6).u16(1);
        // version, revision, vendor
        buf.u16(0).u16(0).u32(0);
        buf.u16(format.channels).u16(16);
        buf.u16(0).u16(0);
        // 16.16 fixed, so the rate sits in the high half.
        buf.u32(format.sample_rate << 16);
        write_esds(buf, &format.config);
        buf.close();

        buf.close();
    }
}

/// The `esds` box: an MPEG-4 elementary stream descriptor wrapping the
/// `AudioSpecificConfig`, which is what tells a decoder the profile, rate and
/// channel layout.
///
/// Built inside out. Every parser we can reach is lenient about descriptor
/// lengths, so computing them by hand gives a file that plays here and breaks
/// elsewhere; nesting finished byte strings makes each length exact by
/// construction.
fn write_esds(buf: &mut BoxBuf, config: &[u8]) {
    // DecoderSpecificInfo: the AudioSpecificConfig itself.
    let specific = descriptor(0x05, config);

    // DecoderConfigDescriptor: MPEG-4 audio (0x40) in an audio stream (0x15).
    // The buffer size and bitrates are zero, which is legal and unread.
    let mut decoder = vec![0x40, 0x15, 0, 0, 0];
    decoder.extend_from_slice(&[0; 8]);
    decoder.extend_from_slice(&specific);
    let decoder = descriptor(0x04, &decoder);

    // SLConfigDescriptor, predefined 2: the MP4 defaults.
    let sl = descriptor(0x06, &[0x02]);

    // ES_Descriptor: no stream dependency, no URL, no OCR.
    let mut es = vec![0, 0, 0];
    es.extend_from_slice(&decoder);
    es.extend_from_slice(&sl);

    buf.open_full(b"esds", 0, 0);
    buf.raw(&descriptor(0x03, &es));
    buf.close();
}

/// One MPEG-4 descriptor: a tag, its length, then the payload.
fn descriptor(tag: u8, payload: &[u8]) -> Vec<u8> {
    let mut out = vec![tag];
    write_descriptor_len(&mut out, payload.len());
    out.extend_from_slice(payload);
    out
}

/// Descriptor lengths use a base-128 continuation encoding. Ours are always
/// short, but writing the minimal form keeps the sizes honest.
fn write_descriptor_len(out: &mut Vec<u8>, length: usize) {
    let mut stack = [0u8; 4];
    let mut count = 0;
    let mut remaining = length;
    loop {
        stack[count] = (remaining & 0x7F) as u8;
        count += 1;
        remaining >>= 7;
        if remaining == 0 || count == 4 {
            break;
        }
    }
    for i in (0..count).rev() {
        out.push(stack[i] | if i == 0 { 0 } else { 0x80 });
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

    fn audio() -> AudioFormat {
        AudioFormat {
            sample_rate: 48_000,
            channels: 2,
            // An AAC-LC 48 kHz stereo AudioSpecificConfig.
            config: vec![0x11, 0x90],
        }
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

    // --- Audio ---

    fn with_audio(seconds: u32) -> Mp4Writer {
        let mut w = writer();
        w.set_audio_format(audio());
        for i in 0..30 * seconds {
            // 30 fps against the 30000 timescale.
            w.push_sample(&[0, 0, 0, 2, 0x65, i as u8], 1000, i % 30 == 0);
        }
        // 1024 samples per AAC-LC frame, so about 47 frames a second at 48 kHz.
        for _ in 0..47 * seconds {
            w.push_audio_sample(&[0xDE, 0xAD, 0xBE, 0xEF], 1024);
        }
        w
    }

    #[test]
    fn an_audio_track_adds_a_second_trak_and_sample_entry() {
        let data = with_audio(1).finish().expect("a file");
        assert!(find(&data, b"mp4a").is_some(), "no audio sample entry");
        assert!(find(&data, b"esds").is_some(), "no decoder config");
        assert!(find(&data, b"smhd").is_some(), "no sound media header");
        let traks = data.windows(4).filter(|w| *w == b"trak").count();
        assert_eq!(traks, 2, "expected a video and an audio track");
    }

    /// Players read the rate from the `AudioSpecificConfig`, so a wrong value
    /// in the sample entry survives playback and shows up only in tools. It is
    /// still wrong, and only a byte check catches it.
    #[test]
    fn the_sample_entry_carries_the_rate_and_channels() {
        let data = with_audio(1).finish().expect("a file");
        let mp4a = find(&data, b"mp4a").expect("mp4a");
        // After the type: 6 reserved, 2 data-reference, 8 version/vendor.
        let fields = mp4a + 4 + 16;
        assert_eq!(&data[fields..fields + 2], &2u16.to_be_bytes(), "channels");
        assert_eq!(&data[fields + 2..fields + 4], &16u16.to_be_bytes(), "bits");
        let rate = u32::from_be_bytes(data[fields + 8..fields + 12].try_into().unwrap());
        assert_eq!(rate >> 16, 48_000, "sample rate");
    }

    /// A silent track looks like a mix bug; refusing to describe one is better.
    #[test]
    fn an_audio_track_with_no_decoder_config_is_dropped() {
        let mut w = writer();
        w.set_audio_format(AudioFormat {
            config: Vec::new(),
            ..audio()
        });
        w.push_sample(&[0, 0, 0, 2, 0x65, 1], 1000, true);
        w.push_audio_sample(&[1, 2, 3, 4], 1024);
        let data = w.finish().expect("a file");
        assert!(
            find(&data, b"mp4a").is_none(),
            "an undescribed track was written"
        );
    }

    /// Both tracks share one `mdat`, so every offset has to survive the
    /// interleave. A stale one points a player at the other stream.
    #[test]
    fn both_tracks_offsets_point_at_their_own_bytes() {
        let mut w = writer();
        w.set_audio_format(audio());
        let video_sample = [0u8, 0, 0, 3, 0x65, 0x11, 0x22];
        let audio_sample = [0xAAu8, 0xBB, 0xCC, 0xDD];
        w.push_sample(&video_sample, 1000, true);
        w.push_audio_sample(&audio_sample, 1024);
        let data = w.finish().expect("a file");

        let offsets: Vec<usize> = data
            .windows(4)
            .enumerate()
            .filter(|(_, w)| *w == b"stco")
            .map(|(at, _)| u32::from_be_bytes(data[at + 12..at + 16].try_into().unwrap()) as usize)
            .collect();
        assert_eq!(offsets.len(), 2, "expected an offset table per track");
        assert_eq!(
            &data[offsets[0]..offsets[0] + video_sample.len()],
            &video_sample
        );
        assert_eq!(
            &data[offsets[1]..offsets[1] + audio_sample.len()],
            &audio_sample
        );
    }

    /// Writing one whole track then the other forces a player to seek between
    /// streams. Samples close in time have to be close in the file.
    #[test]
    fn the_two_tracks_are_interleaved_rather_than_written_end_to_end() {
        let data = with_audio(4).finish().expect("a file");
        let stco = find(&data, b"stco").expect("stco");
        let entries = u32::from_be_bytes(data[stco + 12..stco + 16].try_into().unwrap());
        assert!(
            entries >= 4,
            "video landed in {entries} chunks, so it was not interleaved"
        );
    }

    #[test]
    fn the_movie_duration_covers_the_longer_track() {
        let mut w = writer();
        w.set_audio_format(audio());
        // One video frame, but two seconds of audio.
        w.push_sample(&[0, 0, 0, 2, 0x65, 1], 1000, true);
        for _ in 0..94 {
            w.push_audio_sample(&[1, 2, 3, 4], 1024);
        }
        assert!(
            w.movie_duration() > 1900,
            "the duration stopped at the video track: {}",
            w.movie_duration()
        );
    }

    /// Reads one descriptor at `at`, returning its tag, the offset its payload
    /// starts at, and the length it declares.
    fn descriptor(data: &[u8], at: usize) -> (u8, usize, usize) {
        let tag = data[at];
        let mut length = 0usize;
        let mut cursor = at + 1;
        loop {
            let byte = data[cursor];
            length = (length << 7) | (byte & 0x7F) as usize;
            cursor += 1;
            if byte & 0x80 == 0 {
                break;
            }
        }
        (tag, cursor, length)
    }

    /// Every parser we can reach is lenient about descriptor lengths, so a
    /// wrong one plays fine here and breaks on somebody else's machine. The
    /// invariant is checkable without a parser: a declared length has to cover
    /// exactly the bytes that follow.
    #[test]
    fn every_esds_descriptor_declares_the_length_it_actually_spans() {
        let data = with_audio(1).finish().expect("a file");
        let esds = find(&data, b"esds").expect("esds");
        // Past the type and the full-box version and flags.
        let root = esds + 4 + 4;
        let end = {
            let size = u32::from_be_bytes(data[esds - 4..esds].try_into().unwrap()) as usize;
            esds - 4 + size
        };

        let (tag, payload, length) = descriptor(&data, root);
        assert_eq!(tag, 0x03, "the root is not an ES_Descriptor");
        assert_eq!(payload + length, end, "ES_Descriptor overruns the box");

        // ES_ID and flags, then the decoder config.
        let (tag, payload, length) = descriptor(&data, payload + 3);
        assert_eq!(tag, 0x04, "expected a DecoderConfigDescriptor");
        let config_end = payload + length;
        assert!(config_end <= end, "DecoderConfigDescriptor overruns");

        // Thirteen fixed bytes, then the AudioSpecificConfig.
        let (tag, asc, asc_len) = descriptor(&data, payload + 13);
        assert_eq!(tag, 0x05, "expected a DecoderSpecificInfo");
        assert_eq!(
            asc + asc_len,
            config_end,
            "the decoder config length does not cover its own AudioSpecificConfig"
        );
        assert_eq!(&data[asc..asc + asc_len], &audio().config[..]);

        let (tag, payload, length) = descriptor(&data, config_end);
        assert_eq!(tag, 0x06, "expected an SLConfigDescriptor");
        assert_eq!(payload + length, end, "the descriptors do not fill the box");
    }

    #[test]
    fn a_descriptor_length_uses_the_continuation_form_past_a_hundred_and_twenty_seven() {
        let mut bytes = Vec::new();
        write_descriptor_len(&mut bytes, 5);
        write_descriptor_len(&mut bytes, 200);
        assert_eq!(bytes[0], 5);
        assert_eq!(&bytes[1..3], &[0x81, 0x48], "200 is 0x81 0x48 in base 128");
    }
}
