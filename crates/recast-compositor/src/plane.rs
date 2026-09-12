//! The tilted card: a layer's `Transform3` projected to four canvas-pixel corners with their depths (06, D-1 and D-4).
//! Zoom stays a source-uv window; this is the destination warp that multiplies the placement after it.

use recast_scene::bind::Transform3;

use crate::eval::DestRect;

/// Four projected corners in canvas pixels, in card order (top-left, top-right, bottom-right, bottom-left),
/// and the homogeneous `w` of each so the rasteriser interpolates the card's uv perspective-correctly.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Plane {
    pub corners: [[f32; 2]; 4],
    pub depth: [f32; 4],
}

/// The nearest a corner may come to the eye, as a fraction of the focal length; closer is clamped, never inverted.
const NEAREST: f64 = -0.9;

/// Projects the card. `None` for the identity transform, so the flat path (and its goldens, the rotating segment
/// animation included) stays exactly as it was. `rotate` is that animation's 2D turn in radians; it composes with `rz`.
pub fn project(
    dest: DestRect,
    rotate: f32,
    t: &Transform3,
    canvas_w: u32,
    canvas_h: u32,
) -> Option<Plane> {
    if t.is_identity() {
        return None;
    }
    let (w, h) = (f64::from(dest.w), f64::from(dest.h));
    let (cw, ch) = (f64::from(canvas_w), f64::from(canvas_h).max(1.0));
    let pivot = (
        f64::from(dest.x) + t.anchor_x * w,
        f64::from(dest.y) + t.anchor_y * h,
    );
    let focal = (t.perspective.max(0.1)) * ch;
    let (rz, rx, ry) = (
        f64::from(rotate) + t.rz.to_radians(),
        t.rx.to_radians(),
        t.ry.to_radians(),
    );
    let offset = (t.x * cw, t.y * ch, t.z * ch);
    let mut corners = [[0.0f32; 2]; 4];
    let mut depth = [1.0f32; 4];
    for (i, (u, v)) in [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)]
        .into_iter()
        .enumerate()
    {
        // Local, about the pivot, scaled, then turned in the plane.
        let lx = (u - t.anchor_x) * w * t.scale;
        let ly = (v - t.anchor_y) * h * t.scale;
        let (px, py) = (lx * rz.cos() - ly * rz.sin(), lx * rz.sin() + ly * rz.cos());
        // Tilt about the horizontal axis, then the vertical one; depth grows away from the eye.
        let (ty, tz1) = (py * rx.cos(), -py * rx.sin());
        let (tx, tz2) = (px * ry.cos(), px * ry.sin());
        let z = (tz1 + tz2 + offset.2).max(NEAREST * focal);
        let s = focal / (focal + z);
        corners[i] = [
            (pivot.0 + tx * s + offset.0) as f32,
            (pivot.1 + ty * s + offset.1) as f32,
        ];
        depth[i] = ((focal + z) / focal) as f32;
    }
    Some(Plane { corners, depth })
}

/// A 3x3 projective map in canvas pixels, row-major, applied as `(x, y, 1)` then divided by the last row.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Homography(pub [f64; 9]);

impl Homography {
    pub const IDENTITY: Self = Self([1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]);

    #[must_use]
    pub fn apply(&self, x: f64, y: f64) -> (f64, f64) {
        let m = &self.0;
        let w = m[6] * x + m[7] * y + m[8];
        let w = if w.abs() < 1e-12 { 1e-12 } else { w };
        (
            (m[0] * x + m[1] * y + m[2]) / w,
            (m[3] * x + m[4] * y + m[5]) / w,
        )
    }

    #[must_use]
    pub fn then(&self, next: &Homography) -> Homography {
        let (a, b) = (&next.0, &self.0);
        let mut out = [0.0; 9];
        for r in 0..3 {
            for c in 0..3 {
                out[r * 3 + c] =
                    a[r * 3] * b[c] + a[r * 3 + 1] * b[3 + c] + a[r * 3 + 2] * b[6 + c];
            }
        }
        Homography(out)
    }

    /// `None` when the map is degenerate (a quad folded onto a line).
    #[must_use]
    pub fn inverse(&self) -> Option<Homography> {
        let m = &self.0;
        let det = m[0] * (m[4] * m[8] - m[5] * m[7]) - m[1] * (m[3] * m[8] - m[5] * m[6])
            + m[2] * (m[3] * m[7] - m[4] * m[6]);
        if det.abs() < 1e-12 {
            return None;
        }
        let d = 1.0 / det;
        Some(Homography([
            (m[4] * m[8] - m[5] * m[7]) * d,
            (m[2] * m[7] - m[1] * m[8]) * d,
            (m[1] * m[5] - m[2] * m[4]) * d,
            (m[5] * m[6] - m[3] * m[8]) * d,
            (m[0] * m[8] - m[2] * m[6]) * d,
            (m[2] * m[3] - m[0] * m[5]) * d,
            (m[3] * m[7] - m[4] * m[6]) * d,
            (m[1] * m[6] - m[0] * m[7]) * d,
            (m[0] * m[4] - m[1] * m[3]) * d,
        ]))
    }

    /// The map from the unit square to `quad` (corners in card order), the textbook four-point solve.
    #[must_use]
    pub fn unit_to_quad(quad: [[f64; 2]; 4]) -> Homography {
        let [[x0, y0], [x1, y1], [x2, y2], [x3, y3]] = quad;
        let (dx1, dx2, dx3) = (x1 - x2, x3 - x2, x0 - x1 + x2 - x3);
        let (dy1, dy2, dy3) = (y1 - y2, y3 - y2, y0 - y1 + y2 - y3);
        let den = dx1 * dy2 - dx2 * dy1;
        let (g, h) = if den.abs() < 1e-12 {
            (0.0, 0.0)
        } else {
            ((dx3 * dy2 - dx2 * dy3) / den, (dx1 * dy3 - dx3 * dy1) / den)
        };
        Homography([
            x1 - x0 + g * x1,
            x3 - x0 + h * x3,
            x0,
            y1 - y0 + g * y1,
            y3 - y0 + h * y3,
            y0,
            g,
            h,
            1.0,
        ])
    }

    /// Rows as the shaders take them: three `vec4`s with the fourth lane unused.
    #[must_use]
    pub fn rows(&self) -> [[f32; 4]; 3] {
        let m = self.0;
        [
            [m[0] as f32, m[1] as f32, m[2] as f32, 0.0],
            [m[3] as f32, m[4] as f32, m[5] as f32, 0.0],
            [m[6] as f32, m[7] as f32, m[8] as f32, 0.0],
        ]
    }
}

impl Plane {
    /// Canvas pixels on the flat card (`dest`) to canvas pixels on the tilted one: what a drawable anchored to the
    /// card multiplies by to ride the tilt (06, D-3).
    #[must_use]
    pub fn warp(&self, dest: DestRect) -> Homography {
        let flat_to_unit = Homography([
            1.0 / f64::from(dest.w.max(1e-6)),
            0.0,
            -f64::from(dest.x) / f64::from(dest.w.max(1e-6)),
            0.0,
            1.0 / f64::from(dest.h.max(1e-6)),
            -f64::from(dest.y) / f64::from(dest.h.max(1e-6)),
            0.0,
            0.0,
            1.0,
        ]);
        let quad = self.corners.map(|c| [f64::from(c[0]), f64::from(c[1])]);
        flat_to_unit.then(&Homography::unit_to_quad(quad))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_warp_sends_the_flat_corners_onto_the_projected_ones_and_inverts_back() {
        let tilt = Transform3 {
            ry: 35.0,
            rx: 8.0,
            z: 0.15,
            ..Transform3::IDENTITY
        };
        let d = dest();
        let plane = project(d, 0.0, &tilt, 1920, 1080).unwrap();
        let warp = plane.warp(d);
        let flat = [
            (d.x, d.y),
            (d.x + d.w, d.y),
            (d.x + d.w, d.y + d.h),
            (d.x, d.y + d.h),
        ];
        for (i, (x, y)) in flat.into_iter().enumerate() {
            let (px, py) = warp.apply(f64::from(x), f64::from(y));
            assert!(
                (px - f64::from(plane.corners[i][0])).abs() < 1e-3
                    && (py - f64::from(plane.corners[i][1])).abs() < 1e-3,
                "corner {i}: ({px}, {py}) vs {:?}",
                plane.corners[i]
            );
        }
        let back = warp.inverse().unwrap();
        let (x, y) = warp.apply(250.0, 120.0);
        let (bx, by) = back.apply(x, y);
        assert!((bx - 250.0).abs() < 1e-6 && (by - 120.0).abs() < 1e-6);
        assert_eq!(Homography::IDENTITY.then(&warp), warp);
    }

    fn dest() -> DestRect {
        DestRect {
            x: 100.0,
            y: 50.0,
            w: 400.0,
            h: 300.0,
        }
    }

    #[test]
    fn the_identity_is_no_plane_and_a_turn_about_the_vertical_axis_brings_one_side_nearer() {
        assert_eq!(
            project(dest(), 0.0, &Transform3::IDENTITY, 1920, 1080),
            None
        );
        let tilt = Transform3 {
            ry: 30.0,
            ..Transform3::IDENTITY
        };
        let plane = project(dest(), 0.0, &tilt, 1920, 1080).unwrap();
        // As in CSS `rotateY`, a positive turn sends the right edge away: deeper, so smaller, while the left edge nears.
        assert!(plane.depth[1] > 1.0 && plane.depth[2] > 1.0, "{plane:?}");
        assert!(plane.depth[0] < 1.0 && plane.depth[3] < 1.0, "{plane:?}");
        let right_height = plane.corners[2][1] - plane.corners[1][1];
        let left_height = plane.corners[3][1] - plane.corners[0][1];
        assert!(right_height < 300.0 && left_height > 300.0, "{plane:?}");
        // The card still straddles its pivot, the centre at x = 300.
        assert!(
            plane.corners[0][0] < 300.0 && plane.corners[1][0] > 300.0,
            "{plane:?}"
        );
    }

    #[test]
    fn scale_and_offset_act_in_the_plane_and_z_pushes_the_whole_card_away() {
        let bigger = Transform3 {
            scale: 2.0,
            ..Transform3::IDENTITY
        };
        let plane = project(dest(), 0.0, &bigger, 1920, 1080).unwrap();
        assert!((plane.corners[1][0] - plane.corners[0][0] - 800.0).abs() < 1e-3);
        let moved = Transform3 {
            x: 0.25,
            ..Transform3::IDENTITY
        };
        let plane = project(dest(), 0.0, &moved, 1920, 1080).unwrap();
        assert!((plane.corners[0][0] - 580.0).abs() < 1e-3, "{plane:?}");
        let away = Transform3 {
            z: 2.0,
            ..Transform3::IDENTITY
        };
        let plane = project(dest(), 0.0, &away, 1920, 1080).unwrap();
        let width = plane.corners[1][0] - plane.corners[0][0];
        assert!(
            (width - 200.0).abs() < 1e-3,
            "twice the focal length away halves it: {width}"
        );
        assert!((plane.depth[0] - 2.0).abs() < 1e-6);
    }

    #[test]
    fn a_corner_pulled_through_the_eye_is_clamped_not_inverted() {
        let close = Transform3 {
            z: -5.0,
            ..Transform3::IDENTITY
        };
        let plane = project(dest(), 0.0, &close, 1920, 1080).unwrap();
        assert!(plane.depth.iter().all(|d| *d > 0.0), "{plane:?}");
    }
}
