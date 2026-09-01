use crate::boxes::BoxBuf;

/// One coded sample as the muxer recorded it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sample {
    /// Offset within the mdat payload, not within the file: the file offset is
    /// only known once the moov before it has been sized.
    pub offset: u64,
    pub size: u32,
    /// Decode duration in the track's timescale.
    pub duration: u32,
    pub is_sync: bool,
    /// Composition minus decode time, non-zero only when B-frames reorder.
    pub composition_offset: i32,
}

/// Accumulates samples and emits the `stbl` sub-boxes for them.
#[derive(Debug, Default)]
pub struct SampleTable {
    pub samples: Vec<Sample>,
}

impl SampleTable {
    pub fn push(&mut self, sample: Sample) {
        self.samples.push(sample);
    }

    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    pub fn duration(&self) -> u64 {
        self.samples.iter().map(|s| s.duration as u64).sum()
    }

    /// Run-length coded decode durations.
    pub fn write_stts(&self, buf: &mut BoxBuf) {
        let mut runs: Vec<(u32, u32)> = Vec::new();
        for sample in &self.samples {
            match runs.last_mut() {
                Some((count, delta)) if *delta == sample.duration => *count += 1,
                _ => runs.push((1, sample.duration)),
            }
        }
        buf.open_full(b"stts", 0, 0);
        buf.u32(runs.len() as u32);
        for (count, delta) in runs {
            buf.u32(count).u32(delta);
        }
        buf.close();
    }

    /// Sync sample numbers, 1-based. Omitted entirely when every sample is a
    /// sync point, which is what tells a player the whole track is seekable.
    pub fn write_stss(&self, buf: &mut BoxBuf) {
        if self.samples.iter().all(|s| s.is_sync) {
            return;
        }
        let syncs: Vec<u32> = self
            .samples
            .iter()
            .enumerate()
            .filter(|(_, s)| s.is_sync)
            .map(|(i, _)| i as u32 + 1)
            .collect();
        buf.open_full(b"stss", 0, 0);
        buf.u32(syncs.len() as u32);
        for number in syncs {
            buf.u32(number);
        }
        buf.close();
    }

    /// Composition offsets, written only when something actually reorders.
    pub fn write_ctts(&self, buf: &mut BoxBuf) {
        if self.samples.iter().all(|s| s.composition_offset == 0) {
            return;
        }
        let mut runs: Vec<(u32, i32)> = Vec::new();
        for sample in &self.samples {
            match runs.last_mut() {
                Some((count, offset)) if *offset == sample.composition_offset => *count += 1,
                _ => runs.push((1, sample.composition_offset)),
            }
        }
        // Version 1 so a negative offset is legal; version 0 is unsigned.
        buf.open_full(b"ctts", 1, 0);
        buf.u32(runs.len() as u32);
        for (count, offset) in runs {
            buf.u32(count).i32(offset);
        }
        buf.close();
    }

    pub fn write_stsz(&self, buf: &mut BoxBuf) {
        buf.open_full(b"stsz", 0, 0);
        // A zero default means the per-sample table below is authoritative.
        buf.u32(0).u32(self.samples.len() as u32);
        for sample in &self.samples {
            buf.u32(sample.size);
        }
        buf.close();
    }

    /// Chunks are runs of samples that are contiguous in the file, so a
    /// video-only track collapses to one chunk and an interleaved one gets a
    /// chunk per burst without the caller declaring anything.
    pub fn chunks(&self) -> Vec<Chunk> {
        let mut chunks: Vec<Chunk> = Vec::new();
        for sample in &self.samples {
            match chunks.last_mut() {
                Some(chunk) if chunk.end() == sample.offset => chunk.count += 1,
                _ => chunks.push(Chunk {
                    offset: sample.offset,
                    count: 1,
                    bytes: 0,
                }),
            }
            if let Some(chunk) = chunks.last_mut() {
                chunk.bytes += sample.size as u64;
            }
        }
        chunks
    }

    pub fn write_stsc(&self, buf: &mut BoxBuf) {
        let chunks = self.chunks();
        let mut runs: Vec<(u32, u32)> = Vec::new();
        for (index, chunk) in chunks.iter().enumerate() {
            match runs.last() {
                Some((_, per_chunk)) if *per_chunk == chunk.count => {}
                _ => runs.push((index as u32 + 1, chunk.count)),
            }
        }
        buf.open_full(b"stsc", 0, 0);
        buf.u32(runs.len() as u32);
        for (first_chunk, per_chunk) in runs {
            buf.u32(first_chunk).u32(per_chunk).u32(1);
        }
        buf.close();
    }

    /// Chunk offsets, shifted by where mdat's payload lands in the file. `co64`
    /// when any offset needs more than 32 bits, which a long 4K capture reaches.
    /// `force_64` lets the caller settle the choice BEFORE it knows the final
    /// offsets, so the box cannot change size between moov's two passes.
    pub fn write_stco(&self, buf: &mut BoxBuf, mdat_payload_start: u64, force_64: bool) {
        let offsets: Vec<u64> = self
            .chunks()
            .iter()
            .map(|c| c.offset + mdat_payload_start)
            .collect();
        let needs_64 = force_64 || offsets.iter().any(|&o| o > u32::MAX as u64);
        if needs_64 {
            buf.open_full(b"co64", 0, 0);
            buf.u32(offsets.len() as u32);
            for offset in offsets {
                buf.u64(offset);
            }
        } else {
            buf.open_full(b"stco", 0, 0);
            buf.u32(offsets.len() as u32);
            for offset in offsets {
                buf.u32(offset as u32);
            }
        }
        buf.close();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Chunk {
    pub offset: u64,
    pub count: u32,
    pub bytes: u64,
}

impl Chunk {
    fn end(&self) -> u64 {
        self.offset + self.bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(offset: u64, size: u32, duration: u32, is_sync: bool) -> Sample {
        Sample {
            offset,
            size,
            duration,
            is_sync,
            composition_offset: 0,
        }
    }

    fn table(samples: &[Sample]) -> SampleTable {
        SampleTable {
            samples: samples.to_vec(),
        }
    }

    fn body(write: impl Fn(&mut BoxBuf)) -> Vec<u8> {
        let mut buf = BoxBuf::new();
        write(&mut buf);
        buf.into_bytes()
    }

    /// The writer settles the box width before it knows the final offsets, so
    /// `force_64` has to hold even for offsets that fit in 32 bits: otherwise
    /// moov grows between the probe and the real pass and every chunk offset in
    /// the file is short by that growth.
    #[test]
    fn forcing_sixty_four_bit_offsets_is_honoured_for_small_ones() {
        let t = table(&[sample(0, 1, 1, true)]);
        let narrow = body(|b| t.write_stco(b, 2048, false));
        let wide = body(|b| t.write_stco(b, 2048, true));
        assert_eq!(&narrow[4..8], b"stco");
        assert_eq!(&wide[4..8], b"co64");
        assert!(
            wide.len() > narrow.len(),
            "co64 must be the larger box, or the probe could not go wrong"
        );
    }

    fn entry_count(bytes: &[u8]) -> u32 {
        u32::from_be_bytes(bytes[12..16].try_into().expect("entry count"))
    }

    #[test]
    fn identical_durations_collapse_to_one_stts_run() {
        let t = table(&[
            sample(0, 10, 1000, true),
            sample(10, 10, 1000, false),
            sample(20, 10, 1000, false),
        ]);
        let bytes = body(|b| t.write_stts(b));
        assert_eq!(entry_count(&bytes), 1);
        assert_eq!(&bytes[16..20], &3u32.to_be_bytes(), "sample count");
        assert_eq!(&bytes[20..24], &1000u32.to_be_bytes(), "delta");
    }

    #[test]
    fn a_changed_duration_starts_a_new_stts_run() {
        let t = table(&[
            sample(0, 10, 1000, true),
            sample(10, 10, 500, false),
            sample(20, 10, 500, false),
        ]);
        assert_eq!(entry_count(&body(|b| t.write_stts(b))), 2);
    }

    /// An all-keyframe track omits `stss` entirely: writing one that lists
    /// every sample is legal but tells players the opposite of what we mean.
    #[test]
    fn an_all_sync_track_writes_no_stss() {
        let t = table(&[sample(0, 10, 100, true), sample(10, 10, 100, true)]);
        assert!(body(|b| t.write_stss(b)).is_empty());
    }

    #[test]
    fn stss_lists_sync_samples_one_based() {
        let t = table(&[
            sample(0, 10, 100, true),
            sample(10, 10, 100, false),
            sample(20, 10, 100, true),
        ]);
        let bytes = body(|b| t.write_stss(b));
        assert_eq!(entry_count(&bytes), 2);
        assert_eq!(&bytes[16..20], &1u32.to_be_bytes());
        assert_eq!(&bytes[20..24], &3u32.to_be_bytes());
    }

    #[test]
    fn ctts_is_skipped_when_nothing_reorders() {
        let t = table(&[sample(0, 10, 100, true)]);
        assert!(body(|b| t.write_ctts(b)).is_empty());
    }

    /// Version 1 keeps negative offsets legal; version 0 would wrap them.
    #[test]
    fn ctts_is_written_at_version_one_when_offsets_are_negative() {
        let mut reordered = sample(0, 10, 100, true);
        reordered.composition_offset = -100;
        let t = table(&[reordered]);
        let bytes = body(|b| t.write_ctts(b));
        assert_eq!(bytes[8], 1, "version");
        assert_eq!(&bytes[20..24], &(-100i32).to_be_bytes());
    }

    #[test]
    fn contiguous_samples_collapse_into_one_chunk() {
        let t = table(&[
            sample(0, 10, 100, true),
            sample(10, 20, 100, false),
            sample(30, 5, 100, false),
        ]);
        assert_eq!(
            t.chunks(),
            vec![Chunk {
                offset: 0,
                count: 3,
                bytes: 35
            }]
        );
    }

    /// Interleaving with another track leaves gaps, and a chunk that spans one
    /// would point a player at the other track's bytes.
    #[test]
    fn a_gap_between_samples_starts_a_new_chunk() {
        let t = table(&[
            sample(0, 10, 100, true),
            sample(10, 10, 100, false),
            sample(500, 10, 100, false),
        ]);
        let chunks = t.chunks();
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].count, 2);
        assert_eq!(chunks[1].offset, 500);
    }

    #[test]
    fn stsc_run_length_codes_chunks_with_the_same_sample_count() {
        let t = table(&[
            sample(0, 10, 100, true),
            sample(500, 10, 100, false),
            sample(1000, 10, 100, false),
        ]);
        // Three chunks of one sample each collapse to a single run.
        assert_eq!(entry_count(&body(|b| t.write_stsc(b))), 1);
    }

    #[test]
    fn chunk_offsets_are_shifted_into_the_file() {
        let t = table(&[sample(0, 10, 100, true), sample(500, 10, 100, false)]);
        let bytes = body(|b| t.write_stco(b, 2048, false));
        assert_eq!(&bytes[4..8], b"stco");
        assert_eq!(&bytes[16..20], &2048u32.to_be_bytes());
        assert_eq!(&bytes[20..24], &2548u32.to_be_bytes());
    }

    /// A long 4K capture passes 4 GB, and a 32-bit offset would silently wrap
    /// to the front of the file.
    #[test]
    fn an_offset_past_four_gigabytes_switches_to_co64() {
        let t = table(&[sample(0, 10, 100, true)]);
        let bytes = body(|b| t.write_stco(b, 5_000_000_000, false));
        assert_eq!(&bytes[4..8], b"co64");
        assert_eq!(
            u64::from_be_bytes(bytes[16..24].try_into().expect("offset")),
            5_000_000_000
        );
    }

    #[test]
    fn sizes_are_written_per_sample_with_no_default() {
        let t = table(&[sample(0, 7, 100, true), sample(7, 9, 100, false)]);
        let bytes = body(|b| t.write_stsz(b));
        assert_eq!(&bytes[12..16], &0u32.to_be_bytes(), "no default size");
        assert_eq!(&bytes[16..20], &2u32.to_be_bytes(), "sample count");
        assert_eq!(&bytes[20..24], &7u32.to_be_bytes());
        assert_eq!(&bytes[24..28], &9u32.to_be_bytes());
    }
}
