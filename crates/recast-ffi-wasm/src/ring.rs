/// Index of the newest slot in `[floor_us, ts_us]`.
///
/// The floor is load-bearing: a frame before the current segment's start belongs
/// to a removed cut, and showing one steps the picture back into deleted content
/// at every cut boundary. Mirrors `pickSlot` in
/// `packages/editor/src/lib/playback/frame-textures.ts`.
pub fn pick_slot(timestamps: &[i64], ts_us: i64, floor_us: i64) -> Option<usize> {
    let mut best: Option<(usize, i64)> = None;
    for (index, &ts) in timestamps.iter().enumerate() {
        if ts < 0 || ts > ts_us || ts < floor_us {
            continue;
        }
        match best {
            Some((_, best_ts)) if ts <= best_ts => {}
            _ => best = Some((index, ts)),
        }
    }
    best.map(|(index, _)| index)
}

#[cfg(test)]
mod tests {
    use super::*;

    const EMPTY: i64 = -1;

    #[test]
    fn the_newest_frame_at_or_before_the_playhead_wins() {
        assert_eq!(pick_slot(&[100, 300, 200], 350, 0), Some(1));
        assert_eq!(pick_slot(&[100, 300, 200], 250, 0), Some(2));
    }

    #[test]
    fn a_frame_from_the_future_is_not_shown_early() {
        assert_eq!(pick_slot(&[100, 900], 500, 0), Some(0));
        assert_eq!(pick_slot(&[900], 500, 0), None);
    }

    /// Frames before the floor belong to a cut that was removed, so showing one
    /// steps the picture back into deleted content at the boundary.
    #[test]
    fn a_frame_from_before_the_current_segment_is_refused() {
        assert_eq!(pick_slot(&[100, 400], 500, 300), Some(1));
        assert_eq!(pick_slot(&[100, 200], 500, 300), None);
    }

    #[test]
    fn empty_slots_are_skipped_rather_than_treated_as_time_zero() {
        assert_eq!(pick_slot(&[EMPTY, EMPTY, 50], 500, 0), Some(2));
        assert_eq!(pick_slot(&[EMPTY, EMPTY], 500, 0), None);
        assert_eq!(pick_slot(&[], 500, 0), None);
    }

    /// Two slots holding the same timestamp must resolve the same way every
    /// call, or the picture flickers between two copies of one frame.
    #[test]
    fn a_tie_resolves_to_the_first_slot_every_time() {
        assert_eq!(pick_slot(&[200, 200, 200], 500, 0), Some(0));
    }

    #[test]
    fn a_frame_exactly_on_the_playhead_or_the_floor_is_eligible() {
        assert_eq!(pick_slot(&[500], 500, 0), Some(0));
        assert_eq!(pick_slot(&[300], 500, 300), Some(0));
    }
}
