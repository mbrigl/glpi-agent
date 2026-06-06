// SPDX-License-Identifier: GPL-2.0-only

package inventory

import "testing"

func TestBuildNetworks(t *testing.T) {
	ifaces := []NetIface{
		{
			Name: "eth0", MAC: "ba:e1:19:d7:33:13", Up: true, Virtual: false,
			Speed: 1000, Driver: "e1000",
			Addrs: []NetAddr{{IP: "172.19.0.2", Mask: "255.255.0.0"}},
		},
		{Name: "down0", Up: false},
	}
	nets := BuildNetworks(ifaces)
	if len(nets) != 2 {
		t.Fatalf("got %d entries, want 2", len(nets))
	}

	eth := nets[0]
	if eth["DESCRIPTION"] != "eth0" || eth["MACADDR"] != "ba:e1:19:d7:33:13" {
		t.Errorf("eth0 base wrong: %v", eth)
	}
	if eth["STATUS"] != "Up" || eth["VIRTUALDEV"] != 0 || eth["SPEED"] != 1000 || eth["DRIVER"] != "e1000" {
		t.Errorf("eth0 attributes wrong: %v", eth)
	}
	if eth["IPADDRESS"] != "172.19.0.2" || eth["IPMASK"] != "255.255.0.0" || eth["IPSUBNET"] != "172.19.0.0" {
		t.Errorf("eth0 ip fields wrong: %v", eth)
	}

	if nets[1]["STATUS"] != "Down" {
		t.Errorf("down0 STATUS = %v, want Down", nets[1]["STATUS"])
	}
}

func TestSubnetAddress(t *testing.T) {
	if got := subnetAddress("172.19.0.2", "255.255.0.0"); got != "172.19.0.0" {
		t.Errorf("subnet = %q, want 172.19.0.0", got)
	}
	if got := subnetAddress("10.1.2.3", "255.255.255.0"); got != "10.1.2.0" {
		t.Errorf("subnet = %q, want 10.1.2.0", got)
	}
}
