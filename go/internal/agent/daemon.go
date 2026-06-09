// SPDX-License-Identifier: GPL-2.0-only

package agent

import (
	"context"
	"time"

	"github.com/glpi-project/glpi-agent/go/internal/logging"
	"github.com/glpi-project/glpi-agent/go/internal/scheduler"
)

// retryDelay is the base network-failure backoff (doubled on each consecutive
// failure up to the target's cap), matching the 60s base of Daemon.pm.
const retryDelay = 60 * time.Second

// ScheduledTarget pairs a target's schedule with the work to run when it is due.
// Run returns the server-provided expiration (0 for a local target or when the
// server gave none) and an error on failure.
type ScheduledTarget struct {
	Name  string
	Sched *scheduler.Schedule
	Run   func() (expiration time.Duration, err error)
}

// RunLoop runs targets on their schedules until sleep reports termination,
// mirroring the run-loop of GLPI::Agent::Daemon: a due target is run, then
// rescheduled by its server expiration, by a backoff on error, or by the normal
// interval on success. sleep blocks until the next iteration and returns false
// to stop (so the caller controls the cadence, signals and shutdown).
func RunLoop(ctx context.Context, log *logging.Logger, targets []*ScheduledTarget, sleep func(context.Context) bool) {
	for {
		for _, t := range targets {
			if t.Sched.Due() {
				runOne(log, t)
			}
		}
		if !sleep(ctx) {
			log.Info("daemon stopping")
			return
		}
	}
}

// runOne runs a single due target and reschedules it.
func runOne(log *logging.Logger, t *ScheduledTarget) {
	log.Info("running target " + t.Name)
	expiration, err := t.Run()
	switch {
	case err != nil:
		log.Error("target " + t.Name + " failed: " + err.Error())
		t.Sched.BackOff(retryDelay)
	case expiration > 0:
		t.Sched.SetNextRunOnExpiration(expiration)
	default:
		t.Sched.ResetNextRunDate()
	}
	log.Info("next run for " + t.Name + " at " + t.Sched.NextRunDate().Format(time.RFC3339))
}
