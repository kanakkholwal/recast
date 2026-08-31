//! Sequenced event log fed by one set of listeners for the life of the process, so a reconnecting watcher replays instead of re-snapshotting.
//! Without it a `watch` client could not tell "nothing happened" from "I missed it".

use std::collections::VecDeque;
use std::sync::{Condvar, Mutex};
use std::time::Duration;

use serde::Serialize;
use serde_json::Value;

/// Events retained for replay. A watcher further behind than this is told it
/// lagged rather than being handed a silently incomplete stream.
pub const RING_CAPACITY: usize = 1024;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoggedEvent {
    pub seq: u64,
    pub event: String,
    pub data: Value,
}

/// What a watcher should send next, and what it will never see.
#[derive(Debug, Clone, PartialEq)]
pub struct Replay {
    pub events: Vec<LoggedEvent>,
    /// Cursor to pass to the next read.
    pub cursor: u64,
    /// Events dropped from the ring before this watcher asked for them.
    pub missed: u64,
}

#[derive(Debug, Default)]
struct Ring {
    next_seq: u64,
    entries: VecDeque<LoggedEvent>,
}

impl Ring {
    fn oldest_seq(&self) -> Option<u64> {
        self.entries.front().map(|entry| entry.seq)
    }
}

/// Append-only ring of recent events, shared by every `watch` connection.
#[derive(Debug)]
pub struct EventLog {
    ring: Mutex<Ring>,
    arrival: Condvar,
    capacity: usize,
}

impl Default for EventLog {
    fn default() -> Self {
        Self::with_capacity(RING_CAPACITY)
    }
}

impl EventLog {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            ring: Mutex::new(Ring::default()),
            arrival: Condvar::new(),
            capacity: capacity.max(1),
        }
    }

    /// Record one event and wake every waiting watcher.
    pub fn push(&self, event: impl Into<String>, data: Value) -> u64 {
        let mut ring = self.lock();
        ring.next_seq += 1;
        let seq = ring.next_seq;
        ring.entries.push_back(LoggedEvent {
            seq,
            event: event.into(),
            data,
        });
        while ring.entries.len() > self.capacity {
            ring.entries.pop_front();
        }
        drop(ring);
        self.arrival.notify_all();
        seq
    }

    /// Sequence number of the newest event; `0` before anything is logged.
    pub fn head(&self) -> u64 {
        self.lock().next_seq
    }

    /// Everything after `cursor` whose name is in `names`.
    ///
    /// `missed` counts events evicted before this watcher reached them, so a
    /// slow client learns its stream has a hole instead of assuming continuity.
    /// The returned cursor advances past filtered-out events too, so a watcher
    /// subscribed to one group is not dragged back by traffic on another.
    pub fn since(&self, cursor: u64, names: &[String]) -> Replay {
        let ring = self.lock();
        let missed = match ring.oldest_seq() {
            Some(oldest) if cursor + 1 < oldest => oldest - cursor - 1,
            _ => 0,
        };
        let events: Vec<LoggedEvent> = ring
            .entries
            .iter()
            .filter(|entry| entry.seq > cursor && names.iter().any(|name| name == &entry.event))
            .cloned()
            .collect();
        Replay {
            events,
            cursor: ring.next_seq.max(cursor),
            missed,
        }
    }

    /// Block until an event newer than `cursor` is logged, or `timeout` passes.
    /// Returns `false` on timeout, which the caller turns into a keepalive.
    pub fn wait_past(&self, cursor: u64, timeout: Duration) -> bool {
        let ring = self.lock();
        let (guard, outcome) = self
            .arrival
            .wait_timeout_while(ring, timeout, |ring| ring.next_seq <= cursor)
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        drop(guard);
        !outcome.timed_out()
    }

    /// A poisoned log is not worth crashing the app over: the worst case is a
    /// watcher seeing one duplicated or missing frame.
    fn lock(&self) -> std::sync::MutexGuard<'_, Ring> {
        self.ring
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn names(list: &[&str]) -> Vec<String> {
        list.iter().map(|name| (*name).to_string()).collect()
    }

    fn log_with(events: &[&str]) -> EventLog {
        let log = EventLog::default();
        for event in events {
            log.push(*event, json!({}));
        }
        log
    }

    mod push {
        use super::*;

        #[test]
        fn numbers_the_first_event_one() {
            assert_eq!(EventLog::default().push("a", json!({})), 1);
        }

        #[test]
        fn hands_out_increasing_sequence_numbers() {
            let log = EventLog::default();
            log.push("a", json!({}));

            assert_eq!(log.push("b", json!({})), 2);
        }

        #[test]
        fn head_tracks_the_newest_sequence_number() {
            assert_eq!(log_with(&["a", "b", "c"]).head(), 3);
        }

        #[test]
        fn head_is_zero_before_anything_is_logged() {
            assert_eq!(EventLog::default().head(), 0);
        }
    }

    mod since {
        use super::*;

        #[test]
        fn replays_everything_from_the_start() {
            let log = log_with(&["a", "b"]);

            assert_eq!(log.since(0, &names(&["a", "b"])).events.len(), 2);
        }

        #[test]
        fn skips_what_the_cursor_already_covers() {
            let log = log_with(&["a", "b", "c"]);

            let seqs: Vec<u64> = log
                .since(2, &names(&["a", "b", "c"]))
                .events
                .iter()
                .map(|event| event.seq)
                .collect();

            assert_eq!(seqs, vec![3]);
        }

        #[test]
        fn drops_events_outside_the_requested_names() {
            let log = log_with(&["a", "b", "a"]);

            assert_eq!(log.since(0, &names(&["a"])).events.len(), 2);
        }

        #[test]
        fn advances_the_cursor_past_filtered_out_events() {
            let log = log_with(&["a", "b", "b"]);

            assert_eq!(log.since(0, &names(&["a"])).cursor, 3);
        }

        #[test]
        fn reports_nothing_missed_when_the_ring_still_holds_the_cursor() {
            let log = log_with(&["a", "b"]);

            assert_eq!(log.since(1, &names(&["a", "b"])).missed, 0);
        }

        #[test]
        fn counts_events_evicted_before_the_cursor_caught_up() {
            let log = EventLog::with_capacity(2);
            for _ in 0..5 {
                log.push("a", json!({}));
            }

            assert_eq!(log.since(1, &names(&["a"])).missed, 2);
        }

        #[test]
        fn a_fresh_watcher_on_an_evicting_log_is_told_what_it_lost() {
            let log = EventLog::with_capacity(2);
            for _ in 0..5 {
                log.push("a", json!({}));
            }

            assert_eq!(log.since(0, &names(&["a"])).missed, 3);
        }

        #[test]
        fn never_rewinds_the_cursor_of_a_watcher_ahead_of_the_log() {
            let log = log_with(&["a"]);

            assert_eq!(log.since(9, &names(&["a"])).cursor, 9);
        }

        #[test]
        fn carries_the_event_payload_through() {
            let log = EventLog::default();
            log.push("a", json!({ "path": "p.recast" }));

            assert_eq!(
                log.since(0, &names(&["a"])).events[0].data["path"],
                "p.recast"
            );
        }
    }

    mod capacity {
        use super::*;

        #[test]
        fn keeps_only_the_newest_entries() {
            let log = EventLog::with_capacity(2);
            for _ in 0..5 {
                log.push("a", json!({}));
            }

            assert_eq!(log.since(0, &names(&["a"])).events.len(), 2);
        }

        #[test]
        fn the_retained_entries_are_the_latest_ones() {
            let log = EventLog::with_capacity(2);
            for _ in 0..5 {
                log.push("a", json!({}));
            }

            let seqs: Vec<u64> = log
                .since(0, &names(&["a"]))
                .events
                .iter()
                .map(|event| event.seq)
                .collect();

            assert_eq!(seqs, vec![4, 5]);
        }

        #[test]
        fn a_zero_capacity_request_still_retains_one() {
            let log = EventLog::with_capacity(0);
            log.push("a", json!({}));

            assert_eq!(log.since(0, &names(&["a"])).events.len(), 1);
        }
    }

    mod wait_past {
        use super::*;
        use std::sync::Arc;

        #[test]
        fn returns_immediately_when_the_log_is_already_ahead() {
            let log = log_with(&["a"]);

            assert!(log.wait_past(0, Duration::from_millis(50)));
        }

        #[test]
        fn reports_a_timeout_when_nothing_arrives() {
            let log = EventLog::default();

            assert!(!log.wait_past(0, Duration::from_millis(20)));
        }

        #[test]
        fn wakes_when_another_thread_pushes() {
            let log = Arc::new(EventLog::default());
            let writer = Arc::clone(&log);
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(10));
                writer.push("a", json!({}));
            });

            assert!(log.wait_past(0, Duration::from_secs(2)));
        }
    }
}
