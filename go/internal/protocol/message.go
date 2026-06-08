// SPDX-License-Identifier: GPL-2.0-only

// Package protocol implements the GLPI Agent JSON protocol messages exchanged
// with a GLPI server (CONTACT request and server answers), derived from the
// upstream Perl modules lib/GLPI/Agent/Protocol/{Message,Contact}.pm. Only the
// modern GLPI protocol is implemented; the legacy OCS XML PROLOG/SEND path is
// intentionally not ported.
package protocol

import (
	"encoding/json"
	"regexp"
	"strconv"
)

// Message is a decoded GLPI protocol JSON message — typically a server answer.
// Mirrors GLPI::Agent::Protocol::Message.
type Message struct {
	data map[string]any
}

// Parse decodes a JSON message (Message::set).
func Parse(b []byte) (*Message, error) {
	var m map[string]any
	if err := json.Unmarshal(b, &m); err != nil {
		return nil, err
	}
	return &Message{data: m}, nil
}

// Get returns the raw value of a top-level key, or nil.
func (m *Message) Get(key string) any {
	if m == nil || m.data == nil {
		return nil
	}
	return m.data[key]
}

// GetString returns a top-level string value, or "".
func (m *Message) GetString(key string) string {
	s, _ := m.Get(key).(string)
	return s
}

// Status returns the server answer status ("" when absent), e.g. "ok", "pending"
// or "error" (Message::status).
func (m *Message) Status() string { return m.GetString("status") }

// Message returns the server's human-readable message field, if any (used to
// surface error details).
func (m *Message) Message() string { return m.GetString("message") }

// Action returns the message action, defaulting to "inventory" (Message::action).
func (m *Message) Action() string {
	if a := m.GetString("action"); a != "" {
		return a
	}
	return "inventory"
}

var expirationRE = regexp.MustCompile(`^(\d+)([dshm]?)$`)

// Expiration returns the answer's expiration in seconds, or 0 when absent or
// malformed, mirroring Message::expiration: a bare number or "h" means hours,
// "s" seconds, "m" minutes, "d" days.
func (m *Message) Expiration() int {
	raw, _ := m.Get("expiration").(string)
	mt := expirationRE.FindStringSubmatch(raw)
	if mt == nil {
		return 0
	}
	n, _ := strconv.Atoi(mt[1])
	switch mt[2] {
	case "s":
		return n
	case "m":
		return n * 60
	case "d":
		return n * 86400
	default: // "" or "h"
		return n * 3600
	}
}

// IsValid reports whether the message has content and a status
// (Message::is_valid_message).
func (m *Message) IsValid() bool {
	return m != nil && m.data != nil && m.Status() != ""
}

// IsContactValid reports whether a server CONTACT answer is valid: a valid
// message with an expiration greater than zero (Contact::is_valid_message).
func (m *Message) IsContactValid() bool {
	return m.IsValid() && m.Expiration() > 0
}

// Tasks returns the per-task information the server announced in a CONTACT
// answer (the "tasks" object), keyed by task name, or nil.
func (m *Message) Tasks() map[string]any {
	t, _ := m.Get("tasks").(map[string]any)
	return t
}

// TaskEnabled reports whether the named task is present in the server's task
// support map. A CONTACT answer without a tasks map leaves every planned task
// enabled (the upstream default), so an absent map yields true.
func (m *Message) TaskEnabled(name string) bool {
	tasks := m.Tasks()
	if tasks == nil {
		return true
	}
	_, ok := tasks[name]
	return ok
}
