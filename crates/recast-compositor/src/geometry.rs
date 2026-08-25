pub const MAX_PADDING_PCT: f64 = 20.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CanvasGeometry {
    pub canvas_w: u32,
    pub canvas_h: u32,
    pub video_x: u32,
    pub video_y: u32,
    pub video_w: u32,
    pub video_h: u32,
    pub padding_px: u32,
    pub comp_x: u32,
    pub comp_y: u32,
    pub comp_w: u32,
    pub comp_h: u32,
}

pub fn parse_aspect_ratio(label: Option<&str>) -> Option<f64> {
    match label.unwrap_or("source") {
        "16:9" => Some(16.0 / 9.0),
        "9:16" => Some(9.0 / 16.0),
        "1:1" => Some(1.0),
        "1.91:1" => Some(1.91),
        _ => None,
    }
}

pub fn canvas_geometry(
    src_w: u32,
    src_h: u32,
    padding_pct: f64,
    output_aspect: Option<&str>,
) -> CanvasGeometry {
    let pct = padding_pct.clamp(0.0, MAX_PADDING_PCT);
    let shorter = src_w.min(src_h) as f64;
    let padding_px = ((shorter * pct) / 100.0).round() as u32;

    let comp_w = src_w + padding_px * 2;
    let comp_h = src_h + padding_px * 2;

    let mut canvas_w = comp_w;
    let mut canvas_h = comp_h;
    if let Some(target) = parse_aspect_ratio(output_aspect) {
        if comp_w > 0 && comp_h > 0 {
            let comp_aspect = comp_w as f64 / comp_h as f64;
            if comp_aspect > target {
                canvas_h = ((comp_w as f64) / target).round() as u32;
            } else if comp_aspect < target {
                canvas_w = ((comp_h as f64) * target).round() as u32;
            }
        }
    }

    // Even alignment: H.264 chroma subsampling needs it, and the old pad filter did too.
    canvas_w = (canvas_w + 1) & !1;
    canvas_h = (canvas_h + 1) & !1;

    let comp_x = canvas_w.saturating_sub(comp_w) / 2;
    let comp_y = canvas_h.saturating_sub(comp_h) / 2;

    CanvasGeometry {
        canvas_w,
        canvas_h,
        video_x: comp_x + padding_px,
        video_y: comp_y + padding_px,
        video_w: src_w,
        video_h: src_h,
        padding_px,
        comp_x,
        comp_y,
        comp_w,
        comp_h,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_padding_and_no_aspect_is_the_source_size() {
        let g = canvas_geometry(1920, 1080, 0.0, None);
        assert_eq!((g.canvas_w, g.canvas_h), (1920, 1080));
        assert_eq!((g.video_x, g.video_y), (0, 0));
    }

    #[test]
    fn padding_is_a_percent_of_the_shorter_edge_on_all_four_sides() {
        let g = canvas_geometry(1920, 1080, 10.0, None);
        assert_eq!(g.padding_px, 108);
        assert_eq!((g.canvas_w, g.canvas_h), (1920 + 216, 1080 + 216));
        assert_eq!((g.video_x, g.video_y), (108, 108));
    }

    #[test]
    fn padding_is_clamped_to_twenty_percent() {
        let a = canvas_geometry(1000, 1000, 20.0, None);
        let b = canvas_geometry(1000, 1000, 500.0, None);
        assert_eq!(a, b);
    }

    #[test]
    fn a_portrait_target_extends_the_canvas_height_and_centres_the_comp() {
        let g = canvas_geometry(1920, 1080, 0.0, Some("9:16"));
        assert_eq!(g.canvas_w, 1920);
        assert_eq!(g.canvas_h, 3414);
        assert_eq!(g.comp_x, 0);
        assert!(g.comp_y > 0);
        assert_eq!(g.comp_y, (g.canvas_h - 1080) / 2);
    }

    #[test]
    fn a_landscape_target_on_a_portrait_source_extends_the_width() {
        let g = canvas_geometry(1080, 1920, 0.0, Some("16:9"));
        assert_eq!(g.canvas_h, 1920);
        assert!(g.canvas_w > 1080);
        assert_eq!(g.comp_y, 0);
    }

    #[test]
    fn the_canvas_is_always_even() {
        for (w, h) in [(1919u32, 1079u32), (101, 203), (1, 1)] {
            let g = canvas_geometry(w, h, 3.0, None);
            assert_eq!(g.canvas_w % 2, 0, "{w}x{h}");
            assert_eq!(g.canvas_h % 2, 0, "{w}x{h}");
        }
    }

    #[test]
    fn an_unrecognised_aspect_label_falls_back_to_source() {
        assert_eq!(
            canvas_geometry(1920, 1080, 0.0, Some("banana")),
            canvas_geometry(1920, 1080, 0.0, None)
        );
        assert_eq!(parse_aspect_ratio(Some("source")), None);
    }

    #[test]
    fn a_square_target_matches_the_longer_edge() {
        let g = canvas_geometry(1920, 1080, 0.0, Some("1:1"));
        assert_eq!((g.canvas_w, g.canvas_h), (1920, 1920));
    }
}
