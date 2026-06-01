// SPDX-License-Identifier: GPL-2.0-only

//! Progressive backoff for transient (network) failures.
//!
//! Mirrors the upstream agent: after a failed server contact the next attempt
//! is delayed, doubling each consecutive failure from a base (60 s) up to a
//! cap, and resetting to no delay on the first success.

use std::time::Duration;

/// A doubling backoff counter.
#[derive(Debug, Clone)]
pub struct Backoff {
    base: Duration,
    max: Duration,
    failures: u32,
}

impl Backoff {
    /// Creates a backoff doubling from `base`, capped at `max`.
    #[must_use]
    pub fn new(base: Duration, max: Duration) -> Self {
        Self {
            base,
            max,
            failures: 0,
        }
    }

    /// The upstream default: start at 60 s, double, cap at one hour.
    #[must_use]
    pub fn network() -> Self {
        Self::new(Duration::from_secs(60), Duration::from_secs(3600))
    }

    /// Records a failure and returns the delay before the next attempt.
    ///
    /// The first failure yields `base`, the next `2×base`, then `4×base`, …,
    /// saturating at `max`.
    pub fn record_failure(&mut self) -> Duration {
        self.failures = self.failures.saturating_add(1);
        self.current_delay()
    }

    /// Resets the counter after a success; subsequent [`current_delay`] is zero.
    ///
    /// [`current_delay`]: Self::current_delay
    pub fn reset(&mut self) {
        self.failures = 0;
    }

    /// The number of consecutive failures recorded.
    #[must_use]
    pub fn failures(&self) -> u32 {
        self.failures
    }

    /// The current delay for the recorded number of failures (zero when none).
    #[must_use]
    pub fn current_delay(&self) -> Duration {
        if self.failures == 0 {
            return Duration::ZERO;
        }
        let factor = 2u64.saturating_pow(self.failures - 1);
        let secs = self.base.as_secs().saturating_mul(factor);
        Duration::from_secs(secs.min(self.max.as_secs()))
    }
}

#[cfg(test)]
mod tests {
    use super::Backoff;
    use std::time::Duration;

    #[test]
    fn doubles_from_base_and_caps() {
        let mut backoff = Backoff::new(Duration::from_secs(60), Duration::from_secs(3600));
        assert_eq!(backoff.current_delay(), Duration::ZERO);
        assert_eq!(backoff.record_failure(), Duration::from_secs(60));
        assert_eq!(backoff.record_failure(), Duration::from_secs(120));
        assert_eq!(backoff.record_failure(), Duration::from_secs(240));
        assert_eq!(backoff.record_failure(), Duration::from_secs(480));
        assert_eq!(backoff.record_failure(), Duration::from_secs(960));
        assert_eq!(backoff.record_failure(), Duration::from_secs(1920));
        // Next would be 3840 s, capped at 3600.
        assert_eq!(backoff.record_failure(), Duration::from_secs(3600));
        assert_eq!(backoff.failures(), 7);
    }

    #[test]
    fn reset_clears_the_delay() {
        let mut backoff = Backoff::network();
        backoff.record_failure();
        backoff.record_failure();
        backoff.reset();
        assert_eq!(backoff.failures(), 0);
        assert_eq!(backoff.current_delay(), Duration::ZERO);
    }

    #[test]
    fn many_failures_do_not_overflow() {
        let mut backoff = Backoff::new(Duration::from_secs(60), Duration::from_secs(3600));
        for _ in 0..100 {
            backoff.record_failure();
        }
        assert_eq!(backoff.current_delay(), Duration::from_secs(3600));
    }
}
