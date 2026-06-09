// SPDX-License-Identifier: GPL-2.0-only

package agent

import (
	"context"
	"testing"
	"time"

	"github.com/glpi-project/glpi-agent/go/internal/scheduler"
)

var loopBase = time.Date(2026, 6, 9, 12, 0, 0, 0, time.UTC)

// dueSchedule returns a schedule on the given fake clock that is due immediately.
func dueSchedule(clockPtr *time.Time, maxDelay time.Duration) *scheduler.Schedule {
	s := scheduler.New(maxDelay, 0,
		scheduler.WithClock(func() time.Time { return *clockPtr }),
		scheduler.WithRand(func(int64) int64 { return 0 }),
	)
	s.Trigger() // make it due now
	return s
}

// TestRunLoopHonorsExpiration checks a server expiration drives the next run and
// the loop runs the target once per due tick.
func TestRunLoopHonorsExpiration(t *testing.T) {
	now := loopBase
	var runs int
	target := &ScheduledTarget{
		Name:  "srv",
		Sched: dueSchedule(&now, time.Hour),
		Run: func() (time.Duration, error) {
			runs++
			return 30 * time.Minute, nil
		},
	}

	// Advance the clock by 30m each iteration (so the target is due each time),
	// stopping after 3 advances.
	iter := 0
	sleep := func(context.Context) bool {
		if iter >= 3 {
			return false
		}
		iter++
		now = now.Add(30 * time.Minute)
		return true
	}

	NewAgent(testLogger(t), []*ScheduledTarget{target}).Loop(context.Background(), sleep)

	if runs != 4 { // initial run + 3 due ticks
		t.Errorf("runs = %d, want 4", runs)
	}
	// Last run was at base+90m; expiration 30m -> next at base+120m.
	if got := target.Sched.NextRunDate(); !got.Equal(loopBase.Add(120 * time.Minute)) {
		t.Errorf("next run = %v, want base+120m", got)
	}
}

// TestRunLoopBackoffOnError checks a failed run reschedules with the 60s backoff
// rather than immediately.
func TestRunLoopBackoffOnError(t *testing.T) {
	now := loopBase
	var runs int
	target := &ScheduledTarget{
		Name:  "srv",
		Sched: dueSchedule(&now, time.Hour),
		Run: func() (time.Duration, error) {
			runs++
			return 0, context.DeadlineExceeded
		},
	}

	sleep := func(context.Context) bool { return false } // stop after the first pass
	NewAgent(testLogger(t), []*ScheduledTarget{target}).Loop(context.Background(), sleep)

	if runs != 1 {
		t.Fatalf("runs = %d, want 1", runs)
	}
	if got := target.Sched.NextRunDate(); !got.Equal(loopBase.Add(retryDelay)) {
		t.Errorf("next run = %v, want base+60s (backoff)", got)
	}
}

// TestRunLoopSkipsNotDue checks a target that is not due is not run.
func TestRunLoopSkipsNotDue(t *testing.T) {
	now := loopBase
	var runs int
	// A schedule one hour out, never triggered -> not due.
	s := scheduler.New(time.Hour, 0,
		scheduler.WithClock(func() time.Time { return now }),
		scheduler.WithRand(func(int64) int64 { return 0 }),
	)
	target := &ScheduledTarget{Name: "srv", Sched: s, Run: func() (time.Duration, error) { runs++; return 0, nil }}

	sleep := func(context.Context) bool { return false }
	NewAgent(testLogger(t), []*ScheduledTarget{target}).Loop(context.Background(), sleep)

	if runs != 0 {
		t.Errorf("runs = %d, want 0 (not due)", runs)
	}
}

// TestAgentRunNowAndStatus checks RunNow makes targets due, Status reflects the
// idle state, and Targets exposes the next-run snapshot.
func TestAgentRunNowAndStatus(t *testing.T) {
	now := loopBase
	// A schedule far in the future -> not due until RunNow.
	s := scheduler.New(time.Hour, 0,
		scheduler.WithClock(func() time.Time { return now }),
		scheduler.WithRand(func(int64) int64 { return 0 }),
	)
	target := &ScheduledTarget{Name: "srv", Sched: s, Run: func() (time.Duration, error) { return 0, nil }}
	ag := NewAgent(testLogger(t), []*ScheduledTarget{target})

	if ag.Status() != "waiting" {
		t.Errorf("initial status = %q, want waiting", ag.Status())
	}
	infos := ag.Targets()
	if len(infos) != 1 || infos[0].Name != "srv" || !infos[0].NextRun.Equal(loopBase.Add(time.Hour)) {
		t.Errorf("targets snapshot = %v", infos)
	}

	// Before RunNow the target is not due; after RunNow it is, and the wake fires.
	if s.Due() {
		t.Fatal("should not be due before RunNow")
	}
	ag.RunNow()
	if !s.Due() {
		t.Error("RunNow should make the target due")
	}
	select {
	case <-ag.Wake():
	default:
		t.Error("RunNow should signal the wake channel")
	}
}

// TestRunLoopStops checks the loop returns when sleep reports termination.
func TestRunLoopStops(t *testing.T) {
	done := make(chan struct{})
	go func() {
		NewAgent(testLogger(t), nil).Loop(context.Background(), func(context.Context) bool { return false })
		close(done)
	}()
	select {
	case <-done:
	case <-time.After(time.Second):
		t.Fatal("RunLoop did not stop")
	}
}
