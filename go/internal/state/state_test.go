// SPDX-License-Identifier: GPL-2.0-only

package state

import (
	"path/filepath"
	"testing"
	"time"
)

func TestLoadOrCreatePersistsAgentID(t *testing.T) {
	dir := filepath.Join(t.TempDir(), "var")

	first, err := LoadOrCreate(dir)
	if err != nil {
		t.Fatal(err)
	}
	if first.AgentID == "" {
		t.Fatal("expected a generated agent id")
	}

	// A second load must reuse the same id.
	second, err := LoadOrCreate(dir)
	if err != nil {
		t.Fatal(err)
	}
	if second.AgentID != first.AgentID {
		t.Errorf("agent id not stable: %q vs %q", first.AgentID, second.AgentID)
	}
}

func TestSaveSchedulePersists(t *testing.T) {
	dir := filepath.Join(t.TempDir(), "var")
	a, err := LoadOrCreate(dir)
	if err != nil {
		t.Fatal(err)
	}
	next := time.Date(2026, 6, 9, 13, 0, 0, 0, time.UTC)
	base := next.Add(time.Hour)
	if err := a.SaveSchedule(next, base, 90*time.Second); err != nil {
		t.Fatal(err)
	}

	b, err := LoadOrCreate(dir)
	if err != nil {
		t.Fatal(err)
	}
	if !b.NextRun.Equal(next) || !b.BaseRun.Equal(base) || b.Backoff != 90*time.Second {
		t.Errorf("schedule not persisted: %+v", b)
	}
	if b.AgentID != a.AgentID {
		t.Errorf("agent id changed: %q vs %q", b.AgentID, a.AgentID)
	}
}

func TestLoadOrCreateCreatesDir(t *testing.T) {
	dir := filepath.Join(t.TempDir(), "a", "b", "c")
	if _, err := LoadOrCreate(dir); err != nil {
		t.Fatalf("should create nested vardir: %v", err)
	}
}
