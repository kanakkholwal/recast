use std::time::Instant;

use crate::Timestamp;

#[derive(Debug, Clone, Copy)]
pub struct SessionClock {
    origin: Instant,
}

impl SessionClock {
    pub fn start() -> Self {
        Self {
            origin: Instant::now(),
        }
    }

    pub fn now(&self) -> Timestamp {
        self.stamp(Instant::now())
    }

    pub fn stamp(&self, at: Instant) -> Timestamp {
        if at >= self.origin {
            Timestamp::from_nanos(saturating_nanos(at.duration_since(self.origin)))
        } else {
            Timestamp::from_nanos(-saturating_nanos(self.origin.duration_since(at)))
        }
    }

    pub fn origin(&self) -> Instant {
        self.origin
    }
}

impl Default for SessionClock {
    fn default() -> Self {
        Self::start()
    }
}

fn saturating_nanos(d: std::time::Duration) -> i64 {
    i64::try_from(d.as_nanos()).unwrap_or(i64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn origin_is_zero() {
        let clock = SessionClock::start();
        assert_eq!(clock.stamp(clock.origin()), Timestamp::ZERO);
    }

    #[test]
    fn stamps_before_the_origin_are_negative() {
        let clock = SessionClock::start();
        let before = clock.origin() - Duration::from_millis(40);
        assert_eq!(clock.stamp(before).as_millis(), -40);
    }

    #[test]
    fn stamps_are_monotonic() {
        let clock = SessionClock::start();
        let a = clock.now();
        let b = clock.now();
        assert!(b >= a);
    }

    #[test]
    fn a_stamp_measures_elapsed_time_from_the_origin() {
        let clock = SessionClock::start();
        let later = clock.origin() + Duration::from_secs(2);
        assert_eq!(clock.stamp(later).as_nanos(), 2_000_000_000);
    }
}
