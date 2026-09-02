/// Where a source of `src` lands inside a `dst` frame, fitted and centred.
/// `fills` says the source already covers it, so the caller can skip clearing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FitRect {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
    pub fills: bool,
}

/// Largest centred rect of `src`'s aspect that fits inside `dst`.
#[must_use]
pub fn fit_rect(src: (u32, u32), dst: (u32, u32)) -> FitRect {
    let (sw, sh) = src;
    let (dw, dh) = dst;
    if sw == 0 || sh == 0 || dw == 0 || dh == 0 {
        return FitRect {
            x: 0,
            y: 0,
            w: dw,
            h: dh,
            fills: true,
        };
    }
    // Cross-multiplied so an equal aspect is exact at any size, and the caller can skip the clear.
    if u64::from(sw) * u64::from(dh) == u64::from(sh) * u64::from(dw) {
        return FitRect {
            x: 0,
            y: 0,
            w: dw,
            h: dh,
            fills: true,
        };
    }
    let scale = f64::from(dw) / f64::from(sw);
    let scale = scale.min(f64::from(dh) / f64::from(sh));
    let w = ((f64::from(sw) * scale).round() as u32).clamp(1, dw);
    let h = ((f64::from(sh) * scale).round() as u32).clamp(1, dh);
    FitRect {
        x: (dw - w) / 2,
        y: (dh - h) / 2,
        w,
        h,
        fills: false,
    }
}

/// Draws a packed RGBA source into a packed RGBA frame of `dst`, fitted and
/// centred, with the margins left black.
///
/// Bilinear: a display coming back at another resolution mid-take leaves the
/// rest of the recording going through this, and nearest-neighbour on a whole
/// desktop is visibly ragged.
pub fn fit_into(out: &mut Vec<u8>, src: &[u8], src_size: (u32, u32), dst: (u32, u32)) {
    let (dw, dh) = dst;
    out.clear();
    out.resize(dw as usize * dh as usize * 4, 0);
    let (sw, sh) = src_size;
    if sw == 0 || sh == 0 || src.len() < sw as usize * sh as usize * 4 {
        return;
    }
    let rect = fit_rect(src_size, dst);
    let sample = |sx: f64, sy: f64, out_px: &mut [u8]| {
        let x0 = sx.floor().clamp(0.0, f64::from(sw - 1)) as u32;
        let y0 = sy.floor().clamp(0.0, f64::from(sh - 1)) as u32;
        let x1 = (x0 + 1).min(sw - 1);
        let y1 = (y0 + 1).min(sh - 1);
        let fx = (sx - f64::from(x0)).clamp(0.0, 1.0);
        let fy = (sy - f64::from(y0)).clamp(0.0, 1.0);
        let at = |x: u32, y: u32| (y as usize * sw as usize + x as usize) * 4;
        let (a, b, c, d) = (at(x0, y0), at(x1, y0), at(x0, y1), at(x1, y1));
        for ch in 0..4 {
            let top = f64::from(src[a + ch]) * (1.0 - fx) + f64::from(src[b + ch]) * fx;
            let bottom = f64::from(src[c + ch]) * (1.0 - fx) + f64::from(src[d + ch]) * fx;
            out_px[ch] = (top * (1.0 - fy) + bottom * fy).round() as u8;
        }
    };

    let step_x = f64::from(sw) / f64::from(rect.w.max(1));
    let step_y = f64::from(sh) / f64::from(rect.h.max(1));
    for row in 0..rect.h {
        // Sample the pixel CENTRE of each destination texel, or the whole image shifts half a source pixel toward the origin.
        let sy = (f64::from(row) + 0.5) * step_y - 0.5;
        let base = ((rect.y + row) as usize * dw as usize + rect.x as usize) * 4;
        for col in 0..rect.w {
            let sx = (f64::from(col) + 0.5) * step_x - 0.5;
            let at = base + col as usize * 4;
            let Some(px) = out.get_mut(at..at + 4) else {
                continue;
            };
            sample(sx, sy, px);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(w: u32, h: u32, colour: [u8; 4]) -> Vec<u8> {
        colour.repeat(w as usize * h as usize)
    }

    #[test]
    fn an_equal_aspect_fills_the_frame_at_any_size() {
        let r = fit_rect((1280, 720), (1920, 1080));
        assert!(r.fills);
        assert_eq!((r.x, r.y, r.w, r.h), (0, 0, 1920, 1080));
    }

    #[test]
    fn a_wider_source_leaves_bars_above_and_below() {
        let r = fit_rect((1920, 1080), (1000, 1000));
        assert!(!r.fills);
        assert_eq!((r.w, r.h), (1000, 563));
        assert_eq!(r.x, 0);
        assert!(r.y > 0);
    }

    #[test]
    fn a_taller_source_leaves_bars_at_the_sides() {
        let r = fit_rect((1080, 1920), (1000, 1000));
        assert_eq!((r.w, r.h), (563, 1000));
        assert!(r.x > 0);
        assert_eq!(r.y, 0);
    }

    /// A source arriving with no area is what a backend hands over while it is
    /// still coming back; it must not divide by zero or panic the pump.
    #[test]
    fn a_degenerate_size_is_treated_as_filling() {
        assert!(fit_rect((0, 0), (100, 100)).fills);
        assert!(fit_rect((100, 100), (0, 0)).fills);
    }

    #[test]
    fn the_fitted_rect_never_leaves_the_frame() {
        for src in [(1, 10_000), (10_000, 1), (3, 7), (1920, 1080)] {
            let r = fit_rect(src, (1920, 1080));
            assert!(r.x + r.w <= 1920 && r.y + r.h <= 1080, "{src:?} gave {r:?}");
            assert!(r.w >= 1 && r.h >= 1, "{src:?} collapsed: {r:?}");
        }
    }

    #[test]
    fn the_output_is_always_exactly_the_frame_the_encoder_expects() {
        let mut out = Vec::new();
        fit_into(
            &mut out,
            &solid(64, 32, [1, 2, 3, 255]),
            (64, 32),
            (100, 100),
        );
        assert_eq!(out.len(), 100 * 100 * 4);
    }

    #[test]
    fn a_scaled_source_keeps_its_colour() {
        let mut out = Vec::new();
        fit_into(
            &mut out,
            &solid(64, 36, [10, 20, 30, 255]),
            (64, 36),
            (128, 72),
        );
        assert_eq!(&out[..4], &[10, 20, 30, 255]);
        assert_eq!(&out[out.len() - 4..], &[10, 20, 30, 255]);
    }

    /// The margins are what makes this a letterbox rather than a stretch.
    #[test]
    fn the_margins_are_left_black() {
        let mut out = Vec::new();
        fit_into(
            &mut out,
            &solid(100, 50, [255, 255, 255, 255]),
            (100, 50),
            (100, 100),
        );
        let row = |y: usize| out[y * 100 * 4..y * 100 * 4 + 4].to_vec();
        assert_eq!(row(0), vec![0, 0, 0, 0], "the top bar was painted");
        assert_eq!(row(99), vec![0, 0, 0, 0], "the bottom bar was painted");
        assert_eq!(row(50), vec![255, 255, 255, 255], "the picture is missing");
    }

    /// A source shorter than it claims is a truncated read; the pump must get a
    /// black frame rather than an index panic mid-recording.
    #[test]
    fn a_short_source_yields_a_blank_frame_rather_than_panicking() {
        let mut out = Vec::new();
        fit_into(&mut out, &[0u8; 16], (64, 32), (100, 100));
        assert_eq!(out.len(), 100 * 100 * 4);
        assert!(out.iter().all(|&b| b == 0));
    }

    /// Centre sampling: a gradient scaled up must stay centred rather than
    /// drifting half a source pixel toward the origin.
    #[test]
    fn a_gradient_keeps_its_midpoint_when_scaled() {
        let (w, h) = (8u32, 8u32);
        let mut src = Vec::new();
        for y in 0..h {
            for _ in 0..w {
                let v = (y * 255 / (h - 1)) as u8;
                src.extend_from_slice(&[v, v, v, 255]);
            }
        }
        let mut out = Vec::new();
        fit_into(&mut out, &src, (w, h), (64, 64));
        let mid = out[(32 * 64 + 32) * 4];
        assert!(mid.abs_diff(127) <= 3, "midpoint drifted to {mid}");
    }
}
