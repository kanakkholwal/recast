use recast_compositor::CursorSlot;

/// The wire name for a pointer sprite slot. Kept out of the `wasm32`-gated
/// module so the rejection path is testable on the host.
pub fn parse_slot(slot: &str) -> Result<CursorSlot, String> {
    match slot {
        "rest" => Ok(CursorSlot::Rest),
        "press" => Ok(CursorSlot::Press),
        "rightPress" => Ok(CursorSlot::RightPress),
        "drag" => Ok(CursorSlot::Drag),
        other => Err(format!(
            "unknown cursor slot {other:?}: expected \"rest\", \"press\", \"rightPress\" or \"drag\""
        )),
    }
}

pub fn slot_at(index: usize) -> Option<CursorSlot> {
    [
        CursorSlot::Rest,
        CursorSlot::Press,
        CursorSlot::RightPress,
        CursorSlot::Drag,
    ]
    .get(index)
    .copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_slot_name_maps_to_its_own_slot() {
        for (name, slot) in [
            ("rest", CursorSlot::Rest),
            ("press", CursorSlot::Press),
            ("rightPress", CursorSlot::RightPress),
            ("drag", CursorSlot::Drag),
        ] {
            assert_eq!(parse_slot(name), Ok(slot));
        }
    }

    /// A typo must not silently land on `rest`, which would show the wrong
    /// pointer for the whole press.
    #[test]
    fn an_unknown_slot_name_is_rejected() {
        let err = parse_slot("rightpress").expect_err("should reject");
        assert!(err.contains("rightpress"), "{err}");
    }

    /// The index round trip is what the render loop walks, so a mismatch would
    /// upload a press sprite and draw it as the rest one.
    #[test]
    fn the_index_round_trips_back_to_the_same_slot() {
        for name in ["rest", "press", "rightPress", "drag"] {
            let slot = parse_slot(name).expect("a slot");
            assert_eq!(slot_at(slot.index()), Some(slot));
        }
    }

    #[test]
    fn an_index_past_the_last_slot_is_none() {
        assert_eq!(slot_at(4), None);
    }
}
