// SPDX-License-Identifier: GPL-2.0-only

// Package agent holds the config-agnostic core the CLI and the daemon share: it
// builds a local inventory and runs a GLPI server target (the CONTACT + submit
// dialog), derived from GLPI::Agent::Task::Inventory and GLPI::Agent::getContact.
// Only the modern GLPI protocol is supported (no legacy OCS).
package agent

import (
	"fmt"
	"time"

	"github.com/glpi-project/glpi-agent/go/internal/content"
	"github.com/glpi-project/glpi-agent/go/internal/inventory"
	"github.com/glpi-project/glpi-agent/go/internal/logging"
	"github.com/glpi-project/glpi-agent/go/internal/protocol"
	"github.com/glpi-project/glpi-agent/go/internal/transport"
)

// BuildInventory collects the local inventory document for assetName, tagging it
// when tag is set, mirroring the content assembly of the Inventory task. now
// fixes the timestamp used in the device id.
func BuildInventory(assetName, tag string, now time.Time) *content.Inventory {
	inv := content.New(content.DeviceID(assetName, now))
	if tag != "" {
		inv.Content["ACCOUNTINFO"] = map[string]any{
			"KEYNAME":  "TAG",
			"KEYVALUE": tag,
		}
	}
	// Merge the local category collectors into the document.
	for section, value := range inventory.Collect() {
		if existing, ok := inv.Content[section].(map[string]any); ok {
			if collected, ok := value.(map[string]any); ok {
				for k, v := range collected {
					existing[k] = v
				}
				continue
			}
		}
		inv.Content[section] = value
	}
	return inv
}

// ErrNotGLPIServer is returned when a server does not answer the modern GLPI
// Agent protocol (a valid CONTACT). The legacy OCS fallback is not supported.
var ErrNotGLPIServer = fmt.Errorf("server does not speak the modern GLPI Agent protocol (legacy OCS is not supported)")

// RunServerTarget performs one CONTACT + inventory-submit dialog against a GLPI
// server and returns the server-provided next-run expiration (0 when none).
// Mirrors GLPI::Agent::getContact + Task/Inventory::submit for a server target.
func RunServerTarget(log *logging.Logger, client *transport.GLPIClient, serverURL, deviceID string, inventoryJSON []byte, tag string) (expiration time.Duration, err error) {
	contact := protocol.Contact{
		DeviceID:       deviceID,
		InstalledTasks: []string{"inventory"},
		EnabledTasks:   []string{"inventory"},
		Tag:            tag,
	}
	contactMsg, err := contact.Encode()
	if err != nil {
		return 0, err
	}

	log.Info("sending contact request to " + serverURL)
	answer, err := client.Send(serverURL, contactMsg)
	if err != nil {
		return 0, fmt.Errorf("contact request failed: %w", err)
	}
	if !answer.IsContactValid() {
		return 0, ErrNotGLPIServer
	}
	expiration = time.Duration(answer.Expiration()) * time.Second

	if !answer.TaskEnabled("inventory") {
		log.Info("inventory task disabled by server, nothing to send")
		return expiration, nil
	}

	log.Info("sending inventory to " + serverURL)
	if _, err := client.Send(serverURL, inventoryJSON); err != nil {
		return expiration, fmt.Errorf("inventory submission failed: %w", err)
	}
	log.Info("inventory successfully sent to " + serverURL)
	return expiration, nil
}
