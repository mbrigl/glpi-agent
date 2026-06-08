// SPDX-License-Identifier: GPL-2.0-only

package protocol

import (
	"encoding/json"
	"testing"
)

func TestContactEncode(t *testing.T) {
	c := Contact{
		DeviceID:       "host-2026-06-08-12-00-00",
		InstalledTasks: []string{"inventory", "netdiscovery"},
		EnabledTasks:   []string{"inventory"},
		Tag:            "lab",
	}
	b, err := c.Encode()
	if err != nil {
		t.Fatal(err)
	}
	var m map[string]any
	if err := json.Unmarshal(b, &m); err != nil {
		t.Fatal(err)
	}
	if m["action"] != "contact" || m["name"] != "GLPI-Agent" {
		t.Errorf("envelope = %v", m)
	}
	if m["deviceid"] != "host-2026-06-08-12-00-00" || m["tag"] != "lab" {
		t.Errorf("fields = %v", m)
	}
	if et, _ := m["enabled-tasks"].([]any); len(et) != 1 || et[0] != "inventory" {
		t.Errorf("enabled-tasks = %v", m["enabled-tasks"])
	}
}

// TestContactEncodeOmitsTag checks the tag key is absent when empty, and the
// task lists encode as [] not null.
func TestContactEncodeOmitsTag(t *testing.T) {
	b, _ := Contact{DeviceID: "d"}.Encode()
	var m map[string]any
	_ = json.Unmarshal(b, &m)
	if _, ok := m["tag"]; ok {
		t.Error("empty tag should be omitted")
	}
	if it, ok := m["installed-tasks"].([]any); !ok || it == nil {
		t.Errorf("installed-tasks should be [], got %T %v", m["installed-tasks"], m["installed-tasks"])
	}
}

func TestExpiration(t *testing.T) {
	cases := map[string]int{
		`{"expiration":"24"}`:  24 * 3600, // bare = hours
		`{"expiration":"24h"}`: 24 * 3600,
		`{"expiration":"30s"}`: 30,
		`{"expiration":"5m"}`:  300,
		`{"expiration":"1d"}`:  86400,
		`{"expiration":"bad"}`: 0,
		`{}`:                   0,
	}
	for body, want := range cases {
		m, err := Parse([]byte(body))
		if err != nil {
			t.Fatalf("%s: %v", body, err)
		}
		if got := m.Expiration(); got != want {
			t.Errorf("%s: expiration = %d, want %d", body, got, want)
		}
	}
}

func TestContactAnswerValidity(t *testing.T) {
	// Valid: status + expiration > 0.
	ok, _ := Parse([]byte(`{"status":"ok","expiration":"24h"}`))
	if !ok.IsContactValid() {
		t.Error("expected a valid contact answer")
	}
	// Missing expiration -> invalid contact (but a valid message).
	noExp, _ := Parse([]byte(`{"status":"ok"}`))
	if noExp.IsContactValid() {
		t.Error("contact without expiration must be invalid")
	}
	if !noExp.IsValid() {
		t.Error("a message with a status is still valid")
	}
	// No status -> not a valid message.
	noStatus, _ := Parse([]byte(`{"expiration":"24h"}`))
	if noStatus.IsValid() {
		t.Error("a message without status is invalid")
	}
}

func TestTaskEnabled(t *testing.T) {
	// No tasks map -> everything stays enabled.
	none, _ := Parse([]byte(`{"status":"ok","expiration":"24h"}`))
	if !none.TaskEnabled("inventory") {
		t.Error("absent tasks map should keep inventory enabled")
	}
	// Explicit tasks map -> only listed tasks enabled.
	some, _ := Parse([]byte(`{"status":"ok","expiration":"24h","tasks":{"inventory":{"server":"glpi"}}}`))
	if !some.TaskEnabled("inventory") {
		t.Error("inventory should be enabled")
	}
	if some.TaskEnabled("deploy") {
		t.Error("deploy should not be enabled")
	}
}
