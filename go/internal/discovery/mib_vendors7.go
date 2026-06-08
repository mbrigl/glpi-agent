// SPDX-License-Identifier: GPL-2.0-only

package discovery

import (
	"sort"
	"strconv"
	"strings"
)

// Seventh batch of upstream SNMP/MibSupport/* vendor modules: the ones that use
// the run / getComponents device-mutation hooks (page counters, component
// serial fix-ups, component+firmware rewrites) rather than only identity
// accessors. Ported verbatim from the Perl OIDs.

func init() {
	registerXerox()
	registerNetgear()
	registerSiemensSicam()
}

// --- Xerox printers (page counters) ---
// XEROX-HOST-RESOURCES-EXT-MIB counters under xcmHrDevDetailEntry, summed into
// the PAGECOUNTERS section by the run hook (Xerox::run).
func registerXerox() {
	const (
		xerox             = "1.3.6.1.4.1.253"
		xeroxCommonMIB    = xerox + ".8"
		xcmHrDevDetail    = xeroxCommonMIB + ".53.13.2.1"
		xeroxTotalPrint   = xcmHrDevDetail + ".6.1.20.1"
		xeroxColorPrint   = xcmHrDevDetail + ".6.1.20.33"
		xeroxBlackPrint   = xcmHrDevDetail + ".6.1.20.34"
		xeroxColorCopy    = xcmHrDevDetail + ".6.11.20.25"
		xeroxBlackCopy    = xcmHrDevDetail + ".6.11.20.3"
		xeroxScanByEmail  = xcmHrDevDetail + ".6.10.20.11"
		xeroxScanOnNetwrk = xcmHrDevDetail + ".6.10.20.12"
	)
	// counter name -> the OIDs to sum (a single OID for most).
	mapping := map[string][]string{
		"PRINTCOLOR": {xeroxColorPrint},
		"PRINTBLACK": {xeroxBlackPrint},
		"PRINTTOTAL": {xeroxTotalPrint},
		"COPYCOLOR":  {xeroxColorCopy},
		"COPYBLACK":  {xeroxBlackCopy},
		"SCANNED":    {xeroxScanByEmail, xeroxScanOnNetwrk},
	}
	registerMib(MibModule{
		Name:        "xerox-printer",
		SysObjectID: oidMatch(xeroxCommonMIB),
		Run: func(g SNMPGetter, d Device) {
			// Sorted keys to match the deterministic Perl iteration order.
			names := make([]string, 0, len(mapping))
			for k := range mapping {
				names = append(names, k)
			}
			sort.Strings(names)
			for _, counter := range names {
				count := 0
				for _, oid := range mapping[counter] {
					count += atoiSafe(mibGet(g, oid))
				}
				if count > 0 {
					setPageCounter(d, counter, count)
				}
			}
			// COPYTOTAL when a copy counter is present but none was reported.
			counters, _ := d["PAGECOUNTERS"].(map[string]any)
			if counters == nil {
				return
			}
			cc, _ := counters["COPYCOLOR"].(int)
			cb, _ := counters["COPYBLACK"].(int)
			if cc != 0 || cb != 0 {
				counters["COPYTOTAL"] = cc + cb
			}
		},
	})
}

// --- Netgear switches (fix stacked-chassis serials) ---
// NETGEAR-INVENTORY-MIB / NG700-INVENTORY-MIB: when a stack exposes more than
// one chassis component, fill each unit's SERIAL (and STACK_NUMBER) from the
// inventory unit table (Netgear::run).
func registerNetgear() {
	const (
		netgear     = "1.3.6.1.4.1.4526"
		fastPath    = netgear + ".10.13"
		unitEntry   = fastPath + ".2.2.1"
		unitStatus  = unitEntry + ".11"
		unitSerial  = unitEntry + ".19"
		fastPath2   = netgear + ".11.13"
		unitEntry2  = fastPath2 + ".2.2.1"
		unitStatus2 = unitEntry2 + ".11"
		unitSerial2 = unitEntry2 + ".19"
	)
	run := func(g SNMPGetter, d Device) {
		container, _ := d["COMPONENTS"].(map[string]any)
		if container == nil {
			return
		}
		list, _ := container["COMPONENT"].([]map[string]any)
		var chassis []map[string]any
		for _, c := range list {
			if t, _ := c["TYPE"].(string); t == "chassis" {
				chassis = append(chassis, c)
			}
		}
		if len(chassis) <= 1 {
			return
		}
		status := firstWalk(g, unitStatus, unitStatus2)
		serial := firstWalk(g, unitSerial, unitSerial2)
		for _, c := range chassis {
			name, _ := c["NAME"].(string)
			if !strings.HasPrefix(name, "Unit ") {
				continue
			}
			unit := strings.TrimSpace(strings.TrimPrefix(name, "Unit "))
			if _, err := strconv.Atoi(unit); err != nil {
				continue
			}
			if strings.TrimSpace(status[unit]) != "1" || strings.TrimSpace(serial[unit]) == "" {
				continue
			}
			c["SERIAL"] = strings.TrimSpace(serial[unit])
			// Set STACK_NUMBER unconditionally (the "unknown glpi version" branch).
			c["STACK_NUMBER"] = unit
		}
	}
	registerMib(MibModule{Name: "netgear-ng7000", OID: fastPath, Run: run})
	registerMib(MibModule{Name: "netgear-ng700", OID: fastPath2, Run: run})
}

// --- Siemens Sicam ---
// Identity from the sysDescr plus the DGPI product-component table, which builds
// the COMPONENTS list and rewrites FIRMWARES (SiemensSicam::getComponents).
func registerSiemensSicam() {
	const (
		siemens       = "1.3.6.1.4.1.22638"
		dgpiEntry     = siemens + ".11.1.2.1.1"
		dgpiContained = dgpiEntry + ".2"
		dgpiClass     = dgpiEntry + ".3"
		dgpiName      = dgpiEntry + ".4"
		dgpiDescr     = dgpiEntry + ".5"
		dgpiOrderNum  = dgpiEntry + ".6"
		dgpiSerial    = dgpiEntry + ".7"
		dgpiVersion   = dgpiEntry + ".8"
		dgpiHwSlot    = dgpiEntry + ".9"
	)
	prodCompClass := map[string]string{
		"1": "hwProduct", "2": "swProduct", "3": "mainHwComponent",
		"4": "extensionHwComponent", "5": "updatableHwComponent",
		"6": "mainFwSwComponent", "7": "extensionFwSwComponent",
		"8": "configurationComponent",
	}
	registerMib(MibModule{
		Name: "siemens_sicam",
		// sysobjectid may arrive without the leading dot.
		SysObjectID: oidMatch(siemens),
		Type:        func(SNMPGetter, Device) string { return "NETWORKING" },
		Manufacturer: func(_ SNMPGetter, d Device) string {
			if m, _ := d["MANUFACTURER"].(string); strings.TrimSpace(m) != "" {
				return ""
			}
			return "Siemens"
		},
		Model:    func(_ SNMPGetter, d Device) string { return sicamInfo(d).model },
		Serial:   func(_ SNMPGetter, d Device) string { return sicamInfo(d).serial },
		Firmware: func(_ SNMPGetter, d Device) string { return sicamInfo(d).firmware },
		Components: func(g SNMPGetter, d Device) []map[string]any {
			contained, _ := g.Walk(dgpiContained)
			if len(contained) == 0 {
				return nil
			}
			class, _ := g.Walk(dgpiClass)
			names, _ := g.Walk(dgpiName)
			descrs, _ := g.Walk(dgpiDescr)
			orders, _ := g.Walk(dgpiOrderNum)
			serials, _ := g.Walk(dgpiSerial)
			versions, _ := g.Walk(dgpiVersion)
			slots, _ := g.Walk(dgpiHwSlot)

			keys := make([]string, 0, len(contained))
			for k := range contained {
				keys = append(keys, k)
			}
			sort.Slice(keys, func(i, j int) bool { return ifIndexLess(keys[i], keys[j]) })

			manufacturer, _ := d["MANUFACTURER"].(string)
			var components []map[string]any
			var firmwares []map[string]any
			for _, key := range keys {
				name := strings.TrimSpace(names[key])
				serial := strings.TrimSpace(serials[key])
				version := strings.TrimSpace(firstNonEmpty(versions[key], orders[key]))
				typ := prodCompClass[strings.TrimSpace(class[key])]
				if typ == "" {
					typ = "unknown"
				}
				idx, _ := strconv.Atoi(key)
				comp := map[string]any{
					"CONTAINEDININDEX": strings.TrimSpace(contained[key]),
					"INDEX":            idx,
					"NAME":             name,
					"TYPE":             typ,
				}
				if serial != "" {
					comp["SERIAL"] = serial
				}
				if version != "" {
					comp["FIRMWARE"] = version
					description := strings.TrimSpace(descrs[key])
					if description == "" {
						description = name
					}
					if slot := strings.TrimSpace(slots[key]); slot != "" {
						description += " on " + slot + " slot"
					}
					firmwares = append(firmwares, map[string]any{
						"NAME":         name,
						"DESCRIPTION":  description,
						"TYPE":         typ,
						"VERSION":      version,
						"MANUFACTURER": manufacturer,
					})
				}
				components = append(components, comp)
			}
			// Replace the FIRMWARES section with the component firmwares.
			if len(firmwares) > 0 {
				delete(d, "FIRMWARES")
				for _, fw := range firmwares {
					addFirmware(d, fw)
				}
			}
			return components
		},
	})
}

// sicamFields holds the model/serial/firmware parsed from a Siemens Sicam
// DESCRIPTION, mirroring SiemensSicam::_getDescriptionData.
type sicamFields struct{ model, serial, firmware string }

// sicamInfo parses the device DESCRIPTION of the form
// "Siemens AG, <model0>, <model1>, <hwrev>, FW: <fw>, SN: <sn>".
func sicamInfo(d Device) sicamFields {
	var f sicamFields
	descr, _ := d["DESCRIPTION"].(string)
	if !strings.HasPrefix(descr, "Siemens AG,") {
		return f
	}
	parts := strings.Split(descr, ",")
	for i := range parts {
		parts[i] = strings.TrimSpace(parts[i])
	}
	if len(parts) > 2 {
		f.model = strings.TrimSpace(parts[1] + " " + parts[2])
	}
	if len(parts) > 4 {
		if fw := strings.TrimPrefix(parts[4], "FW:"); fw != parts[4] {
			f.firmware = strings.TrimSpace(fw)
		}
	}
	if len(parts) > 5 {
		if sn := strings.TrimPrefix(parts[5], "SN:"); sn != parts[5] {
			f.serial = strings.TrimSpace(sn)
		}
	}
	return f
}

// atoiSafe parses a base-10 integer, returning 0 for any non-numeric input
// (mirrors the `$count =~ /^\d+$/` guard around the Perl counter reads).
func atoiSafe(s string) int {
	n, err := strconv.Atoi(strings.TrimSpace(s))
	if err != nil {
		return 0
	}
	return n
}

// firstWalk returns the first non-empty Walk result among the given base OIDs
// (mirrors `$self->walk(a) // $self->walk(b)`).
func firstWalk(g SNMPGetter, bases ...string) map[string]string {
	for _, base := range bases {
		if w, _ := g.Walk(base); len(w) > 0 {
			return w
		}
	}
	return map[string]string{}
}
