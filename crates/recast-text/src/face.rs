use std::sync::Arc;

/// A loaded font face. Holds the file bytes so a caption line can be shaped and
/// rasterised without re-reading per chunk.
#[derive(Debug, Clone)]
pub struct FontFace {
    data: Arc<Vec<u8>>,
    index: u32,
    upem: f64,
    ascender: f64,
    descender: f64,
    line_gap: f64,
}

/// Vertical metrics in EM units (multiply by the pixel size to get pixels).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Metrics {
    pub ascender: f64,
    /// Negative, as the font stores it.
    pub descender: f64,
    pub line_gap: f64,
}

impl Metrics {
    /// Baseline-to-baseline distance for consecutive lines, in EM units.
    pub fn line_height(&self) -> f64 {
        self.ascender - self.descender + self.line_gap
    }
}

impl FontFace {
    /// `None` when the bytes are not a face this build can read.
    pub fn from_bytes(data: Arc<Vec<u8>>, index: u32) -> Option<Self> {
        let parsed = ttf_parser::Face::parse(&data, index).ok()?;
        let upem = parsed.units_per_em() as f64;
        if upem <= 0.0 {
            return None;
        }
        let (ascender, descender, line_gap) = (
            parsed.ascender() as f64,
            parsed.descender() as f64,
            parsed.line_gap() as f64,
        );
        Some(Self {
            data,
            index,
            upem,
            ascender: ascender / upem,
            descender: descender / upem,
            line_gap: line_gap / upem,
        })
    }

    pub fn metrics(&self) -> Metrics {
        Metrics {
            ascender: self.ascender,
            descender: self.descender,
            line_gap: self.line_gap,
        }
    }

    pub fn units_per_em(&self) -> f64 {
        self.upem
    }

    /// The file bytes, for a host that resolves a face natively and has to ship
    /// it to a worker or across the wasm boundary.
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    pub(crate) fn parse(&self) -> Option<ttf_parser::Face<'_>> {
        ttf_parser::Face::parse(&self.data, self.index).ok()
    }

    pub(crate) fn shaper(&self) -> Option<rustybuzz::Face<'_>> {
        rustybuzz::Face::from_slice(&self.data, self.index)
    }
}
