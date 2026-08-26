/// A growable buffer of MP4 boxes with size back-patching.
///
/// Boxes nest, and a box's size is only known once its children are written, so
/// `open` records where the size field went and `close` fills it in.
#[derive(Default)]
pub struct BoxBuf {
    bytes: Vec<u8>,
    open: Vec<usize>,
}

impl BoxBuf {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn into_bytes(self) -> Vec<u8> {
        debug_assert!(self.open.is_empty(), "a box was never closed");
        self.bytes
    }

    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// Starts a box, reserving its 32-bit size. `kind` must be four ASCII bytes.
    pub fn open(&mut self, kind: &[u8; 4]) {
        self.open.push(self.bytes.len());
        self.bytes.extend_from_slice(&[0, 0, 0, 0]);
        self.bytes.extend_from_slice(kind);
    }

    /// Starts a full box: the same header plus a version and 24-bit flags.
    pub fn open_full(&mut self, kind: &[u8; 4], version: u8, flags: u32) {
        self.open(kind);
        self.bytes.push(version);
        self.bytes
            .extend_from_slice(&flags.to_be_bytes()[1..4].try_into().unwrap_or([0; 3]));
    }

    pub fn close(&mut self) {
        let start = self.open.pop().expect("close without open");
        let size = (self.bytes.len() - start) as u32;
        self.bytes[start..start + 4].copy_from_slice(&size.to_be_bytes());
    }

    pub fn u8(&mut self, v: u8) -> &mut Self {
        self.bytes.push(v);
        self
    }

    pub fn u16(&mut self, v: u16) -> &mut Self {
        self.bytes.extend_from_slice(&v.to_be_bytes());
        self
    }

    pub fn i16(&mut self, v: i16) -> &mut Self {
        self.bytes.extend_from_slice(&v.to_be_bytes());
        self
    }

    pub fn u32(&mut self, v: u32) -> &mut Self {
        self.bytes.extend_from_slice(&v.to_be_bytes());
        self
    }

    pub fn i32(&mut self, v: i32) -> &mut Self {
        self.bytes.extend_from_slice(&v.to_be_bytes());
        self
    }

    pub fn u64(&mut self, v: u64) -> &mut Self {
        self.bytes.extend_from_slice(&v.to_be_bytes());
        self
    }

    pub fn raw(&mut self, v: &[u8]) -> &mut Self {
        self.bytes.extend_from_slice(v);
        self
    }

    pub fn zeros(&mut self, n: usize) -> &mut Self {
        self.bytes.resize(self.bytes.len() + n, 0);
        self
    }

    /// 16.16 fixed point, which is how MP4 stores rates and the display matrix.
    pub fn fixed16_16(&mut self, v: f64) -> &mut Self {
        self.u32((v * 65_536.0).round() as u32)
    }

    /// The identity display matrix every track we write uses.
    pub fn identity_matrix(&mut self) -> &mut Self {
        for value in [0x0001_0000u32, 0, 0, 0, 0x0001_0000, 0, 0, 0, 0x4000_0000] {
            self.u32(value);
        }
        self
    }
}

/// Reads the top-level box headers of `data` as (kind, total size), in order.
/// Shallow on purpose: it exists so a test can assert the file's own layout
/// without a parser that could share a bug with the writer.
pub fn top_level_boxes(data: &[u8]) -> Vec<([u8; 4], u64)> {
    let mut out = Vec::new();
    let mut at = 0usize;
    while at + 8 <= data.len() {
        let size32 = u32::from_be_bytes([data[at], data[at + 1], data[at + 2], data[at + 3]]);
        let kind = [data[at + 4], data[at + 5], data[at + 6], data[at + 7]];
        let size = match size32 {
            // 1 means the real size is a 64-bit field after the type.
            1 if at + 16 <= data.len() => {
                u64::from_be_bytes(data[at + 8..at + 16].try_into().unwrap_or([0; 8]))
            }
            // 0 means the box runs to end of file.
            0 => (data.len() - at) as u64,
            other => other as u64,
        };
        if size < 8 {
            break;
        }
        out.push((kind, size));
        at += size as usize;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_box_records_its_own_size() {
        let mut buf = BoxBuf::new();
        buf.open(b"free");
        buf.u32(7);
        buf.close();
        let bytes = buf.into_bytes();
        assert_eq!(bytes.len(), 12);
        assert_eq!(&bytes[0..4], &12u32.to_be_bytes());
        assert_eq!(&bytes[4..8], b"free");
    }

    #[test]
    fn a_nested_box_is_counted_inside_its_parent() {
        let mut buf = BoxBuf::new();
        buf.open(b"moov");
        buf.open(b"mvhd");
        buf.u32(1);
        buf.close();
        buf.close();
        let bytes = buf.into_bytes();
        // 8 (moov header) + 12 (mvhd header plus its four payload bytes).
        assert_eq!(&bytes[0..4], &20u32.to_be_bytes());
        assert_eq!(&bytes[8..12], &12u32.to_be_bytes());
    }

    #[test]
    fn a_full_box_carries_its_version_and_flags() {
        let mut buf = BoxBuf::new();
        buf.open_full(b"tkhd", 0, 0x00_0007);
        buf.close();
        let bytes = buf.into_bytes();
        assert_eq!(bytes[8], 0, "version");
        assert_eq!(&bytes[9..12], &[0x00, 0x00, 0x07], "flags");
    }

    #[test]
    fn the_scan_walks_consecutive_boxes() {
        let mut buf = BoxBuf::new();
        buf.open(b"ftyp");
        buf.zeros(8);
        buf.close();
        buf.open(b"mdat");
        buf.zeros(4);
        buf.close();
        let found = top_level_boxes(&buf.into_bytes());
        assert_eq!(found, vec![(*b"ftyp", 16), (*b"mdat", 12)]);
    }

    /// A zero size means "to end of file". Treating it as an empty box would
    /// loop forever.
    #[test]
    fn a_zero_sized_box_is_read_as_running_to_the_end() {
        let mut data = Vec::new();
        data.extend_from_slice(&0u32.to_be_bytes());
        data.extend_from_slice(b"mdat");
        data.extend_from_slice(&[1, 2, 3, 4]);
        assert_eq!(top_level_boxes(&data), vec![(*b"mdat", 12)]);
    }

    #[test]
    fn a_sixty_four_bit_size_is_read_from_the_largesize_field() {
        let mut data = Vec::new();
        data.extend_from_slice(&1u32.to_be_bytes());
        data.extend_from_slice(b"mdat");
        data.extend_from_slice(&24u64.to_be_bytes());
        data.extend_from_slice(&[0; 8]);
        assert_eq!(top_level_boxes(&data), vec![(*b"mdat", 24)]);
    }

    #[test]
    fn fixed_point_writes_the_upper_and_lower_halves() {
        let mut buf = BoxBuf::new();
        buf.fixed16_16(1.0);
        buf.fixed16_16(0.5);
        let bytes = buf.into_bytes();
        assert_eq!(&bytes[0..4], &[0x00, 0x01, 0x00, 0x00]);
        assert_eq!(&bytes[4..8], &[0x00, 0x00, 0x80, 0x00]);
    }
}
