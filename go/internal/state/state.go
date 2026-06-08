// SPDX-License-Identifier: GPL-2.0-only

// Package state persists the small amount of agent state that must survive
// across runs — most importantly the agent id (a stable UUID the GLPI server
// uses to identify this agent). It mirrors the `$PROVIDER-Agent` storage of
// lib/GLPI/Agent.pm (a Storable dump there; a JSON file here), kept under the
// agent's vardir.
package state

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"

	"github.com/google/uuid"
)

// stateFile is the on-disk name of the agent state, under the vardir.
const stateFile = "agent-state.json"

// Agent is the persisted agent state.
type Agent struct {
	// AgentID is the stable UUID sent as the GLPI-Agent-ID header. Created once
	// and reused (GLPI::Agent create_uuid + storage).
	AgentID string `json:"agentid"`

	dir string // the vardir this state was loaded from
}

// LoadOrCreate reads the agent state from dir, creating the directory and a new
// agent id (and persisting it) on first use. Mirrors the agentid handling of
// GLPI::Agent::_loadState/save.
func LoadOrCreate(dir string) (*Agent, error) {
	if err := os.MkdirAll(dir, 0o755); err != nil {
		return nil, fmt.Errorf("creating vardir %s: %w", dir, err)
	}

	a := &Agent{dir: dir}
	raw, err := os.ReadFile(filepath.Join(dir, stateFile))
	switch {
	case err == nil:
		if err := json.Unmarshal(raw, a); err != nil {
			return nil, fmt.Errorf("reading agent state: %w", err)
		}
		a.dir = dir
	case !os.IsNotExist(err):
		return nil, fmt.Errorf("reading agent state: %w", err)
	}

	if a.AgentID == "" {
		a.AgentID = uuid.NewString()
		if err := a.save(); err != nil {
			return nil, err
		}
	}
	return a, nil
}

// save writes the state back to disk.
func (a *Agent) save() error {
	data, err := json.MarshalIndent(a, "", "  ")
	if err != nil {
		return err
	}
	return os.WriteFile(filepath.Join(a.dir, stateFile), data, 0o644)
}
