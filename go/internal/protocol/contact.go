// SPDX-License-Identifier: GPL-2.0-only

package protocol

import (
	"encoding/json"

	"github.com/glpi-project/glpi-agent/go/internal/version"
)

// Contact is a CONTACT request the agent sends to a GLPI server to announce
// itself and learn which tasks are enabled, mirroring
// GLPI::Agent::Protocol::Contact (action "contact").
type Contact struct {
	DeviceID       string
	InstalledTasks []string
	EnabledTasks   []string
	Tag            string
}

// Encode renders the CONTACT request as canonical JSON, mirroring
// Message::getContent (UTF-8, sorted keys, indented). Go's json.Marshal sorts
// map keys, giving the canonical ordering; the request keys are already
// lowercase as the protocol requires.
func (c Contact) Encode() ([]byte, error) {
	m := map[string]any{
		"action":          "contact",
		"name":            version.Provider + "-Agent",
		"version":         version.Version,
		"deviceid":        c.DeviceID,
		"installed-tasks": nonNil(c.InstalledTasks),
		"enabled-tasks":   nonNil(c.EnabledTasks),
	}
	if c.Tag != "" {
		m["tag"] = c.Tag
	}
	return json.MarshalIndent(m, "", "  ")
}

// nonNil returns an empty (non-nil) slice when s is nil, so the JSON encodes as
// [] rather than null.
func nonNil(s []string) []string {
	if s == nil {
		return []string{}
	}
	return s
}
