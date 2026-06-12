// SPDX-License-Identifier: GPL-2.0-only

package discovery

import "testing"

// connectionOf returns PORT.CONNECTIONS.CONNECTION for the given ifnumber.
func connectionOf(d Device, ifnum string) map[string]any {
	port := portsByNumber(d)[ifnum]
	if port == nil {
		return nil
	}
	conns, _ := port["CONNECTIONS"].(map[string]any)
	if conns == nil {
		return nil
	}
	conn, _ := conns["CONNECTION"].(map[string]any)
	return conn
}

// TestLLDPNeighbour checks an LLDP remote entry becomes PORT.CONNECTIONS.
func TestLLDPNeighbour(t *testing.T) {
	getter := netGetter(map[string]string{"1": "Gi0/1", "2": "Gi0/2"}, map[string]map[string]string{
		oidLldpRemChassisId:     {"0.1.1": "aa:bb:cc:dd:ee:ff"},
		oidLldpRemChassisIdSub:  {"0.1.1": "4"},
		oidLldpRemSysName:       {"0.1.1": "neighbor-sw"},
		oidLldpRemSysDesc:       {"0.1.1": "Neighbor OS v1"},
		oidLldpRemPortId:        {"0.1.1": "Gi0/24"},
		oidLldpRemPortIdSubt:    {"0.1.1": "5"}, // interface name -> IFDESCR
		oidDot1dBasePortIfIndex: {"1": "1"},
	})
	d, err := GetInventory("192.0.2.10", getter)
	if err != nil {
		t.Fatal(err)
	}
	conn := connectionOf(d, "1")
	if conn == nil {
		t.Fatal("port 1 has no LLDP connection")
	}
	if conn["SYSMAC"] != "aa:bb:cc:dd:ee:ff" || conn["SYSNAME"] != "neighbor-sw" ||
		conn["SYSDESCR"] != "Neighbor OS v1" || conn["IFDESCR"] != "Gi0/24" {
		t.Errorf("lldp connection = %v", conn)
	}
}

// TestCDPNeighbour checks a Cisco CDP cache entry becomes PORT.CONNECTIONS.
func TestCDPNeighbour(t *testing.T) {
	getter := netGetter(map[string]string{"1": "Gi0/1"}, map[string]map[string]string{
		oidCdpCacheAddress:    {"1.1": "0a:00:00:01"}, // 10.0.0.1
		oidCdpCacheVersion:    {"1.1": "Cisco IOS Software"},
		oidCdpCachePlatform:   {"1.1": "cisco WS-C2960"},
		oidCdpCacheDevicePort: {"1.1": "GigabitEthernet0/1"},
		oidCdpCacheSysName:    {"1.1": "remote-switch"},
	})
	d, err := GetInventory("192.0.2.11", getter)
	if err != nil {
		t.Fatal(err)
	}
	conn := connectionOf(d, "1")
	if conn == nil {
		t.Fatal("port 1 has no CDP connection")
	}
	if conn["IP"] != "10.0.0.1" || conn["MODEL"] != "cisco WS-C2960" ||
		conn["SYSDESCR"] != "Cisco IOS Software" || conn["SYSNAME"] != "remote-switch" ||
		conn["IFDESCR"] != "GigabitEthernet0/1" {
		t.Errorf("cdp connection = %v", conn)
	}
}

// TestColonHexToIP / TestDecimalBytesToMAC cover the address helpers.
func TestColonHexToIP(t *testing.T) {
	if got := colonHexToIP("0a:00:00:01"); got != "10.0.0.1" {
		t.Errorf("colonHexToIP = %q, want 10.0.0.1", got)
	}
	if got := colonHexToIP("c0:a8:01:fe"); got != "192.168.1.254" {
		t.Errorf("colonHexToIP = %q, want 192.168.1.254", got)
	}
}

func TestDecimalBytesToMAC(t *testing.T) {
	if got := decimalBytesToMAC([]string{"170", "187", "204", "0", "1", "2"}); got != "aa:bb:cc:00:01:02" {
		t.Errorf("decimalBytesToMAC = %q", got)
	}
}
