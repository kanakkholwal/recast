use recast_cursor::{CursorSample, CursorTrack, IdlePeriod};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TrackPayload {
    #[serde(default)]
    samples: Vec<CursorSample>,
    #[serde(default)]
    idle_periods: Vec<IdlePeriod>,
}

/// Accepts the recorded track file as-is. Kept out of the `wasm_bindgen` layer
/// so it is testable on the host.
pub fn parse_track(json: &str) -> Result<CursorTrack, serde_json::Error> {
    let payload: TrackPayload = serde_json::from_str(json)?;
    Ok(CursorTrack::new(payload.samples, payload.idle_periods))
}

#[cfg(test)]
mod tests {
    use super::*;

    const TRACK: &str = r##"{
        "samples": [
            { "timestampUs": 0, "x": 10, "y": 20, "visible": true, "leftDown": false, "rightDown": false },
            { "timestampUs": 1000, "x": 30, "y": 40, "visible": true, "leftDown": true, "rightDown": false },
            { "timestampUs": 2000, "x": 30, "y": 40, "visible": true, "leftDown": false, "rightDown": false }
        ],
        "idlePeriods": [{ "startUs": 2000, "endUs": 9000, "x": 30, "y": 40 }]
    }"##;

    #[test]
    fn the_recorded_track_file_parses_as_written() {
        let track = parse_track(TRACK).expect("track");
        assert_eq!(track.samples.len(), 3);
        assert_eq!(track.idle_periods.len(), 1);
    }

    /// The press events are derived, not stored, so a track that arrives without
    /// them must still have clicks.
    #[test]
    fn parsing_derives_the_press_events_rather_than_leaving_them_empty() {
        let track = parse_track(TRACK).expect("track");
        assert_eq!(track.press_events().len(), 1);
        assert_eq!(track.press_events()[0].down_us, 1000);
    }

    #[test]
    fn a_track_with_no_idle_periods_is_accepted() {
        let track = parse_track(r#"{"samples":[]}"#).expect("track");
        assert!(track.is_empty());
    }

    #[test]
    fn a_malformed_track_is_an_error_rather_than_an_empty_one() {
        assert!(parse_track("{not json").is_err());
    }
}
