// SPDX-License-Identifier: GPL-2.0-only

package discovery

import "strings"

// Seventh batch of upstream SNMP/MibSupport/* vendor modules: the identity-only
// accessors the existing MibSupport framework can host without a device-mutation
// hook. The page-counter / component / firmware rewrites of Xerox, Netgear and
// SiemensSicam's getComponents need a `run`-style hook and a richer device
// model; they remain follow-on (see docs/UPSTREAM-MAPPING.md).

func init() {
	// --- Siemens Sicam (identity from the sysDescr) ---
	// The full DGPI component/firmware walk needs a run hook; here we port the
	// identity accessors that parse "Siemens AG, <model...>, <hwrev>, FW: ..,
	// SN: .." out of the device DESCRIPTION (SiemensSicam::_getDescriptionData).
	registerMib(MibModule{
		Name: "siemens_sicam",
		// sysobjectid may arrive without the leading dot.
		SysObjectID: oidMatch("1.3.6.1.4.1.22638"),
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
