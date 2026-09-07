use core::fmt;

/// A rectangle in device pixels. The coordinate space is the caller's to name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct Rect {
    /// Left edge.
    pub x: i32,
    /// Top edge.
    pub y: i32,
    /// Width in pixels. Zero means empty, never negative.
    pub width: u32,
    /// Height in pixels. Zero means empty, never negative.
    pub height: u32,
}

impl Rect {
    /// A rectangle at the origin with the given size.
    #[must_use]
    pub const fn from_size(width: u32, height: u32) -> Self {
        Self {
            x: 0,
            y: 0,
            width,
            height,
        }
    }

    /// A rectangle at an arbitrary position.
    #[must_use]
    pub const fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Exclusive right edge. Widened to `i64` because `x + width` overflows `i32`
    /// on a virtual desktop that spans far enough right.
    #[must_use]
    pub const fn right(&self) -> i64 {
        self.x as i64 + self.width as i64
    }

    /// Exclusive bottom edge, widened for the same reason as [`Rect::right`].
    #[must_use]
    pub const fn bottom(&self) -> i64 {
        self.y as i64 + self.height as i64
    }

    /// Whether the rectangle covers no pixels at all.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.width == 0 || self.height == 0
    }

    /// Pixel count, wide enough that an 8K rectangle cannot overflow it.
    #[must_use]
    pub const fn area(&self) -> u64 {
        self.width as u64 * self.height as u64
    }

    /// The overlapping region, or `None` when the two do not touch.
    #[must_use]
    pub fn intersect(&self, other: &Self) -> Option<Self> {
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        let right = self.right().min(other.right());
        let bottom = self.bottom().min(other.bottom());
        if right <= i64::from(x) || bottom <= i64::from(y) {
            return None;
        }
        Some(Self {
            x,
            y,
            width: (right - i64::from(x)) as u32,
            height: (bottom - i64::from(y)) as u32,
        })
    }

    /// The smallest rectangle containing both.
    #[must_use]
    pub fn union(&self, other: &Self) -> Self {
        if self.is_empty() {
            return *other;
        }
        if other.is_empty() {
            return *self;
        }
        let x = self.x.min(other.x);
        let y = self.y.min(other.y);
        Self {
            x,
            y,
            width: (self.right().max(other.right()) - i64::from(x)) as u32,
            height: (self.bottom().max(other.bottom()) - i64::from(y)) as u32,
        }
    }

    /// Whether `other` lies entirely inside this rectangle.
    #[must_use]
    pub fn contains(&self, other: &Self) -> bool {
        other.x >= self.x
            && other.y >= self.y
            && other.right() <= self.right()
            && other.bottom() <= self.bottom()
    }

    /// Whether `point` lies inside, treating the right and bottom edges as belonging to the next rectangle along.
    /// Half-open on purpose: adjacent displays share an edge, and a closed test puts a point on it in both of them.
    #[must_use]
    pub fn contains_point(&self, point: (i32, i32)) -> bool {
        i64::from(point.0) >= i64::from(self.x)
            && i64::from(point.0) < self.right()
            && i64::from(point.1) >= i64::from(self.y)
            && i64::from(point.1) < self.bottom()
    }

    /// The middle of the rectangle, for asking which surface holds it.
    #[must_use]
    pub fn centre(&self) -> (i32, i32) {
        (
            self.x.saturating_add((self.width / 2) as i32),
            self.y.saturating_add((self.height / 2) as i32),
        )
    }

    /// This rectangle re-expressed with `origin`'s top-left as `(0, 0)`.
    #[must_use]
    pub fn relative_to(&self, origin: &Self) -> Self {
        Self {
            x: self.x.saturating_sub(origin.x),
            y: self.y.saturating_sub(origin.y),
            width: self.width,
            height: self.height,
        }
    }

    /// This rectangle moved by `origin`, the inverse of [`Rect::relative_to`].
    /// Takes a rectangle expressed inside some surface and puts it back into the space that surface lives in.
    #[must_use]
    pub fn offset_by(&self, origin: &Self) -> Self {
        Self {
            x: self.x.saturating_add(origin.x),
            y: self.y.saturating_add(origin.y),
            width: self.width,
            height: self.height,
        }
    }

    /// This rectangle scaled about the origin, for lifting logical points into
    /// physical pixels on a HiDPI display.
    #[must_use]
    pub fn scaled(&self, factor: f64) -> Self {
        if !factor.is_finite() || factor <= 0.0 {
            return *self;
        }
        Self {
            x: (f64::from(self.x) * factor).round() as i32,
            y: (f64::from(self.y) * factor).round() as i32,
            width: (f64::from(self.width) * factor).round() as u32,
            height: (f64::from(self.height) * factor).round() as u32,
        }
    }

    /// The largest even-sized rectangle inside both `self` and `bounds`.
    /// Even because every subsampled format and most encoders reject odd dimensions; it shrinks rather than grows, so the result never reads past the captured frame.
    #[must_use]
    pub fn fit_inside(&self, bounds: &Self) -> Option<Self> {
        let clipped = self.intersect(bounds)?;
        let width = clipped.width & !1;
        let height = clipped.height & !1;
        if width == 0 || height == 0 {
            return None;
        }
        Some(Self {
            width,
            height,
            ..clipped
        })
    }
}

impl fmt::Display for Rect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}x{}+{}+{}", self.width, self.height, self.x, self.y)
    }
}

/// How far the source is rotated clockwise from upright.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub enum Rotation {
    /// Upright.
    #[default]
    None,
    /// A quarter turn clockwise.
    Cw90,
    /// A half turn.
    Cw180,
    /// Three quarter turns clockwise.
    Cw270,
}

impl Rotation {
    /// Degrees clockwise.
    #[must_use]
    pub const fn degrees(self) -> u32 {
        match self {
            Self::None => 0,
            Self::Cw90 => 90,
            Self::Cw180 => 180,
            Self::Cw270 => 270,
        }
    }

    /// Whether the rotation swaps width and height.
    #[must_use]
    pub const fn is_quarter_turn(self) -> bool {
        matches!(self, Self::Cw90 | Self::Cw270)
    }

    /// The size a frame presents after the rotation is applied.
    #[must_use]
    pub const fn apply_to_size(self, width: u32, height: u32) -> (u32, u32) {
        if self.is_quarter_turn() {
            (height, width)
        } else {
            (width, height)
        }
    }
}

/// The regions of a frame that changed since the previous one.
/// EMPTY means assume everything changed, not nothing: backends that cannot report damage return an empty set, and reading it as no-work-to-do freezes on those platforms.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DirtyRects {
    rects: Vec<Rect>,
}

impl DirtyRects {
    /// The "no damage information available" value: treat the frame as fully dirty.
    #[must_use]
    pub const fn unknown() -> Self {
        Self { rects: Vec::new() }
    }

    /// Collect damage, dropping empty rectangles the OS sometimes reports.
    pub fn from_rects<I: IntoIterator<Item = Rect>>(rects: I) -> Self {
        Self {
            rects: rects.into_iter().filter(|r| !r.is_empty()).collect(),
        }
    }

    /// Whether the backend gave no damage information, so the whole frame is dirty.
    #[must_use]
    pub fn is_unknown(&self) -> bool {
        self.rects.is_empty()
    }

    /// The reported regions, empty when [`DirtyRects::is_unknown`].
    #[must_use]
    pub fn as_slice(&self) -> &[Rect] {
        &self.rects
    }

    /// The smallest rectangle covering every reported region.
    #[must_use]
    pub fn bounds(&self) -> Option<Rect> {
        self.rects.iter().copied().reduce(|acc, r| acc.union(&r))
    }

    /// The region of `frame` a consumer must redraw, which is all of it when the
    /// damage is unknown.
    #[must_use]
    pub fn damaged_area(&self, frame: &Rect) -> Option<Rect> {
        match self.bounds() {
            None => Some(*frame),
            Some(b) => b.intersect(frame),
        }
    }
}

#[cfg(test)]
mod point_tests {
    use super::Rect;

    /// Adjacent displays share an edge; a closed test would put a point on it in
    /// both of them, and the picker would pick the wrong one.
    #[test]
    fn containment_is_half_open_on_the_far_edge() {
        let rect = Rect::new(0, 0, 100, 100);
        assert!(rect.contains_point((0, 0)));
        assert!(rect.contains_point((99, 99)));
        assert!(!rect.contains_point((100, 50)));
        assert!(!rect.contains_point((50, 100)));
    }

    #[test]
    fn a_display_left_of_the_primary_holds_its_own_points() {
        let left = Rect::new(-1920, 0, 1920, 1080);
        assert!(left.contains_point((-1000, 500)));
        assert!(!left.contains_point((10, 500)));
    }

    /// The right edge is computed in i64, so a desktop that spans past i32 does
    /// not wrap into reporting a point as outside.
    #[test]
    fn a_rectangle_reaching_past_i32_still_contains_its_points() {
        let far = Rect::new(i32::MAX - 10, 0, 100, 100);
        assert!(far.contains_point((i32::MAX - 1, 50)));
    }

    #[test]
    fn the_centre_is_the_middle_of_the_rectangle() {
        assert_eq!(Rect::new(10, 20, 100, 50).centre(), (60, 45));
        assert_eq!(Rect::new(-1920, 0, 1920, 1080).centre(), (-960, 540));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intersect_returns_none_for_rectangles_that_only_touch_edges() {
        let a = Rect::new(0, 0, 10, 10);
        let b = Rect::new(10, 0, 10, 10);
        assert_eq!(a.intersect(&b), None);
    }

    #[test]
    fn intersect_returns_the_overlap() {
        let a = Rect::new(0, 0, 10, 10);
        let b = Rect::new(5, 5, 10, 10);
        assert_eq!(a.intersect(&b), Some(Rect::new(5, 5, 5, 5)));
    }

    #[test]
    fn right_edge_survives_a_virtual_desktop_that_overflows_i32() {
        let far = Rect::new(i32::MAX - 10, 0, 100, 100);
        assert_eq!(far.right(), i64::from(i32::MAX) - 10 + 100);
    }

    #[test]
    fn intersect_does_not_overflow_at_the_far_edge() {
        let far = Rect::new(i32::MAX - 10, 0, 100, 100);
        let bounds = Rect::new(i32::MAX - 10, 0, 4, 4);
        assert_eq!(far.intersect(&bounds), Some(bounds));
    }

    #[test]
    fn fit_inside_shrinks_an_odd_region_rather_than_growing_past_the_frame() {
        let frame = Rect::from_size(1920, 1080);
        let wanted = Rect::new(1917, 5, 7, 9);
        let fitted = wanted.fit_inside(&frame).expect("overlaps the frame");
        assert_eq!(fitted, Rect::new(1917, 5, 2, 8));
        assert!(frame.contains(&fitted));
    }

    #[test]
    fn fit_inside_refuses_a_sliver_too_thin_to_be_even() {
        let frame = Rect::from_size(100, 100);
        assert_eq!(Rect::new(99, 0, 50, 50).fit_inside(&frame), None);
    }

    #[test]
    fn fit_inside_refuses_a_region_outside_the_frame() {
        let frame = Rect::from_size(100, 100);
        assert_eq!(Rect::new(200, 200, 10, 10).fit_inside(&frame), None);
    }

    #[test]
    fn relative_to_moves_a_crop_into_frame_coordinates() {
        let source = Rect::new(-1920, 0, 1920, 1080);
        let crop = Rect::new(-1820, 100, 640, 480);
        assert_eq!(crop.relative_to(&source), Rect::new(100, 100, 640, 480));
    }

    #[test]
    fn offset_by_is_the_inverse_of_relative_to() {
        let surface = Rect::new(1920, 0, 1920, 1080);
        let local = Rect::new(100, 50, 640, 480);
        assert_eq!(local.offset_by(&surface), Rect::new(2020, 50, 640, 480));
        assert_eq!(local.offset_by(&surface).relative_to(&surface), local);
    }

    #[test]
    fn scaled_lifts_logical_points_to_physical_pixels() {
        let logical = Rect::new(10, 20, 100, 50);
        assert_eq!(logical.scaled(2.0), Rect::new(20, 40, 200, 100));
    }

    #[test]
    fn scaled_ignores_a_nonsense_factor_rather_than_producing_a_zero_rect() {
        let r = Rect::new(10, 20, 100, 50);
        assert_eq!(r.scaled(0.0), r);
        assert_eq!(r.scaled(f64::NAN), r);
        assert_eq!(r.scaled(-1.0), r);
    }

    #[test]
    fn union_of_an_empty_rect_is_the_other_one() {
        let a = Rect::default();
        let b = Rect::new(5, 5, 10, 10);
        assert_eq!(a.union(&b), b);
        assert_eq!(b.union(&a), b);
    }

    #[test]
    fn unknown_damage_means_the_whole_frame_is_dirty() {
        let frame = Rect::from_size(800, 600);
        let dirty = DirtyRects::unknown();
        assert!(dirty.is_unknown());
        assert_eq!(dirty.damaged_area(&frame), Some(frame));
    }

    #[test]
    fn damage_bounds_cover_every_reported_region() {
        let dirty = DirtyRects::from_rects([Rect::new(10, 10, 5, 5), Rect::new(100, 50, 20, 20)]);
        assert_eq!(dirty.bounds(), Some(Rect::new(10, 10, 110, 60)));
    }

    #[test]
    fn empty_reported_rects_do_not_make_the_damage_look_known() {
        let dirty = DirtyRects::from_rects([Rect::new(10, 10, 0, 5)]);
        assert!(dirty.is_unknown());
    }

    #[test]
    fn damage_outside_the_frame_is_clipped_to_it() {
        let frame = Rect::from_size(100, 100);
        let dirty = DirtyRects::from_rects([Rect::new(90, 90, 50, 50)]);
        assert_eq!(dirty.damaged_area(&frame), Some(Rect::new(90, 90, 10, 10)));
    }

    #[test]
    fn a_quarter_turn_swaps_the_presented_size() {
        assert_eq!(Rotation::Cw90.apply_to_size(1920, 1080), (1080, 1920));
        assert_eq!(Rotation::Cw180.apply_to_size(1920, 1080), (1920, 1080));
    }
}
