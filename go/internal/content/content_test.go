// SPDX-License-Identifier: GPL-2.0-only

package content

import (
	"encoding/json"
	"testing"
	"time"
)

// TestDeviceID checks the "<assetname>-YYYY-MM-DD-HH-MM-SS" format from the Perl
// agent's deviceid handling.
func TestDeviceID(t *testing.T) {
	got := DeviceID("host.example", time.Date(2022, 1, 10, 11, 13, 28, 0, time.UTC))
	want := "host.example-2022-01-10-11-13-28"
	if got != want {
		t.Fatalf("DeviceID = %q, want %q", got, want)
	}
}

// TestJSONShapeAndLowercasing verifies the protocol top-level keys and that
// content keys are recursively lowercased, mirroring Protocol::Message::_convert.
func TestJSONShapeAndLowercasing(t *testing.T) {
	inv := New("host-2022-01-10-11-13-28")
	data, err := inv.JSON()
	if err != nil {
		t.Fatal(err)
	}

	var msg map[string]any
	if err := json.Unmarshal(data, &msg); err != nil {
		t.Fatalf("output is not valid JSON: %v", err)
	}

	for _, k := range []string{"action", "deviceid", "itemtype", "content"} {
		if _, ok := msg[k]; !ok {
			t.Errorf("missing top-level key %q", k)
		}
	}
	if msg["action"] != "inventory" {
		t.Errorf("action = %v, want inventory", msg["action"])
	}
	if msg["itemtype"] != "Computer" {
		t.Errorf("itemtype = %v, want Computer", msg["itemtype"])
	}

	cont, ok := msg["content"].(map[string]any)
	if !ok {
		t.Fatal("content is not an object")
	}
	if cont["versionclient"] != "GLPI-Agent_v2.17.0" {
		t.Errorf("versionclient = %v, want GLPI-Agent_v2.17.0", cont["versionclient"])
	}
	hw, ok := cont["hardware"].(map[string]any)
	if !ok {
		t.Fatal("content.hardware is not an object (key not lowercased?)")
	}
	if hw["vmsystem"] != "Physical" {
		t.Errorf("hardware.vmsystem = %v, want Physical", hw["vmsystem"])
	}
	// No uppercase keys must survive the conversion.
	if _, bad := cont["HARDWARE"]; bad {
		t.Error("found uppercase key HARDWARE; _convert lowercasing not applied")
	}
}
