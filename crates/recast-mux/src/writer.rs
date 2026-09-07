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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct VideoFormat {
    pub width: u16,
    pub height: u16,
    /// Ticks per second for this track's sample durations.
    pub timescale: u32,
}

/// An AAC track. `config` is the `AudioSpecificConfig` the encoder reported; without it no decoder can start, so the writer drops the track rather than emit one that plays as silence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioFormat {
    pub sample_rate: u32,
    pub channels: u16,
    pub config: Vec<u8>,
}

/// Which track a sample in the interleave order belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Lane {
    Video,
    Audio,
}

/// Where one track's sample bytes wait for layout. Spilled, they are appended
/// in push order and read back in the same order, so no index is needed beyond
/// the sizes the sample table already carries.
#[derive(Debug)]
enum Payloads {
    Memory(Vec<Vec<u8>>),
    /// The path is kept so `Drop` can remove it: Windows refuses to unlink a
    /// file that is still open, so it cannot be dropped at creation the way it
    /// would be on Unix.
    Spilled {
        file: std::fs::File,
        path: std::path::PathBuf,
    },
}

impl Default for Payloads {
    fn default() -> Self {
        Self::Memory(Vec::new())
    }
}

#[derive(Debug, Default)]
struct TrackBuffer {
    table: SampleTable,
    payloads: Payloads,
    /// Set once a spilled file has been rewound for reading.
    reading: bool,
    /// The first write that failed, reported at `finish` rather than swallowed.
    spill_error: Option<std::io::Error>,
}

impl Drop for TrackBuffer {
    fn drop(&mut self) {
        if let Payloads::Spilled { path, .. } = &self.payloads {
            let _ = std::fs::remove_file(path);
        }
    }
}

impl TrackBuffer {
    fn push(&mut self, data: &[u8], duration: u32, is_sync: bool, composition_offset: i32) {
        self.table.push(Sample {
            // Filled in during layout: a sample's place in `mdat` isn't known until both tracks are interleaved.
            offset: 0,
            size: data.len() as u32,
            duration,
            is_sync,
            composition_offset,
        });
        match &mut self.payloads {
            Payloads::Memory(held) => held.push(data.to_vec()),
            // A write that fails leaves the table describing bytes that are not there, which `finish` reports rather than writing a torn file.
            Payloads::Spilled { file, .. } => {
                if let Err(e) = std::io::Write::write_all(file, data) {
                    self.spill_error = Some(e);
                }
            }
        }
    }

    fn is_empty(&self) -> bool {
        self.table.is_empty()
    }

    /// Copies sample `index` into `out`, reading forwards for a spilled track.
    fn write_sample<W: std::io::Write>(
        &mut self,
        index: usize,
        out: &mut W,
        scratch: &mut Vec<u8>,
    ) -> std::io::Result<()> {
        let size = self.table.samples[index].size as usize;
        match &mut self.payloads {
            Payloads::Memory(held) => out.write_all(&held[index]),
            Payloads::Spilled { file, .. } => {
                if !self.reading {
                    std::io::Seek::seek(file, std::io::SeekFrom::Start(0))?;
                    self.reading = true;
                }
                scratch.resize(size, 0);
                std::io::Read::read_exact(file, scratch)?;
                out.write_all(scratch)
            }
        }
    }
}

/// Builds a progressive MP4 with `moov` before `mdat`, so a player starts on the first bytes instead of seeking to the end.
/// Samples are buffered because the sample tables carry `mdat` offsets that are known only once `moov` is sized: `+faststart`'s trade, minus the second pass.
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

    /// Hold sample bytes in `dir` instead of in memory, for a mux whose payload
    /// is too big to keep: a 30-minute export is gigabytes of it.
    ///
    /// Call before the first sample. The files are unlinked when the writer
    /// drops, so a cancelled export leaves nothing behind.
    pub fn spill_to(&mut self, dir: &std::path::Path) -> std::io::Result<()> {
        std::fs::create_dir_all(dir)?;
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default();
        for (buffer, name) in [(&mut self.video, "v"), (&mut self.audio, "a")] {
            let path = dir.join(format!("recast-mux-{stamp}-{name}.bin"));
            let file = std::fs::OpenOptions::new()
                .create(true)
                .read(true)
                .write(true)
                .truncate(true)
                .open(&path)?;
            buffer.payloads = Payloads::Spilled { file, path };
        }
        Ok(())
    }

    /// The header and the order its offsets describe. `None` when there is
    /// nothing to play: no video samples, or no parameter sets to decode them.
    fn lay_out(&mut self) -> Option<(Vec<u8>, Vec<Lane>)> {
        if self.video.is_empty() {
            return None;
        }
        let record = self.avc.record()?;
        // An undescribed or sample-less audio track is dropped: a silent track looks like a mix bug and is harder to chase.
        let undescribed = self
            .audio_format
            .as_ref()
            .is_some_and(|f| f.config.is_empty());
        if undescribed || self.audio.table.is_empty() {
            self.audio_format = None;
            self.audio = TrackBuffer::default();
        }

        let order = self.interleave();
        let payload_len = self.payload_len();
        let ftyp = self.ftyp();
        // moov's size depends on offsets that depend on where mdat lands, so a placeholder pass settles the size first.
        let mdat_header = mdat_header_len(payload_len);
        // Settled before the offsets are known: a payload near 4 GiB would otherwise pick stco in the probe and co64 in the real pass, growing moov and leaving every offset short by that growth.
        let widest = (ftyp.len() + self.moov(&record, 0, true).len() + mdat_header) as u64;
        let force_64 = widest.saturating_add(payload_len as u64) > u64::from(u32::MAX);
        let probe = self.moov(&record, 0, force_64);
        let payload_start = (ftyp.len() + probe.len() + mdat_header) as u64;
        let moov = self.moov(&record, payload_start, force_64);
        debug_assert_eq!(
            moov.len(),
            probe.len(),
            "the offset pass changed moov's size"
        );

        let mut header = Vec::with_capacity(ftyp.len() + moov.len() + mdat_header);
        header.extend_from_slice(&ftyp);
        header.extend_from_slice(&moov);
        write_mdat_header(&mut header, payload_len);
        Some((header, order))
    }

    /// The finished file. `None` when there is nothing to play: no video
    /// samples, or no parameter sets to decode them with.
    pub fn finish(mut self) -> Option<Vec<u8>> {
        let (header, order) = self.lay_out()?;
        let mut out = Vec::with_capacity(header.len() + self.payload_len());
        out.extend_from_slice(&header);
        self.write_payload(&mut out, &order).ok()?;
        Some(out)
    }

    /// The finished file, streamed into `out` rather than assembled in memory.
    ///
    /// A whole export otherwise sits in RAM twice over at the moment it is
    /// written: once as the samples and once as the file built from them.
    pub fn finish_into<W: std::io::Write>(mut self, out: &mut W) -> std::io::Result<bool> {
        let Some((header, order)) = self.lay_out() else {
            return Ok(false);
        };
        out.write_all(&header)?;
        self.write_payload(out, &order)?;
        Ok(true)
    }

    /// Copies every sample into `out` in interleave order. Each track is read
    /// strictly forwards, which is what lets the payloads live anywhere.
    fn write_payload<W: std::io::Write>(
        &mut self,
        out: &mut W,
        order: &[Lane],
    ) -> std::io::Result<()> {
        for buffer in [&mut self.video, &mut self.audio] {
            if let Some(e) = buffer.spill_error.take() {
                return Err(e);
            }
        }
        let mut scratch = Vec::new();
        let (mut video_at, mut audio_at) = (0usize, 0usize);
        for lane in order {
            let (buffer, at) = match lane {
                Lane::Video => (&mut self.video, &mut video_at),
                Lane::Audio => (&mut self.audio, &mut audio_at),
            };
            buffer.write_sample(*at, out, &mut scratch)?;
            *at += 1;
        }
        Ok(())
    }

    /// Which track each sample of `mdat` comes from, in file order, filling in
    /// every sample's offset as it goes.
    ///
    /// An order rather than a payload: the bytes are copied once, straight into
    /// the output, instead of into an interleave buffer and out of it again.
    fn interleave(&mut self) -> Vec<Lane> {
        let mut order = Vec::with_capacity(self.video.table.len() + self.audio.table.len());
        let mut at = 0u64;
        let (mut video_at, mut audio_at) = (0usize, 0usize);
        let (mut video_time, mut audio_time) = (0.0f64, 0.0f64);
        let video_rate = self.video_format.timescale.max(1) as f64;
        let audio_rate = self
            .audio_format
            .as_ref()
            .map(|f| f.sample_rate.max(1) as f64)
            .unwrap_or(1.0);

        while video_at < self.video.table.len() || audio_at < self.audio.table.len() {
            // Whichever track is further behind goes next, so a player reading forward always has both streams to hand.
            let take_video = audio_at >= self.audio.table.len()
                || (video_at < self.video.table.len() && video_time <= audio_time);
            if take_video {
                let boundary = video_time + INTERLEAVE_SECONDS;
                while video_at < self.video.table.len() && video_time < boundary {
                    let sample = &mut self.video.table.samples[video_at];
                    sample.offset = at;
                    at += u64::from(sample.size);
                    video_time += f64::from(sample.duration) / video_rate;
                    order.push(Lane::Video);
                    video_at += 1;
                }
            } else {
                let boundary = audio_time + INTERLEAVE_SECONDS;
                while audio_at < self.audio.table.len() && audio_time < boundary {
                    let sample = &mut self.audio.table.samples[audio_at];
                    sample.offset = at;
                    at += u64::from(sample.size);
                    audio_time += f64::from(sample.duration) / audio_rate;
                    order.push(Lane::Audio);
                    audio_at += 1;
                }
            }
        }
        order
    }

    /// The `mdat` payload's total length, which the header needs before a byte
    /// of it is written.
    fn payload_len(&self) -> usize {
        let sum = |t: &SampleTable| t.samples.iter().map(|s| s.size as usize).sum::<usize>();
        sum(&self.video.table) + sum(&self.audio.table)
    }

    fn ftyp(&self) -> Vec<u8> {
        ftyp(false)
    }

    /// `ftyp` plus a `moov` with empty sample tables and the `mvex` that says so: a fragmented file's header, written once. `None` until the parameter sets are known.
    /// Built from a writer holding no samples, so the tables are empty because there is nothing in them, not because anything special was done.
    pub fn initialization_segment(&self) -> Option<Vec<u8>> {
        let record = self.avc.record()?;
        let mut out = ftyp(true);
        let mut moov = self.moov(&record, 0, false);
        // `mvex` goes at the END of `moov`, after the tracks it describes.
        let mut extends = BoxBuf::new();
        extends.open(b"mvex");
        self.track_extends(&mut extends, 1);
        if self.audio_format.is_some() {
            self.track_extends(&mut extends, 2);
        }
        extends.close();
        let extends = extends.into_bytes();

        let size = (moov.len() + extends.len()) as u32;
        moov[..4].copy_from_slice(&size.to_be_bytes());
        moov.extend_from_slice(&extends);
        out.extend_from_slice(&moov);
        Some(out)
    }

    /// Per-track defaults for the fragments. Every value is written explicitly in each `trun`, so these are all zero and exist only because a reader requires the box.
    fn track_extends(&self, buf: &mut BoxBuf, track: u32) {
        buf.open_full(b"trex", 0, 0);
        buf.u32(track).u32(1).u32(0).u32(0).u32(0);
        buf.close();
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

    fn moov(&self, record: &[u8], payload_start: u64, force_64: bool) -> Vec<u8> {
        let mut buf = BoxBuf::new();
        buf.open(b"moov");

        buf.open_full(b"mvhd", 0, 0);
        buf.u32(0).u32(0).u32(MOVIE_TIMESCALE);
        buf.u32(self.movie_duration() as u32);
        buf.fixed16_16(1.0).u16(0x0100).zeros(10);
        buf.identity_matrix().zeros(24);
        buf.u32(self.next_track_id());
        buf.close();

        self.video_trak(&mut buf, record, payload_start, force_64);
        if self.audio_format.is_some() {
            self.audio_trak(&mut buf, payload_start, force_64);
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

    fn video_trak(&self, buf: &mut BoxBuf, record: &[u8], payload_start: u64, force_64: bool) {
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
        self.video.table.write_stco(buf, payload_start, force_64);
        buf.close();

        buf.close();
        buf.close();
        buf.close();
    }

    fn audio_trak(&self, buf: &mut BoxBuf, payload_start: u64, force_64: bool) {
        let Some(format) = &self.audio_format else {
            return;
        };
        buf.open(b"trak");
        self.track_header(buf, 2, false);

        buf.open(b"mdia");
        self.media_header(
            buf,
            format.sample_rate,
            self.audio.table.duration(),
            b"soun",
        );

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
        self.audio.table.write_stco(buf, payload_start, force_64);
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
        buf.u16(self.video_format.width)
            .u16(self.video_format.height);
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

/// The `esds` box: an MPEG-4 elementary stream descriptor wrapping the `AudioSpecificConfig`, which tells a decoder the profile, rate and channel layout.
/// Built inside out, because parsers are lenient about descriptor lengths and hand-computing them yields a file that plays here and breaks elsewhere.
fn write_esds(buf: &mut BoxBuf, config: &[u8]) {
    // DecoderSpecificInfo: the AudioSpecificConfig itself.
    let specific = descriptor(0x05, config);

    // DecoderConfigDescriptor: MPEG-4 audio (0x40) in an audio stream (0x15); the zero sizes are legal and unread.
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
/// `iso5` is the brand that says a reader must understand `moof`, and `msdh`
/// that the fragments are self-describing.
fn ftyp(fragmented: bool) -> Vec<u8> {
    let mut buf = BoxBuf::new();
    buf.open(b"ftyp");
    buf.raw(b"isom").u32(512);
    for brand in [b"isom", b"iso2", b"avc1", b"mp41"] {
        buf.raw(brand);
    }
    if fragmented {
        for brand in [b"iso5", b"iso6", b"msdh"] {
            buf.raw(brand);
        }
    }
    buf.close();
    buf.into_bytes()
}

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
        // Fields start after the four type bytes, then 24 of reserved and pre_defined before the dimensions.
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

    /// A recording with the microphone off still sets the format. Writing the track anyway leaves a stream with no samples, which players list and then play as nothing.
    #[test]
    fn an_audio_track_with_no_samples_is_dropped() {
        let mut w = writer();
        w.set_audio_format(audio());
        w.push_sample(&[0, 0, 0, 2, 0x65, 1], 1000, true);
        let data = w.finish().expect("a file");
        assert!(
            find(&data, b"mp4a").is_none(),
            "an empty audio track was written"
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
