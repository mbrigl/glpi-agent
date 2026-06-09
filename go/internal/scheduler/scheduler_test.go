// SPDX-License-Identifier: GPL-2.0-only

package scheduler

import (
	"testing"
	"time"
)

var base = time.Date(2026, 6, 8, 12, 0, 0, 0, time.UTC)

// fixed builds a schedule with a fixed clock and a deterministic rng (always 0
// jitter unless overridden), for predictable timing assertions.
func fixed(maxDelay, delaytime time.Duration, randn func(int64) int64) *Schedule {
	if randn == nil {
		randn = func(int64) int64 { return 0 }
	}
	now := base
	return New(maxDelay, delaytime,
		WithClock(func() time.Time { return now }),
		WithRand(randn),
	)
}

// TestInitialDelayStagger checks the first run is staggered by the delaytime
// (jittered down by up to delaytime/2).
func TestInitialDelayStagger(t *testing.T) {
	// No jitter -> first run exactly delaytime ahead.
	s := fixed(time.Hour, 10*time.Minute, nil)
	if got := s.NextRunDate().Sub(base); got != 10*time.Minute {
		t.Errorf("next run = %v, want 10m (initial delay, no jitter)", got)
	}

	// Max jitter (returns n-1 ~ initialDelay/2) -> roughly half the delay.
	maxJitter := func(n int64) int64 {
		if n <= 0 {
			return 0
		}
		return n // jitter() multiplies by second after dividing; returning n gives delay/2
	}
	s2 := fixed(time.Hour, 10*time.Minute, maxJitter)
	// initialDelay - jitter(initialDelay/2) = 600s - 300s = 300s.
	if got := s2.NextRunDate().Sub(base); got != 5*time.Minute {
		t.Errorf("next run with max jitter = %v, want 5m", got)
	}
}

// TestResetUsesMaxDelay checks that after a successful run the next run is one
// maxDelay out (no jitter), and the schedule is no longer due.
func TestResetUsesMaxDelay(t *testing.T) {
	s := fixed(2*time.Hour, 0, nil) // no delaytime -> no initial stagger
	// First run: now + maxDelay.
	if got := s.NextRunDate().Sub(base); got != 2*time.Hour {
		t.Fatalf("first run = %v, want 2h", got)
	}
	s.ResetNextRunDate()
	// baseRunDate was now+maxDelay; reset computes baseRunDate + maxDelay.
	if got := s.NextRunDate().Sub(base); got != 4*time.Hour {
		t.Errorf("reset run = %v, want 4h", got)
	}
}

// TestExpirationOverridesAndSkipsReset checks a server expiration sets the next
// run and the following reset is skipped (Target setNextRunOnExpiration).
func TestExpirationOverridesAndSkipsReset(t *testing.T) {
	s := fixed(time.Hour, 0, nil)
	s.SetNextRunOnExpiration(15 * time.Minute)
	if got := s.NextRunDate().Sub(base); got != 15*time.Minute {
		t.Fatalf("expiration run = %v, want 15m", got)
	}
	// The next reset must be a no-op (expiration wins once).
	s.ResetNextRunDate()
	if got := s.NextRunDate().Sub(base); got != 15*time.Minute {
		t.Errorf("reset after expiration changed the run to %v, want 15m unchanged", got)
	}
	// A subsequent reset now applies normally.
	s.ResetNextRunDate()
	if s.NextRunDate().Sub(base) == 15*time.Minute {
		t.Error("second reset should reschedule")
	}
}

// TestBackoffDoubles checks the network-failure backoff doubles up to the cap.
func TestBackoffDoubles(t *testing.T) {
	s := fixed(time.Hour, 5*time.Minute, nil) // errMaxDelay = 5m cap
	s.BackOff(time.Minute)
	if got := s.NextRunDate().Sub(base); got != time.Minute {
		t.Fatalf("backoff 1 = %v, want 1m", got)
	}
	s.BackOff(time.Minute) // doubles to 2m
	if got := s.NextRunDate().Sub(base); got != 2*time.Minute {
		t.Fatalf("backoff 2 = %v, want 2m", got)
	}
	s.BackOff(time.Minute) // doubles to 4m
	if got := s.NextRunDate().Sub(base); got != 4*time.Minute {
		t.Fatalf("backoff 3 = %v, want 4m", got)
	}
	s.BackOff(time.Minute) // would be 8m, capped at 5m (errMaxDelay)
	if got := s.NextRunDate().Sub(base); got != 5*time.Minute {
		t.Errorf("backoff 4 = %v, want 5m (capped)", got)
	}
}

// TestDue checks the due predicate against a moving clock.
func TestDue(t *testing.T) {
	now := base
	s := New(time.Hour, 0, WithClock(func() time.Time { return now }), WithRand(func(int64) int64 { return 0 }))
	if s.Due() {
		t.Error("should not be due immediately (first run is maxDelay out)")
	}
	now = base.Add(time.Hour) // advance to the scheduled time
	if !s.Due() {
		t.Error("should be due once the clock reaches the next run date")
	}
}

// TestSnapshotRestoreRoundtrip checks Snapshot captures the timing and Restore
// adopts a recent stored next-run date.
func TestSnapshotRestoreRoundtrip(t *testing.T) {
	s := fixed(time.Hour, 10*time.Minute, nil)
	// A stored next-run 45m out is within the last maxDelay -> kept on restore,
	// and the startup stagger is cancelled.
	stored := State{NextRun: base.Add(45 * time.Minute), BaseRun: base.Add(90 * time.Minute), Backoff: 30 * time.Second}

	fresh := fixed(time.Hour, 10*time.Minute, nil)
	fresh.Restore(stored)
	if !fresh.NextRunDate().Equal(base.Add(45 * time.Minute)) {
		t.Errorf("restored next run = %v, want base+45m", fresh.NextRunDate())
	}
	snap := fresh.Snapshot()
	if !snap.NextRun.Equal(stored.NextRun) || snap.Backoff != stored.Backoff {
		t.Errorf("snapshot = %+v, want %+v", snap, stored)
	}
	_ = s
}

// TestRestoreIgnoresStaleNextRun checks a stored next-run older than maxDelay is
// not adopted (the freshly computed schedule stands).
func TestRestoreIgnoresStaleNextRun(t *testing.T) {
	s := fixed(time.Hour, 0, nil) // fresh next run = base + 1h
	s.Restore(State{NextRun: base.Add(-2 * time.Hour)})
	if !s.NextRunDate().Equal(base.Add(time.Hour)) {
		t.Errorf("stale stored next run was adopted: %v", s.NextRunDate())
	}
}

// TestMaxDelayDefault checks a non-positive maxDelay falls back to one hour.
func TestMaxDelayDefault(t *testing.T) {
	s := fixed(0, 0, nil)
	if got := s.NextRunDate().Sub(base); got != time.Hour {
		t.Errorf("default maxDelay run = %v, want 1h", got)
	}
}
