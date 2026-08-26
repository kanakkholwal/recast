use std::collections::HashMap;

use crate::face::FontFace;
use crate::raster::rasterize;

/// Transparent gutter between packed glyphs so linear sampling cannot pull ink
/// from the neighbour.
const PADDING: u32 = 1;

/// Sub-pixel steps the requested size is snapped to. Caption size tracks canvas
/// height, so rounding to whole pixels would visibly step during a resize.
const SIZE_STEPS: f64 = 4.0;

/// Identifies one rasterised glyph. `face` is the caller's own numbering: the
/// atlas never compares font bytes, so two ids must not share a face.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct GlyphKey {
    face: u32,
    glyph: u16,
    size: u32,
}

/// Where a glyph landed in the atlas. `left`/`top` are the mask's offset from
/// the pen, carried through so the caller does not have to keep the mask.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AtlasGlyph {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub left: i32,
    pub top: i32,
}

struct Shelf {
    y: u32,
    height: u32,
    pen: u32,
}

/// A growing single-channel coverage atlas for shaped glyphs.
///
/// Width is fixed at construction so growth is a plain buffer extend and every
/// packed coordinate stays valid; only the height doubles.
pub struct GlyphAtlas {
    width: u32,
    height: u32,
    max_height: u32,
    pixels: Vec<u8>,
    shelves: Vec<Shelf>,
    entries: HashMap<GlyphKey, Option<AtlasGlyph>>,
    dirty: Option<(u32, u32)>,
    generation: u64,
    overflowed: bool,
}

impl GlyphAtlas {
    pub fn new(width: u32, max_height: u32) -> Self {
        let width = width.max(1);
        let height = 64.min(max_height.max(1));
        Self {
            width,
            height,
            max_height: max_height.max(height),
            pixels: vec![0; (width * height) as usize],
            shelves: Vec::new(),
            entries: HashMap::new(),
            dirty: None,
            generation: 0,
            overflowed: false,
        }
    }

    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    pub fn coverage(&self) -> &[u8] {
        &self.pixels
    }

    /// Bumped whenever the backing buffer is replaced, so a GPU mirror knows to
    /// recreate its texture instead of uploading a dirty range into a stale one.
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// True once a glyph has been refused for want of room. The caller decides
    /// when to `reset`, because doing it mid-frame would invalidate coordinates
    /// already handed out.
    pub fn overflowed(&self) -> bool {
        self.overflowed
    }

    /// The row range written since the last call, for an incremental upload.
    pub fn take_dirty(&mut self) -> Option<(u32, u32)> {
        self.dirty.take()
    }

    /// Drops every packed glyph, keeping the allocation. The generation bump
    /// tells the mirror its texture contents are gone.
    pub fn reset(&mut self) {
        self.pixels.fill(0);
        self.shelves.clear();
        self.entries.clear();
        self.dirty = None;
        self.generation += 1;
        self.overflowed = false;
    }

    /// Rasterises and packs `glyph` if it is not already present. `None` means
    /// the glyph has no ink (a space) or the atlas is full.
    pub fn insert(
        &mut self,
        face_id: u32,
        face: &FontFace,
        glyph: u16,
        px: f64,
    ) -> Option<AtlasGlyph> {
        let steps = (px * SIZE_STEPS).round().max(0.0) as u32;
        let key = GlyphKey {
            face: face_id,
            glyph,
            size: steps,
        };
        if let Some(cached) = self.entries.get(&key) {
            return *cached;
        }
        // Rasterise at the snapped size, so a cache hit is the same pixels.
        let mask = rasterize(face, glyph, steps as f64 / SIZE_STEPS);
        let placed = mask.filter(|m| !m.is_empty()).and_then(|mask| {
            let spot = self.allocate(mask.width, mask.height)?;
            self.blit(spot, &mask);
            Some(AtlasGlyph {
                x: spot.0,
                y: spot.1,
                width: mask.width,
                height: mask.height,
                left: mask.left,
                top: mask.top,
            })
        });
        // A refusal is cached too, so a full atlas stops re-rasterising; `reset`
        // is what frees it.
        self.entries.insert(key, placed);
        placed
    }

    fn allocate(&mut self, width: u32, height: u32) -> Option<(u32, u32)> {
        if width > self.width {
            self.overflowed = true;
            return None;
        }
        for shelf in &mut self.shelves {
            if shelf.height >= height && shelf.pen + width <= self.width {
                let spot = (shelf.pen, shelf.y);
                shelf.pen += width + PADDING;
                return Some(spot);
            }
        }

        let top = self
            .shelves
            .last()
            .map(|s| s.y + s.height + PADDING)
            .unwrap_or(0);
        while top + height > self.height {
            if !self.grow() {
                self.overflowed = true;
                return None;
            }
        }
        self.shelves.push(Shelf {
            y: top,
            height,
            pen: width + PADDING,
        });
        Some((0, top))
    }

    fn grow(&mut self) -> bool {
        if self.height >= self.max_height {
            return false;
        }
        self.height = (self.height * 2).min(self.max_height);
        self.pixels.resize((self.width * self.height) as usize, 0);
        self.generation += 1;
        self.dirty = None;
        true
    }

    fn blit(&mut self, (x, y): (u32, u32), mask: &crate::raster::GlyphMask) {
        for row in 0..mask.height {
            let src = (row * mask.width) as usize;
            let dst = ((y + row) * self.width + x) as usize;
            self.pixels[dst..dst + mask.width as usize]
                .copy_from_slice(&mask.coverage[src..src + mask.width as usize]);
        }
        self.dirty = Some(match self.dirty {
            Some((from, to)) => (from.min(y), to.max(y + mask.height)),
            None => (y, y + mask.height),
        });
    }
}
