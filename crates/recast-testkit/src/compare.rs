#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrameDelta {
    pub max_channel: u8,
    pub mean_channel: f64,
    pub differing_pixels: u64,
    pub total_pixels: u64,
}

impl FrameDelta {
    pub fn is_within(&self, max_channel: u8, max_mean: f64) -> bool {
        self.max_channel <= max_channel && self.mean_channel <= max_mean
    }
}

/// Compares two RGBA buffers channel-wise. Alpha is included: a compositor bug
/// that only shows in alpha is still a bug.
pub fn frame_delta(a: &[u8], b: &[u8]) -> Option<FrameDelta> {
    if a.len() != b.len() || a.is_empty() || !a.len().is_multiple_of(4) {
        return None;
    }
    let mut max_channel = 0u8;
    let mut total = 0u64;
    let mut differing_pixels = 0u64;

    for (pa, pb) in a.chunks_exact(4).zip(b.chunks_exact(4)) {
        let mut pixel_differs = false;
        for channel in 0..4 {
            let d = pa[channel].abs_diff(pb[channel]);
            if d > 0 {
                pixel_differs = true;
            }
            max_channel = max_channel.max(d);
            total += d as u64;
        }
        if pixel_differs {
            differing_pixels += 1;
        }
    }

    let total_pixels = (a.len() / 4) as u64;
    Some(FrameDelta {
        max_channel,
        mean_channel: total as f64 / (total_pixels * 4) as f64,
        differing_pixels,
        total_pixels,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_buffers_have_no_delta() {
        let buf = vec![7u8; 64];
        let delta = frame_delta(&buf, &buf).unwrap();
        assert_eq!(delta.max_channel, 0);
        assert_eq!(delta.differing_pixels, 0);
        assert!(delta.is_within(0, 0.0));
    }

    #[test]
    fn one_off_pixel_is_reported_without_drowning_in_the_mean() {
        let a = vec![0u8; 4000];
        let mut b = a.clone();
        b[0] = 255;
        let delta = frame_delta(&a, &b).unwrap();
        assert_eq!(delta.max_channel, 255);
        assert_eq!(delta.differing_pixels, 1);
        assert!(delta.mean_channel < 1.0);
        assert!(
            !delta.is_within(8, 1.0),
            "a 255 spike must not pass a max of 8"
        );
    }

    #[test]
    fn alpha_only_differences_are_not_ignored() {
        let a = vec![10u8, 20, 30, 255];
        let b = vec![10u8, 20, 30, 0];
        assert_eq!(frame_delta(&a, &b).unwrap().max_channel, 255);
    }

    #[test]
    fn mismatched_or_empty_buffers_are_rejected() {
        assert!(frame_delta(&[0u8; 4], &[0u8; 8]).is_none());
        assert!(frame_delta(&[], &[]).is_none());
        assert!(frame_delta(&[0u8; 3], &[0u8; 3]).is_none());
    }
}
