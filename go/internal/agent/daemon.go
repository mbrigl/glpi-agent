// SPDX-License-Identifier: GPL-2.0-only

package agent

import (
	"context"
	"sync"
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

// TargetInfo is a read-only snapshot of a target for the control server.
type TargetInfo struct {
	Name    string
	NextRun time.Time
}

// Agent owns the scheduled targets and the run-loop, mirroring the parts of
// GLPI::Agent the daemon and the HTTP control server share: the targets, the
// run state (getStatus) and the "run now" trigger. It is safe for the control
// server to query Status/Targets and call RunNow concurrently with the loop.
type Agent struct {
	log     *logging.Logger
	mu      sync.Mutex
	targets []*ScheduledTarget
	status  string
	wake    chan struct{}
}

// NewAgent builds an agent over the given targets.
func NewAgent(log *logging.Logger, targets []*ScheduledTarget) *Agent {
	return &Agent{
		log:     log,
		targets: targets,
		status:  "waiting",
		wake:    make(chan struct{}, 1),
	}
}

// Status returns the current run state ("waiting" or "running: <target>"),
// mirroring GLPI::Agent::getStatus.
func (a *Agent) Status() string {
	a.mu.Lock()
	defer a.mu.Unlock()
	return a.status
}

func (a *Agent) setStatus(s string) {
	a.mu.Lock()
	a.status = s
	a.mu.Unlock()
}

// Targets returns a snapshot of the targets and their next-run times, for the
// control server's status page.
func (a *Agent) Targets() []TargetInfo {
	a.mu.Lock()
	defer a.mu.Unlock()
	out := make([]TargetInfo, len(a.targets))
	for i, t := range a.targets {
		out[i] = TargetInfo{Name: t.Name, NextRun: t.Sched.NextRunDate()}
	}
	return out
}

// RunNow forces every target due immediately and wakes the loop, mirroring the
// "run now" trigger of the /now endpoint and SIGUSR1.
func (a *Agent) RunNow() {
	a.mu.Lock()
	for _, t := range a.targets {
		t.Sched.Trigger()
	}
	a.mu.Unlock()
	select {
	case a.wake <- struct{}{}:
	default: // a wake is already pending
	}
}

// Wake is the channel the loop's sleeper selects on so RunNow can interrupt it.
func (a *Agent) Wake() <-chan struct{} { return a.wake }

// Loop runs targets on their schedules until sleep reports termination,
// mirroring the run-loop of GLPI::Agent::Daemon: a due target is run, then
// rescheduled by its server expiration, by a backoff on error, or by the normal
// interval on success. sleep blocks until the next iteration and returns false
// to stop (so the caller controls the cadence, signals and shutdown). Schedule
// access is guarded by the agent lock; the network work in Run happens outside
// it so Status/RunNow stay responsive during a run.
func (a *Agent) Loop(ctx context.Context, sleep func(context.Context) bool) {
	for {
		for _, t := range a.targets {
			a.mu.Lock()
			due := t.Sched.Due()
			a.mu.Unlock()
			if !due {
				continue
			}
			a.setStatus("running: " + t.Name)
			expiration, err := t.Run()
			a.mu.Lock()
			a.reschedule(t, expiration, err)
			a.mu.Unlock()
		}
		a.setStatus("waiting")
		if !sleep(ctx) {
			a.log.Info("daemon stopping")
			return
		}
	}
}

// reschedule applies the run outcome to a target's schedule (caller holds a.mu).
func (a *Agent) reschedule(t *ScheduledTarget, expiration time.Duration, err error) {
	switch {
	case err != nil:
		a.log.Error("target " + t.Name + " failed: " + err.Error())
		t.Sched.BackOff(retryDelay)
	case expiration > 0:
		t.Sched.SetNextRunOnExpiration(expiration)
	default:
		t.Sched.ResetNextRunDate()
	}
	a.log.Info("next run for " + t.Name + " at " + t.Sched.NextRunDate().Format(time.RFC3339))
}
