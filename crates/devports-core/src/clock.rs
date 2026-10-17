//! Time, injected.
//!
//! Leases, warnings and reopen guards depend on time, and a test must never wait
//! for real time to pass (`PRUEBAS.md`, principle 3). Everything that reads the
//! clock takes a [`Clock`]; production passes [`SystemClock`], tests pass
//! [`ManualClock`] and move it by hand.
//!
//! Two readings, because they answer different questions: [`Clock::now`] is
//! monotonic and measures *how long* something took; [`Clock::wall`] is the
//! calendar time that is persisted (`expires_at`) and survives a restart.

use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use jiff::{SignedDuration, Timestamp};

/// A source of time.
pub trait Clock: Send + Sync {
    /// A monotonic instant, for measuring elapsed time.
    fn now(&self) -> Instant;

    /// The calendar time, for deadlines that are stored and compared later.
    fn wall(&self) -> Timestamp;
}

impl<C: Clock + ?Sized> Clock for std::sync::Arc<C> {
    fn now(&self) -> Instant {
        (**self).now()
    }

    fn wall(&self) -> Timestamp {
        (**self).wall()
    }
}

/// The real clock.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Instant {
        Instant::now()
    }

    fn wall(&self) -> Timestamp {
        Timestamp::now()
    }
}

/// A clock that only moves when told to.
#[derive(Debug)]
pub struct ManualClock {
    start: Instant,
    wall_start: Timestamp,
    elapsed: Mutex<Duration>,
}

impl ManualClock {
    /// Creates a clock that reads `wall_start` as its calendar time.
    #[must_use]
    pub fn new(wall_start: Timestamp) -> Self {
        Self {
            start: Instant::now(),
            wall_start,
            elapsed: Mutex::new(Duration::ZERO),
        }
    }

    /// Moves both readings forward by `by`.
    pub fn advance(&self, by: Duration) {
        let mut elapsed = self.elapsed.lock().unwrap_or_else(PoisonError::into_inner);
        *elapsed = elapsed.saturating_add(by);
    }

    fn elapsed(&self) -> Duration {
        *self.elapsed.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl Clock for ManualClock {
    fn now(&self) -> Instant {
        self.start.checked_add(self.elapsed()).unwrap_or(self.start)
    }

    fn wall(&self) -> Timestamp {
        SignedDuration::try_from(self.elapsed())
            .ok()
            .and_then(|elapsed| self.wall_start.checked_add(elapsed).ok())
            .unwrap_or(self.wall_start)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clock() -> ManualClock {
        ManualClock::new("2026-10-17T12:00:00Z".parse().unwrap())
    }

    #[test]
    fn a_manual_clock_stands_still_until_advanced() {
        let clock = clock();
        let (first, wall) = (clock.now(), clock.wall());
        assert_eq!(clock.now(), first);
        assert_eq!(clock.wall(), wall);
    }

    #[test]
    fn advancing_moves_both_readings_by_the_same_amount() {
        let clock = clock();
        let (first, wall) = (clock.now(), clock.wall());

        clock.advance(Duration::from_secs(90));
        clock.advance(Duration::from_millis(500));

        assert_eq!(
            clock.now().duration_since(first),
            Duration::from_millis(90_500)
        );
        assert_eq!(
            clock.wall(),
            "2026-10-17T12:01:30.5Z".parse::<Timestamp>().unwrap()
        );
        assert!(wall < clock.wall());
    }

    #[test]
    fn the_system_clock_reads_real_time() {
        let before = Timestamp::now();
        let reading = SystemClock.wall();
        assert!(reading >= before);
        let first = SystemClock.now();
        assert!(SystemClock.now() >= first);
    }
}
