// SPDX-License-Identifier: GPL-2.0-only

// Package vsphere collects inventory from VMware ESX/vCenter.
//
// The set of inventory sections and their fields are derived from the upstream
// Perl modules lib/GLPI/Agent/Task/ESX.pm (createInventory) and
// lib/GLPI/Agent/SOAP/VMware/Host.pm (the get* accessors). The transport,
// however, uses the official govmomi SDK instead of the hand-rolled SOAP client
// — govmomi exposes the same vSphere managed objects, so the Perl accessors map
// field-for-field onto the typed govmomi structs (e.g.
// hardware.biosInfo.releaseDate -> Hardware.BiosInfo.ReleaseDate).
package vsphere

import (
	"context"
	"fmt"
	"net/url"

	"github.com/vmware/govmomi"
	"github.com/vmware/govmomi/property"
	"github.com/vmware/govmomi/view"
	"github.com/vmware/govmomi/vim25/mo"

	"github.com/glpi-project/glpi-agent/go/internal/content"
)

// Client wraps a govmomi connection to an ESX host or vCenter.
type Client struct {
	govmomi *govmomi.Client
}

// Connect establishes a session, mirroring the connect step of
// GLPI::Agent::Task::ESX (host/user/password). The endpoint is the vSphere SDK
// URL https://<host>/sdk. insecure disables certificate verification, matching
// the Perl SOAP client which sets SSL_verify_mode => 0.
func Connect(ctx context.Context, host, user, password string, insecure bool) (*Client, error) {
	u := &url.URL{Scheme: "https", Host: host, Path: "/sdk"}
	u.User = url.UserPassword(user, password)
	return connectURL(ctx, u, insecure)
}

// connectURL connects to an already-formed vSphere SDK URL. It is the seam used
// by the simulator-backed tests.
func connectURL(ctx context.Context, u *url.URL, insecure bool) (*Client, error) {
	c, err := govmomi.NewClient(ctx, u, insecure)
	if err != nil {
		return nil, fmt.Errorf("ESX connection failed: %w", err)
	}
	return &Client{govmomi: c}, nil
}

// Close logs out of the session.
func (c *Client) Close(ctx context.Context) error {
	return c.govmomi.Logout(ctx)
}

// hostProps are the managed-object properties Host.pm reads from each host.
var hostProps = []string{"name", "summary", "hardware", "config", "vm"}

// vmProps are the properties getVirtualMachines reads from each VM.
var vmProps = []string{"name", "summary", "config", "guest"}

// CollectHosts builds one inventory document per ESX host, mirroring the loop in
// ESX::serverInventory/createInventory. itemtype defaults to "Computer" (the
// esx-itemtype option in bin/glpi-esx).
func (c *Client) CollectHosts(ctx context.Context, itemtype, tag string) ([]*content.Inventory, error) {
	m := view.NewManager(c.govmomi.Client)
	v, err := m.CreateContainerView(ctx, c.govmomi.ServiceContent.RootFolder, []string{"HostSystem"}, true)
	if err != nil {
		return nil, err
	}
	defer func() { _ = v.Destroy(ctx) }()

	var hosts []mo.HostSystem
	if err := v.Retrieve(ctx, []string{"HostSystem"}, hostProps, &hosts); err != nil {
		return nil, err
	}

	pc := property.DefaultCollector(c.govmomi.Client)

	var inventories []*content.Inventory
	for i := range hosts {
		host := &hosts[i]

		var vms []mo.VirtualMachine
		if len(host.Vm) > 0 {
			if err := pc.Retrieve(ctx, host.Vm, vmProps, &vms); err != nil {
				return nil, err
			}
		}

		inventories = append(inventories, buildInventory(host, vms, itemtype, tag))
	}
	return inventories, nil
}
