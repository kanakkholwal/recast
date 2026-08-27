use core::time::Duration;

use crate::time::Timestamp;

/// How the output timeline relates to what the source actually produced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pacing {
    /// Hold a true constant frame rate, repeating the last frame when the source
    /// produces nothing.
    ///
    /// A desktop that is not changing emits no frames at all on Desktop
    /// Duplication or the portal, so a recording that just forwards the source
    /// has gaps wherever the user paused. This fills them.
    Constant {
        /// Frames per second.
        fps: u32,
    },
    /// Forward only what the source produced, with the timestamps it gave.
    ///
    /// Nothing is invented, so the stream shows exactly when the screen changed.
    Passthrough,
}

impl Default for Pacing {
    fn default() -> Self {
        Self::Constant { fps: 60 }
    }
}

impl Pacing {
    /// Frames per second, or `None` when the source sets the rate.
    #[must_use]
    pub const fn fps(self) -> Option<u32> {
        match self {
            Self::Constant { fps } => Some(fps),
            Self::Passthrough => None,
        }
    }
}

/// The default ceiling on how many slots one catch-up may emit.
const DEFAULT_MAX_CATCH_UP: u32 = 4;

/// Turns wall-clock time into a constant-rate timeline.
///
/// Slot `n` is always `origin + n * interval`, computed from the origin rather
/// than by adding an interval to the previous slot. Accumulating would drift:
/// 60 fps is 16 666 666.66 ns, so a running sum is a millisecond out after a
/// minute and the audio and video tracks visibly separate.
#[derive(Debug, Clone)]
pub struct Pacer {
    interval: Duration,
    origin: Option<Timestamp>,
    emitted: u64,
    skipped: u64,
    max_catch_up: u32,
}

impl Pacer {
    /// A pacer for `fps`, which is clamped to at least 1.
    #[must_use]
    pub fn new(fps: u32) -> Self {
        let fps = u64::from(fps.max(1));
        Self {
            interval: Duration::from_nanos(1_000_000_000 / fps),
            origin: None,
            emitted: 0,
            skipped: 0,
            max_catch_up: DEFAULT_MAX_CATCH_UP,
        }
    }

    /// How many slots a single catch-up may emit before the rest are skipped.
    ///
    /// A process that stalls for five seconds at 60 fps owes 300 frames. Emitting
    /// them all turns a hiccup into a memory spike and a wall of duplicate frames,
    /// so the backlog is dropped and counted instead.
    pub fn set_max_catch_up(&mut self, slots: u32) {
        self.max_catch_up = slots.max(1);
    }

    /// Time between slots.
    #[must_use]
    pub const fn interval(&self) -> Duration {
        self.interval
    }

    /// The next slot whose deadline has passed at `now`, or `None` if it is not
    /// yet due. Call in a loop until it returns `None`.
    pub fn next_due(&mut self, now: Timestamp) -> Option<Timestamp> {
        let origin = *self.origin.get_or_insert(now);
        let elapsed = now.saturating_since(origin).as_nanos() as u64;
        let interval = self.interval.as_nanos() as u64;
        // Guarded at construction, but a caller could still reach here with a
        // pacer built from a zero-length interval on a platform with no timer.
        if interval == 0 {
            return None;
        }
        let due = elapsed / interval + 1;

        if self.emitted >= due {
            return None;
        }
        // Everything older than the catch-up window is abandoned rather than
        // emitted, so a stall costs frames instead of memory.
        let behind = due - self.emitted;
        if behind > u64::from(self.max_catch_up) {
            let dropped = behind - u64::from(self.max_catch_up);
            self.skipped += dropped;
            self.emitted += dropped;
        }

        let pts = origin.saturating_add(Duration::from_nanos(self.emitted * interval));
        self.emitted += 1;
        Some(pts)
    }

    /// When the next slot falls due, or `None` before the first call.
    #[must_use]
    pub fn next_deadline(&self) -> Option<Timestamp> {
        let origin = self.origin?;
        Some(origin.saturating_add(Duration::from_nanos(
            self.emitted * self.interval.as_nanos() as u64,
        )))
    }

    /// Slots handed out so far, which is the frame count of the output timeline.
    #[must_use]
    pub const fn emitted(&self) -> u64 {
        self.emitted
    }

    /// Slots abandoned to catch-up, for a caller that wants to report a stall.
    #[must_use]
    pub const fn skipped(&self) -> u64 {
        self.skipped
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(millis: u64) -> Timestamp {
        Timestamp::from_nanos((millis * 1_000_000) as i64)
    }

    #[test]
    fn the_first_call_starts_the_timeline_at_zero() {
        let mut pacer = Pacer::new(60);
        assert_eq!(pacer.next_due(at(1_000)), Some(at(1_000)));
        assert_eq!(
            pacer.next_due(at(1_000)),
            None,
            "one slot is due at the origin"
        );
    }

    #[test]
    fn sixty_fps_emits_a_slot_every_sixteen_and_a_bit_milliseconds() {
        let mut pacer = Pacer::new(60);
        pacer.next_due(at(0));
        assert_eq!(
            pacer.next_due(at(16)),
            None,
            "16ms is short of the second slot"
        );
        assert!(pacer.next_due(at(17)).is_some());
    }

    /// The bug this design exists to prevent: a pacer that schedules the next
    /// slot as `now + interval` drifts, because `now` is whenever the poll
    /// actually happened. Polling at irregular, late instants must still land
    /// every slot exactly on `origin + n * interval`.
    #[test]
    fn slots_stay_on_the_ideal_grid_however_late_the_polls_are() {
        let mut pacer = Pacer::new(60);
        pacer.set_max_catch_up(u32::MAX);
        let origin = at(1_000);
        let mut last = pacer.next_due(origin).expect("the first slot");

        for now in [at(1_017), at(1_040), at(1_051), at(1_099), at(2_500)] {
            while let Some(pts) = pacer.next_due(now) {
                last = pts;
            }
        }

        let slots = pacer.emitted() - 1;
        let interval = pacer.interval().as_nanos() as i64;
        assert_eq!(
            last.as_nanos() - origin.as_nanos(),
            slots as i64 * interval,
            "slot {slots} left the grid"
        );
    }

    #[test]
    fn a_stall_is_caught_up_only_to_the_bound() {
        let mut pacer = Pacer::new(60);
        pacer.set_max_catch_up(4);
        pacer.next_due(at(0));
        // Five seconds of stall owes 300 slots; only the bound is emitted.
        let mut emitted = 0;
        while pacer.next_due(at(5_000)).is_some() {
            emitted += 1;
        }
        assert_eq!(emitted, 4);
        assert!(pacer.skipped() > 250, "skipped {}", pacer.skipped());
    }

    #[test]
    fn catching_up_keeps_the_timeline_aligned_to_the_origin() {
        let mut pacer = Pacer::new(100);
        pacer.set_max_catch_up(u32::MAX);
        pacer.next_due(at(0));
        let mut last = Timestamp::ZERO;
        while let Some(pts) = pacer.next_due(at(50)) {
            last = pts;
        }
        // 50ms at 100fps is slots 0..=5, the last at exactly 50ms.
        assert_eq!(pacer.emitted(), 6);
        assert_eq!(last, at(50));
    }

    #[test]
    fn a_zero_fps_pacer_still_produces_a_usable_interval() {
        let pacer = Pacer::new(0);
        assert_eq!(pacer.interval(), Duration::from_secs(1));
    }

    #[test]
    fn the_next_deadline_is_unknown_until_the_timeline_starts() {
        let mut pacer = Pacer::new(30);
        assert_eq!(pacer.next_deadline(), None);
        pacer.next_due(at(500));
        assert_eq!(
            pacer.next_deadline(),
            Some(at(500).saturating_add(pacer.interval()))
        );
    }

    #[test]
    fn time_going_backwards_does_not_emit_a_slot() {
        let mut pacer = Pacer::new(60);
        pacer.next_due(at(1_000));
        assert_eq!(pacer.next_due(at(500)), None);
    }

    #[test]
    fn passthrough_pacing_names_no_rate() {
        assert_eq!(Pacing::Passthrough.fps(), None);
        assert_eq!(Pacing::Constant { fps: 30 }.fps(), Some(30));
    }

    #[test]
    fn recording_defaults_to_sixty() {
        assert_eq!(Pacing::default(), Pacing::Constant { fps: 60 });
    }
}
