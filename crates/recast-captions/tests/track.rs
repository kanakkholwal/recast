use recast_captions::{active_cue, CaptionCue, CaptionTrack, TranscriptWord};

/// The shape the editor sends: the transcript as stored, extra keys and all.
const TRANSCRIPT: &str = r#"{
  "engine": "parakeet",
  "modelId": "v3",
  "language": "en",
  "segments": [
    { "id": "s0", "start": 1.0, "end": 2.0, "text": "one two",
      "words": [{"start":1.0,"end":1.4,"text":"one"},{"start":1.5,"end":1.9,"text":"two"}] },
    { "id": "s1", "start": 4.0, "end": 5.0, "text": "three",
      "words": [{"start":4.0,"end":4.9,"text":"three"}] }
  ]
}"#;

#[test]
fn the_transcript_the_editor_sends_deserializes_into_a_track() {
    let track: CaptionTrack = serde_json::from_str(TRANSCRIPT).expect("transcript");
    assert_eq!(track.segments.len(), 2);
    assert_eq!(track.segments[0].words.len(), 2);
}

/// The wasm preview documented a bare word array before the track carried
/// segments, and a project saved then still sends one.
#[test]
fn a_bare_word_array_still_loads_as_one_cue() {
    let json = r#"[{"start":1.0,"end":1.4,"text":"one"},{"start":1.5,"end":1.9,"text":"two"}]"#;
    let track: CaptionTrack = serde_json::from_str(json).expect("words");
    assert_eq!(track.segments.len(), 1);
    assert_eq!(track.segments[0].start, 1.0);
    assert_eq!(track.segments[0].end, 1.9);
}

#[test]
fn a_track_of_empty_cues_counts_as_empty() {
    let track = CaptionTrack {
        segments: vec![CaptionCue {
            start: 0.0,
            end: 1.0,
            words: Vec::new(),
        }],
    };
    assert!(track.is_empty());
    assert!(CaptionTrack::from(Vec::<TranscriptWord>::new()).is_empty());
}

#[test]
fn a_time_between_two_cues_is_in_neither() {
    let track: CaptionTrack = serde_json::from_str(TRANSCRIPT).expect("transcript");
    assert!(active_cue(&track.segments, 3.0).is_none());
    assert_eq!(active_cue(&track.segments, 1.5).map(|c| c.start), Some(1.0));
    assert_eq!(active_cue(&track.segments, 4.5).map(|c| c.start), Some(4.0));
}

/// A cue is half-open, so a word starting exactly where the previous segment
/// ends belongs to the new one rather than flickering between both.
#[test]
fn a_cues_end_belongs_to_the_next_cue() {
    let track: CaptionTrack = serde_json::from_str(TRANSCRIPT).expect("transcript");
    assert!(active_cue(&track.segments, 2.0).is_none());
    assert_eq!(active_cue(&track.segments, 4.0).map(|c| c.start), Some(4.0));
}
