// SPDX-License-Identifier: GPL-2.0-only

package discovery

import (
	"testing"
)

// fakeGetter is an in-memory SNMPGetter: hosts present in the map answer with
// their OID values; absent hosts return an error (unreachable). walks maps a
// base OID to its index->value table.
type fakeGetter struct {
	values map[string]string
	walks  map[string]map[string]string
}

func (f *fakeGetter) Get(oids []string) (map[string]string, error) {
	out := map[string]string{}
	for _, oid := range oids {
		if v, ok := f.values[oid]; ok {
			out[oid] = v
		}
	}
	return out, nil
}

func (f *fakeGetter) Walk(base string) (map[string]string, error) {
	if t, ok := f.walks[base]; ok {
		return t, nil
	}
	return map[string]string{}, nil
}
func (f *fakeGetter) Close() error { return nil }

// TestBuildDevice checks the generic DEVICE field mapping from SNMP/Device.pm.
func TestBuildDevice(t *testing.T) {
	values := map[string]string{
		oidSysDescr:    "Linux router 5.10",
		oidSysName:     "gw1",
		oidSysLocation: "rack 3",
		oidSysContact:  "noc@example",
		oidSysUpTime:   "12345",
		oidSysObjectID: "1.3.6.1.4.1.8072.3.2.10",
	}
	d := BuildDevice("192.0.2.10", values)
	if d == nil {
		t.Fatal("device is nil despite a sysDescr")
	}
	if d["IP"] != "192.0.2.10" || d["DESCRIPTION"] != "Linux router 5.10" {
		t.Errorf("IP/DESCRIPTION wrong: %v", d)
	}
	if d["SNMPHOSTNAME"] != "gw1" || d["LOCATION"] != "rack 3" || d["CONTACT"] != "noc@example" {
		t.Errorf("base variables wrong: %v", d)
	}
}

// TestBuildDeviceNoSNMP verifies a host with no sysDescr yields no device.
func TestBuildDeviceNoSNMP(t *testing.T) {
	if d := BuildDevice("192.0.2.99", map[string]string{}); d != nil {
		t.Errorf("expected nil device without sysDescr, got %v", d)
	}
}

// TestScan exercises the range scan end to end through the SNMP seam: one host
// answers, the rest are unreachable.
func TestScan(t *testing.T) {
	answering := "192.0.2.2"
	dial := func(host string) (SNMPGetter, error) {
		if host == answering {
			return &fakeGetter{values: map[string]string{
				oidSysDescr: "Switch A",
				oidSysName:  "sw-a",
			}}, nil
		}
		return nil, errUnreachable
	}

	// Both the sequential (threads=1) and concurrent (threads>1) paths must find
	// exactly the one answering host.
	for _, threads := range []int{1, 4} {
		devices, err := Scan([]string{"192.0.2.0/29"}, dial, threads, nil)
		if err != nil {
			t.Fatal(err)
		}
		if len(devices) != 1 {
			t.Fatalf("threads=%d: found %d devices, want 1", threads, len(devices))
		}
		if devices[0]["IP"] != answering || devices[0]["SNMPHOSTNAME"] != "sw-a" {
			t.Errorf("threads=%d: device = %v", threads, devices[0])
		}
	}
}

// TestParseRange covers the CIDR, range and single-IP forms.
func TestParseRange(t *testing.T) {
	cidr, err := ParseRange("10.0.0.0/30")
	if err != nil {
		t.Fatal(err)
	}
	// /30 has 4 addresses; network and broadcast are skipped -> 2 hosts.
	if len(cidr) != 2 || cidr[0] != "10.0.0.1" || cidr[1] != "10.0.0.2" {
		t.Errorf("CIDR hosts = %v, want [10.0.0.1 10.0.0.2]", cidr)
	}

	rng, err := ParseRange("10.0.0.5-10.0.0.7")
	if err != nil {
		t.Fatal(err)
	}
	if len(rng) != 3 || rng[0] != "10.0.0.5" || rng[2] != "10.0.0.7" {
		t.Errorf("range = %v", rng)
	}

	single, err := ParseRange("10.0.0.42")
	if err != nil || len(single) != 1 || single[0] != "10.0.0.42" {
		t.Errorf("single = %v (err %v)", single, err)
	}

	if _, err := ParseRange("not-an-ip"); err == nil {
		t.Error("expected error for invalid spec")
	}
}

var errUnreachable = &scanError{}

type scanError struct{}

func (*scanError) Error() string { return "unreachable" }
