// SPDX-License-Identifier: GPL-2.0-only

//! Run scheduling: when a target is next due.
//!
//! A [`RunSchedule`] tracks the next-run time for a target. The first run is
//! offset by an initial delay (the `delaytime` jitter) so a fleet of agents
//! does not stampede the server; subsequent runs are spaced by `period`. After
//! a failure the caller can defer the next run using a backoff delay instead.

use std::time::Duration;

use chrono::{DateTime, TimeDelta, Utc};

/// Converts a [`std::time::Duration`] to a [`TimeDelta`] at second granularity,
/// saturating rather than overflowing.
fn to_delta(duration: Duration) -> TimeDelta {
    TimeDelta::try_seconds(i64::try_from(duration.as_secs()).unwrap_or(i64::MAX))
        .unwrap_or(TimeDelta::MAX)
}

/// Computes an initial jitter delay in `[0, max)` from a fraction in `[0, 1)`.
///
/// The caller supplies the fraction (from an RNG in production, a fixed value
/// in tests), keeping the schedule itself deterministic.
#[must_use]
pub fn jitter(max: Duration, fraction: f64) -> Duration {
    let fraction = fraction.clamp(0.0, 1.0);
    Duration::from_secs_f64(max.as_secs_f64() * fraction)
}

/// Tracks the next-run time of a single target.
#[derive(Debug, Clone)]
pub struct RunSchedule {
    period: Duration,
    next_run: DateTime<Utc>,
}

impl RunSchedule {
    /// Creates a schedule whose first run is `initial_delay` after `now`.
    #[must_use]
    pub fn new(now: DateTime<Utc>, period: Duration, initial_delay: Duration) -> Self {
        Self {
            period,
            next_run: now + to_delta(initial_delay),
        }
    }

    /// The time the target is next due to run.
    #[must_use]
    pub fn next_run(&self) -> DateTime<Utc> {
        self.next_run
    }

    /// Whether the target is due at `now`.
    #[must_use]
    pub fn is_due(&self, now: DateTime<Utc>) -> bool {
        now >= self.next_run
    }

    /// Schedules the next run one `period` after `now` (call after a success).
    pub fn schedule_next(&mut self, now: DateTime<Utc>) {
        self.next_run = now + to_delta(self.period);
    }

    /// Defers the next run by `delay` from `now` (call after a failure, using a
    /// [`Backoff`](crate::Backoff) delay).
    pub fn defer(&mut self, now: DateTime<Utc>, delay: Duration) {
        self.next_run = now + to_delta(delay);
    }

    /// Forces the target due immediately (a `runnow` event).
    pub fn run_now(&mut self, now: DateTime<Utc>) {
        self.next_run = now;
    }
}

#[cfg(test)]
mod tests {
    use super::{jitter, RunSchedule};
    use chrono::{TimeZone, Utc};
    use std::time::Duration;

    fn epoch() -> chrono::DateTime<Utc> {
        Utc.timestamp_opt(1_000_000_000, 0).unwrap()
    }

    #[test]
    fn first_run_is_offset_by_initial_delay() {
        let now = epoch();
        let schedule = RunSchedule::new(now, Duration::from_secs(3600), Duration::from_secs(30));
        assert_eq!(schedule.next_run(), now + chrono::TimeDelta::seconds(30));
        assert!(!schedule.is_due(now));
        assert!(schedule.is_due(now + chrono::TimeDelta::seconds(30)));
    }

    #[test]
    fn schedule_next_spaces_by_period() {
        let now = epoch();
        let mut schedule = RunSchedule::new(now, Duration::from_secs(3600), Duration::ZERO);
        assert!(schedule.is_due(now));
        schedule.schedule_next(now);
        assert_eq!(schedule.next_run(), now + chrono::TimeDelta::seconds(3600));
    }

    #[test]
    fn defer_and_run_now() {
        let now = epoch();
        let mut schedule = RunSchedule::new(now, Duration::from_secs(3600), Duration::ZERO);
        schedule.defer(now, Duration::from_secs(120));
        assert_eq!(schedule.next_run(), now + chrono::TimeDelta::seconds(120));
        schedule.run_now(now);
        assert!(schedule.is_due(now));
    }

    #[test]
    fn jitter_scales_with_fraction_and_clamps() {
        assert_eq!(jitter(Duration::from_secs(100), 0.0), Duration::ZERO);
        assert_eq!(
            jitter(Duration::from_secs(100), 0.5),
            Duration::from_secs(50)
        );
        // Out-of-range fractions are clamped.
        assert_eq!(
            jitter(Duration::from_secs(100), 2.0),
            Duration::from_secs(100)
        );
        assert_eq!(jitter(Duration::from_secs(100), -1.0), Duration::ZERO);
    }
}
