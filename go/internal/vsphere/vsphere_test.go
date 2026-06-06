// SPDX-License-Identifier: GPL-2.0-only

package vsphere

import (
	"context"
	"encoding/json"
	"testing"

	"github.com/vmware/govmomi/simulator"
)

// TestCollectHostsAgainstSimulator runs the collector against the govmomi
// simulator (vcsim), which serves the same vSphere managed objects a real
// vCenter does. It is a genuine end-to-end check of the Perl-derived field
// mapping without external infrastructure.
func TestCollectHostsAgainstSimulator(t *testing.T) {
	model := simulator.VPX()
	if err := model.Create(); err != nil {
		t.Fatal(err)
	}
	defer model.Remove()

	server := model.Service.NewServer()
	defer server.Close()

	ctx := context.Background()
	client, err := connectURL(ctx, server.URL, true)
	if err != nil {
		t.Fatal(err)
	}
	defer client.Close(ctx)

	inventories, err := client.CollectHosts(ctx, "Computer", "lab")
	if err != nil {
		t.Fatal(err)
	}
	if len(inventories) == 0 {
		t.Fatal("expected at least one ESX host inventory from the simulator")
	}

	inv := inventories[0]
	if inv.ItemType != "Computer" {
		t.Errorf("itemtype = %q, want Computer", inv.ItemType)
	}
	if inv.DeviceID == "" {
		t.Error("deviceid is empty")
	}

	// The document must serialise to valid GLPI Agent Protocol JSON.
	data, err := inv.JSON()
	if err != nil {
		t.Fatal(err)
	}
	var msg map[string]any
	if err := json.Unmarshal(data, &msg); err != nil {
		t.Fatalf("inventory is not valid JSON: %v", err)
	}
	cont, ok := msg["content"].(map[string]any)
	if !ok {
		t.Fatal("content missing")
	}
	if cont["versionclient"] != "GLPI-Agent_v2.17.0" {
		t.Errorf("versionclient = %v", cont["versionclient"])
	}
	// HARDWARE is always populated by createInventory.
	if _, ok := cont["hardware"].(map[string]any); !ok {
		t.Error("hardware section missing")
	}
	// The tag must surface as ACCOUNTINFO (lowercased on export).
	if ai, ok := cont["accountinfo"].(map[string]any); !ok || ai["keyvalue"] != "lab" {
		t.Errorf("accountinfo = %v, want tag lab", cont["accountinfo"])
	}

	// The simulated host owns VMs; ensure the VM mapping produced entries.
	foundVM := false
	for _, inv := range inventories {
		if vms, ok := inv.Content["VIRTUALMACHINES"].([]map[string]any); ok && len(vms) > 0 {
			foundVM = true
			vm := vms[0]
			if vm["VMTYPE"] != "VMware" {
				t.Errorf("VM VMTYPE = %v, want VMware", vm["VMTYPE"])
			}
			if vm["NAME"] == "" || vm["NAME"] == nil {
				t.Error("VM NAME is empty")
			}
		}
	}
	if !foundVM {
		t.Error("expected at least one VIRTUALMACHINES entry across hosts")
	}
}
