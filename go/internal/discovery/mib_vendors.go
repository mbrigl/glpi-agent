// SPDX-License-Identifier: GPL-2.0-only

package discovery

import (
	"regexp"
	"strings"
)

// This file ports an initial batch of the upstream SNMP/MibSupport/* vendor
// modules. Each registers its match rule(s) and the value accessors; the OIDs
// are copied verbatim from the corresponding Perl module. The remaining vendor
// modules are mechanical additions following the same pattern.

// mibGet reads a scalar OID and returns its trimmed value (Device::get).
func mibGet(g SNMPGetter, oid string) string { return getOne(g, oid) }

func init() {
	// --- Mikrotik (MIKROTIK-MIB) ---
	const mikrotik = "1.3.6.1.4.1.14988"
	const mtxrSystem = mikrotik + ".1.1.7"
	registerMib(MibModule{
		Name:        "mikrotik",
		SysObjectID: oidMatch(mikrotik),
		Firmware:    func(g SNMPGetter, _ Device) string { return mibGet(g, mtxrSystem+".4.0") },
		Serial:      func(g SNMPGetter, _ Device) string { return mibGet(g, mtxrSystem+".3.0") },
		Model: func(_ SNMPGetter, d Device) string {
			// RouterOS exposes the model in sysDescr ("RouterOS <model>").
			descr, _ := d["DESCRIPTION"].(string)
			if m := regexp.MustCompile(`^RouterOS\s+(.*)$`).FindStringSubmatch(descr); m != nil {
				return m[1]
			}
			return ""
		},
	})

	// --- Ubiquiti (UBNT) ---
	const ubnt = "1.3.6.1.4.1.41112"
	const ubntWlStatApMac = ubnt + ".1.4.5.1.4.1"
	const unifiApSystemVersion = ubnt + ".1.6.3.6.0"
	const unifiApSystemModel = ubnt + ".1.6.3.3.0"
	const unifiVapEssid = ubnt + ".1.6.1.2.1.6"
	const unifiVapName = ubnt + ".1.6.1.2.1.7"
	registerMib(MibModule{
		Name:        "ubnt",
		OID:         ubnt,
		SysObjectID: oidMatch(ubnt),
		Firmware:    func(g SNMPGetter, _ Device) string { return mibGet(g, unifiApSystemVersion) },
		Model:       func(g SNMPGetter, _ Device) string { return mibGet(g, unifiApSystemModel) },
		Mac:         func(g SNMPGetter, _ Device) string { return mibGet(g, ubntWlStatApMac) },
		Serial: func(g SNMPGetter, d Device) string {
			serial := mibGet(g, ubntWlStatApMac)
			if serial == "" {
				serial, _ = d["MAC"].(string)
			}
			return strings.ReplaceAll(serial, ":", "")
		},
		Run: func(g SNMPGetter, d Device) { ubntEnrichRadioPorts(g, d, unifiVapEssid, unifiVapName) },
	})

	// --- Dell (PowerConnect / OS10) ---
	const dell = "1.3.6.1.4.1.674"
	const powerConnectVendorMIB = dell + ".10895.3000.1.2"
	const productIdentification = powerConnectVendorMIB + ".100"
	const os10Products = dell + ".11000.5000.100.2"
	const os10ChassisObject = dell + ".11000.5000.100.4.1.1"
	registerMib(MibModule{
		Name:        "dell-powerconnect",
		OID:         powerConnectVendorMIB,
		SysObjectID: oidMatch(os10Products),
		Type:        func(SNMPGetter, Device) string { return "NETWORKING" },
		Firmware:    func(g SNMPGetter, _ Device) string { return mibGet(g, productIdentification+".4.0") },
		Manufacturer: func(g SNMPGetter, _ Device) string {
			if v := mibGet(g, productIdentification+".3.0"); v != "" {
				return v
			}
			return "Dell"
		},
		Serial: func(g SNMPGetter, _ Device) string {
			if v := mibGet(g, productIdentification+".8.1.2.1"); v != "" {
				return v
			}
			return mibGet(g, os10ChassisObject+".3.1.5.1") // os10 PPID
		},
		Mac: func(g SNMPGetter, _ Device) string { return mibGet(g, os10ChassisObject+".3.1.3.1") },
	})

	// --- Fortinet (FortiGate / FortiAP) ---
	const fortinet = "1.3.6.1.4.1.12356"
	const fnCoreMib = fortinet + ".100"
	const fnFortiGateMib = fortinet + ".101"
	const fnFortiAPMib = fortinet + ".120"
	registerMib(MibModule{
		Name:        "fortinet",
		SysObjectID: oidMatch(fnFortiGateMib),
		Serial:      func(g SNMPGetter, _ Device) string { return mibGet(g, fnCoreMib+".1.1.1.0") },
	})
	registerMib(MibModule{
		Name:        "fortinet-ap",
		SysObjectID: oidMatch(fnFortiAPMib),
		Serial:      func(g SNMPGetter, _ Device) string { return mibGet(g, fnFortiAPMib+".1.2.0") },
		Firmware:    func(g SNMPGetter, _ Device) string { return mibGet(g, fnFortiAPMib+".1.1.0") },
	})
}

// UBNT radio-port enrichment regexes (Ubnt::run).
var (
	ubntRadioRE = regexp.MustCompile(`^(?:ra\d+|rai\d+|wifi\d+ap\d+)(?:\.\d+)?$`)
	ubntVlanRE  = regexp.MustCompile(`^(.+)\.(\d+)$`)
	ubnt24RE    = regexp.MustCompile(`^(?:ra|wifi0ap)\d+$`)
	ubnt5RE     = regexp.MustCompile(`^(?:rai|wifi1ap)\d+$`)
)

// ubntEnrichRadioPorts ports Ubnt::run: for each WiFi radio port (raX / raiX /
// wifiNapX, optionally a .VLAN sub-interface) it fixes the bogus Ethernet
// iftype (6 -> 71 WiFi), sets IFALIAS to the interface name, and replaces IFNAME
// with the SSID annotated by radio band and any VLAN id. The radio<->SSID
// correlation comes from the unifiVapName / unifiVapEssid tables by shared index.
func ubntEnrichRadioPorts(g SNMPGetter, d Device, essidOID, nameOID string) {
	ports, _ := d["PORTS"].([]map[string]any)
	if len(ports) == 0 {
		return
	}
	essids, _ := g.Walk(essidOID)
	names, _ := g.Walk(nameOID)

	for _, p := range ports {
		ifdescr, _ := p["IFDESCR"].(string)
		if !ubntRadioRE.MatchString(ifdescr) {
			continue
		}
		// UBNT APs misreport WiFi interfaces as Ethernet(6); fix to WiFi(71).
		if t, _ := p["IFTYPE"].(string); t == "6" {
			p["IFTYPE"] = "71"
		}

		parent, vlan := ifdescr, ""
		if m := ubntVlanRE.FindStringSubmatch(ifdescr); m != nil {
			parent, vlan = m[1], m[2]
		}

		for idx, vapName := range names {
			if strings.TrimSpace(vapName) != parent {
				continue
			}
			p["IFALIAS"] = ifdescr
			ssid := strings.TrimSpace(essids[idx])
			if ssid == "" {
				break
			}
			band := ""
			switch {
			case ubnt24RE.MatchString(parent):
				band = "2.4GHz"
			case ubnt5RE.MatchString(parent):
				band = "5GHz"
			}
			switch {
			case band != "" && vlan != "":
				ssid += " (" + band + ", VLAN " + vlan + ")"
			case band != "":
				ssid += " (" + band + ")"
			case vlan != "":
				ssid += " (VLAN " + vlan + ")"
			}
			p["IFNAME"] = ssid
			break
		}
	}
}
