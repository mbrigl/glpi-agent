// SPDX-License-Identifier: GPL-2.0-only

// Package scheduler ports the run-timing of a GLPI agent target from
// lib/GLPI/Agent/Target.pm: when a target should next run, with randomised
// staggering, a server-driven expiration, and exponential backoff on network
// failure. The clock and the randomness are injectable so the timing is
// deterministically testable. State persistence across restarts (the Perl
// _saveState) is deferred.
package scheduler

import (
	"math/rand"
	"time"
)

const (
	defaultMaxDelay = time.Hour // Target.pm maxDelay default (3600s)
	sixHours        = 6 * time.Hour
	oneDay          = 24 * time.Hour
)

// Schedule holds the timing state of one target.
type Schedule struct {
	now   func() time.Time
	randn func(n int64) int64 // returns a value in [0,n); n<=0 yields 0

	maxDelay     time.Duration // nominal interval between runs
	errMaxDelay  time.Duration // backoff cap (the delaytime)
	initialDelay time.Duration // one-shot startup stagger (delaytime), consumed once

	nextRunDate  time.Time
	baseRunDate  time.Time
	nextRunDelay time.Duration // current backoff delay (_nextrundelay)
	expiration   bool          // skip the next resetNextRunDate (set via SetNextRunOnExpiration)
}

// Option customises a Schedule (used by tests to inject the clock/rng).
type Option func(*Schedule)

// WithClock injects the time source.
func WithClock(now func() time.Time) Option { return func(s *Schedule) { s.now = now } }

// WithRand injects the randomness source (a function returning a value in [0,n)).
func WithRand(randn func(n int64) int64) Option { return func(s *Schedule) { s.randn = randn } }

// New builds a schedule with the given nominal interval (maxDelay) and the
// delaytime used both as the initial startup stagger and the backoff cap,
// mirroring Target.pm's constructor + _init. A non-positive maxDelay defaults to
// one hour.
func New(maxDelay, delaytime time.Duration, opts ...Option) *Schedule {
	s := &Schedule{
		now:          time.Now,
		randn:        defaultRandn,
		maxDelay:     maxDelay,
		errMaxDelay:  delaytime,
		initialDelay: delaytime,
	}
	for _, o := range opts {
		o(s)
	}
	if s.maxDelay <= 0 {
		s.maxDelay = defaultMaxDelay
	}
	if s.errMaxDelay <= 0 {
		s.errMaxDelay = defaultMaxDelay
	}

	// _init: baseRunDate uses the full initialDelay (or maxDelay); nextRunDate
	// then applies the jittered computeNextRunDate, which consumes initialDelay.
	first := s.initialDelay
	if first <= 0 {
		first = s.maxDelay
	}
	s.baseRunDate = s.now().Add(first)
	s.nextRunDate = s.computeNextRunDate(s.now())
	return s
}

// NextRunDate returns the scheduled next run time.
func (s *Schedule) NextRunDate() time.Time { return s.nextRunDate }

// Due reports whether the target is due to run at the current time.
func (s *Schedule) Due() bool { return !s.now().Before(s.nextRunDate) }

// computeNextRunDate returns timeref advanced by the (jittered) delay, mirroring
// Target.pm::computeNextRunDate. The initial delay, when present, is used once
// and then cleared.
func (s *Schedule) computeNextRunDate(timeref time.Time) time.Time {
	if s.initialDelay > 0 {
		timeref = timeref.Add(s.initialDelay - s.jitter(s.initialDelay/2))
		s.initialDelay = 0
		return timeref
	}
	// Reduce the delay randomly: up to 1h, or maxDelay/6 below 6h, or maxDelay/24
	// above a day.
	reduc := time.Hour
	switch {
	case s.maxDelay < sixHours:
		reduc = s.maxDelay / 6
	case s.maxDelay > oneDay:
		reduc = s.maxDelay / 24
	}
	return timeref.Add(s.maxDelay - s.jitter(reduc))
}

// SetNextRunOnExpiration schedules the next run a server-provided expiration from
// now and marks the schedule so the following ResetNextRunDate is skipped
// (Target.pm::setNextRunOnExpiration).
func (s *Schedule) SetNextRunOnExpiration(expiration time.Duration) {
	if expiration < 0 {
		expiration = 0
	}
	s.nextRunDate = s.now().Add(expiration)
	s.baseRunDate = s.nextRunDate
	s.expiration = true
}

// BackOff schedules the next run after a network failure, doubling the delay on
// each consecutive call up to maxDelay/errMaxDelay
// (Target.pm::setNextRunDateFromNow). delay is the base retry delay (e.g. 60s).
func (s *Schedule) BackOff(delay time.Duration) {
	if delay > 0 {
		if s.nextRunDelay > 0 {
			delay = 2 * s.nextRunDelay
		}
		if delay > s.maxDelay {
			delay = s.maxDelay
		}
		if delay > s.errMaxDelay {
			delay = s.errMaxDelay
		}
		s.nextRunDelay = delay
	}
	s.nextRunDate = s.now().Add(delay)
	s.baseRunDate = s.nextRunDate
	s.initialDelay = 0
}

// ResetNextRunDate schedules the next regular run after a successful run,
// mirroring Target.pm::resetNextRunDate. It is a no-op once after an
// expiration-driven schedule.
func (s *Schedule) ResetNextRunDate() {
	if s.expiration {
		s.expiration = false
		return
	}
	now := s.now()
	timeref := s.baseRunDate
	if timeref.IsZero() {
		timeref = now
	}
	// Reset timeref when it drifted out of the [now-maxDelay, now+maxDelay] range.
	if timeref.Before(now.Add(-s.maxDelay)) || timeref.After(now.Add(s.maxDelay)) {
		timeref = now
	}
	s.nextRunDelay = 0
	s.nextRunDate = s.computeNextRunDate(timeref)
	s.baseRunDate = timeref.Add(s.maxDelay)
}

// jitter returns a random duration in [0,max) at one-second granularity,
// matching the int(rand(...)) seconds reductions in Target.pm.
func (s *Schedule) jitter(max time.Duration) time.Duration {
	secs := int64(max / time.Second)
	return time.Duration(s.randn(secs)) * time.Second
}

func defaultRandn(n int64) int64 {
	if n <= 0 {
		return 0
	}
	return rand.Int63n(n)
}
