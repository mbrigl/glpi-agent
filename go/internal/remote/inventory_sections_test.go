// SPDX-License-Identifier: GPL-2.0-only

package remote

import (
	"testing"

	"github.com/glpi-project/glpi-agent/go/internal/content"
)

func newTestInv() *content.Inventory { return content.New("test") }

// TestCollectRemoteNetworks checks NETWORKS is built from the remote sysfs net
// tree + `ip`.
func TestCollectRemoteNetworks(t *testing.T) {
	sys := &fakeSystem{
		commands: map[string]string{
			"ls -d /sys/class/net/* 2>/dev/null":                "/sys/class/net/eth0\n",
			"test -e /sys/devices/virtual/net/eth0 && echo yes": "",
			"ip -o -4 addr show dev eth0":                       "2: eth0    inet 192.168.1.10/24 brd 192.168.1.255 scope global eth0\\       valid_lft forever\n",
		},
		files: map[string]string{
			"/sys/class/net/eth0/address":       "aa:bb:cc:dd:ee:ff",
			"/sys/class/net/eth0/flags":         "0x1003", // IFF_UP set
			"/sys/class/net/eth0/speed":         "1000",
			"/sys/class/net/eth0/device/uevent": "DRIVER=e1000e\nPCI_ID=8086:10D3\n",
		},
	}
	inv := newTestInv()
	collectRemoteNetworks(sys, inv)

	nets, _ := inv.Content["NETWORKS"].([]map[string]any)
	if len(nets) != 1 {
		t.Fatalf("got %d networks, want 1", len(nets))
	}
	n := nets[0]
	if n["DESCRIPTION"] != "eth0" || n["MACADDR"] != "aa:bb:cc:dd:ee:ff" ||
		n["IPADDRESS"] != "192.168.1.10" || n["IPMASK"] != "255.255.255.0" ||
		n["STATUS"] != "Up" || n["DRIVER"] != "e1000e" || n["SPEED"] != 1000 {
		t.Errorf("network = %v", n)
	}
}

// TestCollectRemoteFirewall checks the ufw/firewalld status mapping.
func TestCollectRemoteFirewall(t *testing.T) {
	sys := &fakeSystem{
		runnable: map[string]bool{"ufw": true, "systemctl": true},
		commands: map[string]string{
			"ufw status":                    "Status: active\n",
			"systemctl is-active firewalld": "inactive\n",
		},
	}
	inv := newTestInv()
	collectRemoteFirewall(sys, inv)
	fw, _ := inv.Content["FIREWALL"].([]map[string]any)
	if len(fw) != 1 || fw[0]["DESCRIPTION"] != "ufw" || fw[0]["STATUS"] != "on" {
		t.Errorf("firewall = %v", fw)
	}
}

// TestCidrToIPMask covers the CIDR -> dotted-mask conversion.
func TestCidrToIPMask(t *testing.T) {
	cases := map[string][2]string{
		"10.0.0.5/24":    {"10.0.0.5", "255.255.255.0"},
		"172.16.0.1/16":  {"172.16.0.1", "255.255.0.0"},
		"192.168.1.1/32": {"192.168.1.1", "255.255.255.255"},
	}
	for in, want := range cases {
		ip, mask := cidrToIPMask(in)
		if ip != want[0] || mask != want[1] {
			t.Errorf("cidrToIPMask(%q) = %q/%q, want %q/%q", in, ip, mask, want[0], want[1])
		}
	}
}
