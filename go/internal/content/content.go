// SPDX-License-Identifier: GPL-2.0-only

// Package content models the GLPI inventory content and serialises it in the
// GLPI Agent Protocol JSON form.
//
// Derived from the upstream Perl modules:
//   - lib/GLPI/Agent/Inventory.pm            (canonical UPPERCASE section model,
//     HARDWARE.VMSYSTEM="Physical" and VERSIONCLIENT defaults)
//   - lib/GLPI/Agent/Protocol/Inventory.pm   (top-level keys: action, deviceid,
//     content, itemtype, partial; action="inventory", default itemtype "Computer")
//   - lib/GLPI/Agent/Protocol/Message.pm     (_convert: recursively lowercases
//     every key before export; JSON is canonical/indented)
//
// The internal model keeps the canonical UPPERCASE keys used by the legacy XML
// (HARDWARE, VERSIONCLIENT, ...); JSON export lowercases them, exactly as the
// Perl _convert routine does.
package content

import (
	"encoding/json"
	"fmt"
	"time"

	"github.com/glpi-project/glpi-agent/go/internal/version"
)

// Inventory is a GLPI Agent Protocol inventory message. Field names mirror the
// supported_params of GLPI::Agent::Protocol::Inventory.
type Inventory struct {
	// DeviceID is the agent device id ("<assetname>-YYYY-MM-DD-HH-MM-SS").
	DeviceID string
	// ItemType defaults to "Computer" (GLPI::Agent::Inventory).
	ItemType string
	// Partial marks a partial inventory; omitted when false (Perl deletes the
	// key unless set).
	Partial bool
	// Content holds the inventory sections under their canonical UPPERCASE keys
	// (e.g. "HARDWARE", "VERSIONCLIENT", "CPUS").
	Content map[string]any
}

// New returns an inventory pre-populated with the same defaults the Perl
// Inventory.pm sets in its constructor: HARDWARE.VMSYSTEM="Physical" and
// VERSIONCLIENT=$AGENT_STRING.
func New(deviceID string) *Inventory {
	return &Inventory{
		DeviceID: deviceID,
		ItemType: "Computer",
		Content: map[string]any{
			"VERSIONCLIENT": version.AgentString(),
			"HARDWARE": map[string]any{
				"VMSYSTEM": "Physical",
			},
		},
	}
}

// DeviceID builds the agent device id, mirroring the Perl format
// "<assetname>-YYYY-MM-DD-HH-MM-SS" (see GLPI::Agent deviceid handling).
func DeviceID(assetName string, t time.Time) string {
	return fmt.Sprintf("%s-%s", assetName, t.Format("2006-01-02-15-04-05"))
}

// message assembles the top-level protocol object with lowercased keys, matching
// GLPI::Agent::Protocol::Message::_convert applied to an Inventory message.
func (inv *Inventory) message() map[string]any {
	itemtype := inv.ItemType
	if itemtype == "" {
		itemtype = "Computer"
	}
	msg := map[string]any{
		"action":   "inventory",
		"deviceid": inv.DeviceID,
		"itemtype": itemtype,
		"content":  lowercaseKeys(inv.Content),
	}
	if inv.Partial {
		msg["partial"] = true
	}
	return msg
}

// JSON returns the canonical, indented GLPI Agent Protocol JSON for the
// inventory. The encoding/json package sorts map keys, matching Perl's
// canonical encoder; indentation mirrors ->indent->space_after.
func (inv *Inventory) JSON() ([]byte, error) {
	return json.MarshalIndent(inv.message(), "", "   ")
}

// lowercaseKeys recursively lowercases every map key, mirroring the Perl
// _convert routine in GLPI::Agent::Protocol::Message.
func lowercaseKeys(v any) any {
	switch val := v.(type) {
	case map[string]any:
		out := make(map[string]any, len(val))
		for k, inner := range val {
			out[lower(k)] = lowercaseKeys(inner)
		}
		return out
	case []any:
		out := make([]any, len(val))
		for i, inner := range val {
			out[i] = lowercaseKeys(inner)
		}
		return out
	case []map[string]any:
		// Section entry lists (e.g. CPUS, PORTS) are commonly typed this way.
		out := make([]any, len(val))
		for i, inner := range val {
			out[i] = lowercaseKeys(inner)
		}
		return out
	default:
		return v
	}
}

// lower lowercases ASCII, matching Perl lc() on the ASCII section/key names used
// by the inventory model.
func lower(s string) string {
	b := []byte(s)
	for i, c := range b {
		if c >= 'A' && c <= 'Z' {
			b[i] = c + ('a' - 'A')
		}
	}
	return string(b)
}
