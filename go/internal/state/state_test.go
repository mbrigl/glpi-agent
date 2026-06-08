// SPDX-License-Identifier: GPL-2.0-only

package state

import (
	"path/filepath"
	"testing"
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

func TestLoadOrCreateCreatesDir(t *testing.T) {
	dir := filepath.Join(t.TempDir(), "a", "b", "c")
	if _, err := LoadOrCreate(dir); err != nil {
		t.Fatalf("should create nested vardir: %v", err)
	}
}
