// SPDX-License-Identifier: GPL-2.0-only

package remote

import (
	"time"

	"github.com/glpi-project/glpi-agent/go/internal/content"
)

// CollectInventory builds an inventory for a remote host over SSH, mirroring the
// remote path of GLPI::Agent::Task::RemoteInventory: the same local inventory
// commands are run through the remote executor. Phase 7 collects the
// host-identifying basics (hostname, OS kernel, architecture) that exercise the
// SSH exec path; the full per-category collectors are shared with Phase 6.
func (c *SSHClient) CollectInventory(itemtype, tag, fallbackHost string) (*content.Inventory, error) {
	hostname := c.Hostname(fallbackHost)

	inv := content.New(content.DeviceID(hostname, time.Now()))
	if itemtype != "" {
		inv.ItemType = itemtype
	}
	if tag != "" {
		inv.Content["ACCOUNTINFO"] = map[string]any{"KEYNAME": "TAG", "KEYVALUE": tag}
	}

	hardware := inv.Content["HARDWARE"].(map[string]any)
	hardware["NAME"] = hostname

	os := map[string]any{}
	if osname, err := c.OSName(); err == nil && osname != "" {
		os["KERNEL_NAME"] = osname
	}
	if version, err := c.firstLine("uname -r"); err == nil && version != "" {
		os["KERNEL_VERSION"] = version
	}
	if arch, err := c.firstLine("uname -m"); err == nil && arch != "" {
		hardware["ARCH"] = arch
	}
	if fqdn := c.FQDN(); fqdn != "" {
		os["FQDN"] = fqdn
	}
	if len(os) > 0 {
		inv.Content["OPERATINGSYSTEM"] = os
	}

	return inv, nil
}
