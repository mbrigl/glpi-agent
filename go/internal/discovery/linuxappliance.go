// SPDX-License-Identifier: GPL-2.0-only

package discovery

import (
	"encoding/hex"
	"regexp"
	"strconv"
	"strings"
)

// LinuxAppliance enhances generic net-snmp-on-Linux devices (sysObjectID under
// .1.3.6.1.4.1.8072.3.2.10) by detecting the actual appliance behind them.
// Ported from SNMP/MibSupport/LinuxAppliance.pm. The detection runs in getType,
// stashing what it finds under the private device key "_appliance", which the
// other accessors and the run hook then read (and GetInventory strips before
// output).

// LinuxAppliance OIDs (iso = .1.3.6.1.2.1, enterprises = .1.3.6.1.4.1).
const (
	laLinux = "1.3.6.1.4.1.8072.3.2.10"

	laDlmodName = "1.3.6.1.4.1.2021.13.14.2.1.2.1"

	laDsmModelName    = "1.3.6.1.4.1.6574.1.5.1.0"
	laDsmSerialNumber = "1.3.6.1.4.1.6574.1.5.2.0"
	laDsmVersion      = "1.3.6.1.4.1.6574.1.5.3.0"

	laSynoDiskID    = "1.3.6.1.4.1.6574.2.1.1.2"
	laSynoDiskModel = "1.3.6.1.4.1.6574.2.1.1.3"
	laSynoDiskType  = "1.3.6.1.4.1.6574.2.1.1.4"
	laSynoDiskName  = "1.3.6.1.4.1.6574.2.1.1.12"

	laSynoRaidName      = "1.3.6.1.4.1.6574.3.1.1.2"
	laSynoRaidFreeSize  = "1.3.6.1.4.1.6574.3.1.1.4"
	laSynoRaidTotalSize = "1.3.6.1.4.1.6574.3.1.1.5"

	laSvnProdName      = "1.3.6.1.4.1.2620.1.6.1.0"
	laSvnVersion       = "1.3.6.1.4.1.2620.1.6.4.1.0"
	laSvnApplianceSN   = "1.3.6.1.4.1.2620.1.6.16.3.0"
	laSvnApplianceMod  = "1.3.6.1.4.1.2620.1.6.16.7.0"
	laSvnApplianceManu = "1.3.6.1.4.1.2620.1.6.16.9.0"

	laSnmpEngineID = "1.3.6.1.6.3.10.2.1.1.0"

	laHrStorageEntry    = "1.3.6.1.2.1.25.2.3.1.3"
	laHrSWRunName       = "1.3.6.1.2.1.25.4.2.1.2"
	laHrSWInstalledName = "1.3.6.1.2.1.25.6.3.1.2"

	laUnifiApSystemModel   = "1.3.6.1.4.1.41112.1.6.3.3.0"
	laUnifiApSystemVersion = "1.3.6.1.4.1.41112.1.6.3.6.0"

	laUpsIdentModel  = "1.3.6.1.4.1.4555.1.1.7.1.1.1.0"
	laUpsIdentSerial = "1.3.6.1.4.1.4555.1.1.7.1.1.2.0"
	laUpsIdentSwVer  = "1.3.6.1.4.1.4555.1.1.7.1.1.5.0"
	laPrtPrinterName = "1.3.6.1.2.1.43.5.1.1.16.1"
	laQVendorID      = "1.3.6.1.4.1.2036.2.1.1.4.0"
	laQProdID        = "1.3.6.1.4.1.2036.2.1.1.5.0"
	laQProdRev       = "1.3.6.1.4.1.2036.2.1.1.6.0"
	laQSerialNumber  = "1.3.6.1.4.1.2036.2.1.1.12.0"
	laTplinkModel    = "1.3.6.1.4.1.11863.20.1.1.2.0"
	laTplinkFirmware = "1.3.6.1.4.1.11863.20.1.1.3.0"
	laTplinkMacID    = "1.3.6.1.4.1.11863.20.1.3.1.0"
	laDigiSerial     = "1.3.6.1.4.1.40083.6.1.0"
	laDigiFirmware   = "1.3.6.1.4.1.40083.6.2.0"
	laDigiModel      = "1.3.6.1.4.1.40083.6.3.0"
)

var laTplinkSysDescrRE = regexp.MustCompile(`(?i)^Linux (TL-\S+) [0-9.]+ #1`)
var laOmadaRE = regexp.MustCompile(`(?i)^(Omada .*)$`)
var laVeritasRE = regexp.MustCompile(`^VRTSnbserver-`)

func init() {
	registerMib(MibModule{
		Name:        "linuxAppliance",
		SysObjectID: oidMatch(laLinux),
		Type: func(g SNMPGetter, d Device) string {
			info, typ := laDetect(g, d)
			if typ == "" {
				return ""
			}
			d["_appliance"] = info
			return typ
		},
		Model: func(_ SNMPGetter, d Device) string { return laApplianceField(d, "MODEL") },
		Manufacturer: func(_ SNMPGetter, d Device) string {
			return laApplianceField(d, "MANUFACTURER")
		},
		Firmware: func(_ SNMPGetter, d Device) string { return laApplianceField(d, "FIRMWARE") },
		Serial:   laSerial,
		Run:      laRun,
	})
}

// laApplianceField reads a stashed _appliance string field.
func laApplianceField(d Device, key string) string {
	a, _ := d["_appliance"].(map[string]any)
	if a == nil {
		return ""
	}
	v, _ := a[key].(string)
	return v
}

// laDetect runs the ordered appliance detection of getType, returning the
// appliance info map and the device TYPE (or "" when nothing matched).
func laDetect(g SNMPGetter, d Device) (map[string]any, string) {
	// Seagate NAS: a /lacie storage path.
	if store, _ := g.Walk(laHrStorageEntry); anyValueMatch(store, regexp.MustCompile(`(?i)^/lacie`)) {
		return map[string]any{"MODEL": "Seagate NAS", "MANUFACTURER": "Seagate"}, "STORAGE"
	}
	// QuesCom.
	if mibGet(g, laDlmodName) == "QuesComSnmpObject" {
		return map[string]any{"MODEL": "QuesCom", "MANUFACTURER": "QuesCom"}, "NETWORKING"
	}
	// Synology.
	if model := mibGet(g, laDsmModelName); model != "" {
		return map[string]any{"MODEL": model, "MANUFACTURER": "Synology"}, "STORAGE"
	}
	// CheckPoint.
	if mibGet(g, laSvnApplianceManu) != "" {
		return map[string]any{"MODEL": mibGet(g, laSvnApplianceMod), "MANUFACTURER": "CheckPoint"}, "NETWORKING"
	}
	// Sophos UTM: an existing process.
	if laHasProcess(g, "mdw.plx") {
		return map[string]any{"MODEL": "Sophos UTM", "MANUFACTURER": "Sophos"}, "NETWORKING"
	}
	// UniFi AP.
	if model := mibGet(g, laUnifiApSystemModel); model != "" {
		return map[string]any{"MODEL": model, "MANUFACTURER": "Ubiquiti"}, "NETWORKING"
	}
	// Socomec UPS.
	if model := mibGet(g, laUpsIdentModel); model != "" {
		return map[string]any{"MODEL": model, "MANUFACTURER": "Socomec"}, "NETWORKING"
	}
	// Quantum appliance.
	if vendor := mibGet(g, laQVendorID); vendor != "" {
		return map[string]any{
			"MODEL": mibGet(g, laQProdID), "MANUFACTURER": vendor,
			"FIRMWARE": mibGet(g, laQProdRev), "SERIAL": mibGet(g, laQSerialNumber),
			"_QUANTUM": true,
		}, "NETWORKING"
	}
	// Digi Anywhere modem.
	if serial := mibGet(g, laDigiSerial); serial != "" {
		return map[string]any{
			"MODEL": mibGet(g, laDigiModel), "MANUFACTURER": "Digi",
			"FIRMWARE": mibGet(g, laDigiFirmware), "SERIAL": serial,
		}, "NETWORKING"
	}
	// TP-Link (private MIB, then sysDescr).
	if model := mibGet(g, laTplinkModel); model != "" {
		return map[string]any{
			"MODEL": model, "FIRMWARE": mibGet(g, laTplinkFirmware),
			"SERIAL": mibGet(g, laTplinkMacID), "MANUFACTURER": "TP-Link",
		}, "NETWORKING"
	}
	sysDescr := getOne(g, oidSysDescr)
	if m := laTplinkSysDescrRE.FindStringSubmatch(sysDescr); m != nil {
		return map[string]any{"MODEL": m[1], "MANUFACTURER": "TP-Link"}, "NETWORKING"
	}
	// Printer.
	if name := mibGet(g, laPrtPrinterName); name != "" {
		info := map[string]any{"MODEL": name}
		if regexp.MustCompile(`(?i)^Katusha`).MatchString(sysDescr) {
			info["MANUFACTURER"] = "Katusha"
		}
		return info, "PRINTER"
	}
	// SNMP-FRAMEWORK-MIB snmpEngineID analysis.
	return laDetectByEngineID(g, sysDescr)
}

// laDetectByEngineID decodes the snmpEngineID for the IANA manufacturer id and
// the embedded unique identifier, then refines via process/installed lookups.
func laDetectByEngineID(g SNMPGetter, sysDescr string) (map[string]any, string) {
	engine := laEngineBytes(mibGet(g, laSnmpEngineID))
	if len(engine) < 4 {
		return nil, ""
	}
	manufacturerID := (int(engine[0]&0x7f) << 24) | (int(engine[1]) << 16) | (int(engine[2]) << 8) | int(engine[3])
	match, ok := manufacturerIDInfo(strconv.Itoa(manufacturerID))
	if !ok || match.Manufacturer == "" || match.Type == "" {
		return nil, ""
	}
	info := map[string]any{"MODEL": match.Model, "MANUFACTURER": match.Manufacturer}

	// A unique identifier may follow when the high bit of the first byte is set.
	if engine[0]&0x80 != 0 && len(engine) >= 5 {
		remaining := engine[5:]
		switch {
		case engine[4] == 3:
			info["SERIAL"] = canonicalMAC(hexColon(remaining))
		case engine[4] == 4:
			info["SERIAL"] = strings.TrimSpace(string(remaining))
		case engine[4] == 5 || engine[4] >= 128:
			info["SERIAL"] = hex.EncodeToString(remaining)
		}
	}

	// Try to identify the device more precisely.
	switch {
	case laHasProcess(g, "sfestreamer"):
		info["MODEL"], info["MANUFACTURER"] = "FMC", "Cisco"
		return info, "NETWORKING"
	case laOmadaRE.MatchString(sysDescr):
		info["MODEL"] = laOmadaRE.FindStringSubmatch(sysDescr)[1]
		info["MANUFACTURER"] = "TP-Link"
		return info, "NETWORKING"
	case laHasInstalled(g, laVeritasRE):
		info["MODEL"], info["MANUFACTURER"] = "Veritas NetBackup", "Veritas Technologies LLC"
		return info, "NETWORKING"
	}
	return info, match.Type
}

// laSerial implements the manufacturer-specific serial selection of getSerial.
func laSerial(g SNMPGetter, d Device) string {
	a, _ := d["_appliance"].(map[string]any)
	if a == nil {
		return ""
	}
	manufacturer, _ := a["MANUFACTURER"].(string)
	if manufacturer == "" {
		return ""
	}
	switch manufacturer {
	case "Synology":
		return mibGet(g, laDsmSerialNumber)
	case "CheckPoint":
		return mibGet(g, laSvnApplianceSN)
	case "Seagate":
		return strings.TrimPrefix(strings.TrimSpace(mibGet(g, laSnmpEngineID)), "0x")
	case "Ubiquiti":
		if mac, _ := d["MAC"].(string); mac != "" {
			return strings.ReplaceAll(mac, ":", "")
		}
	case "Socomec":
		return mibGet(g, laUpsIdentSerial)
	default:
		if serial, _ := a["SERIAL"].(string); serial != "" {
			// Quantum: also fix a badly hex-encoded LOCATION.
			if q, _ := a["_QUANTUM"].(bool); q {
				if loc, _ := d["LOCATION"].(string); laIsEvenHex(loc) {
					if b, err := hex.DecodeString(loc); err == nil {
						d["LOCATION"] = strings.TrimSpace(string(b))
					}
				}
			}
			return serial
		}
	}
	return ""
}

// laRun adds the firmware (and, for Synology, the disks/volumes) of the detected
// appliance.
func laRun(g SNMPGetter, d Device) {
	a, _ := d["_appliance"].(map[string]any)
	if a == nil {
		return
	}
	manufacturer, _ := a["MANUFACTURER"].(string)
	if manufacturer == "" {
		return
	}

	var firmware map[string]any
	switch manufacturer {
	case "Synology":
		laSynologyStorages(g, d)
		laSynologyVolumes(g, d)
		if v := mibGet(g, laDsmVersion); v != "" {
			firmware = map[string]any{
				"NAME": manufacturer + " DSM", "DESCRIPTION": manufacturer + " DSM firmware",
				"TYPE": "system", "VERSION": v, "MANUFACTURER": manufacturer,
			}
		}
	case "CheckPoint":
		if v := mibGet(g, laSvnVersion); v != "" {
			firmware = map[string]any{
				"NAME": mibGet(g, laSvnProdName), "DESCRIPTION": manufacturer + " SVN version",
				"TYPE": "system", "VERSION": v, "MANUFACTURER": manufacturer,
			}
		}
	case "Ubiquiti":
		if v := mibGet(g, laUnifiApSystemVersion); v != "" {
			firmware = map[string]any{
				"NAME": laApplianceField(d, "MODEL"), "DESCRIPTION": "Unifi AP System version",
				"TYPE": "system", "VERSION": v, "MANUFACTURER": manufacturer,
			}
		}
	case "Socomec":
		if v := mibGet(g, laUpsIdentSwVer); v != "" {
			name, version := laApplianceField(d, "MODEL"), v
			if m := regexp.MustCompile(`^(.*) v([0-9.]+)$`).FindStringSubmatch(v); m != nil {
				name, version = m[1], m[2]
			}
			firmware = map[string]any{
				"NAME": name, "DESCRIPTION": "Socomec " + laApplianceField(d, "MODEL") + " software version",
				"TYPE": "system", "VERSION": version, "MANUFACTURER": manufacturer,
			}
		}
	}
	if firmware != nil {
		addFirmware(d, firmware)
	}
}

// laSynologyStorages appends the Synology disks to the device STORAGES.
func laSynologyStorages(g SNMPGetter, d Device) {
	models, _ := g.Walk(laSynoDiskModel)
	ids, _ := g.Walk(laSynoDiskID)
	types, _ := g.Walk(laSynoDiskType)
	names, _ := g.Walk(laSynoDiskName)
	for key, rawModel := range models {
		model := strings.TrimSpace(rawModel)
		if model == "" {
			continue
		}
		storage := map[string]any{"MODEL": model, "TYPE": "disk"}
		name := strings.TrimSpace(names[key])
		if name == "" {
			name = strings.TrimSpace(ids[key])
		}
		if name != "" {
			storage["NAME"] = name
		}
		if manu := canonicalDiskManufacturer(model); manu != "" {
			storage["MANUFACTURER"] = manu
		}
		if iface := strings.TrimSpace(types[key]); iface != "" {
			storage["INTERFACE"] = iface
		}
		list, _ := d["STORAGES"].([]map[string]any)
		d["STORAGES"] = append(list, storage)
	}
}

// laSynologyVolumes appends the Synology RAID volumes to the device DRIVES.
func laSynologyVolumes(g SNMPGetter, d Device) {
	names, _ := g.Walk(laSynoRaidName)
	frees, _ := g.Walk(laSynoRaidFreeSize)
	totals, _ := g.Walk(laSynoRaidTotalSize)
	for key, rawName := range names {
		name := strings.TrimSpace(rawName)
		if name == "" {
			continue
		}
		free, freeOK := canonicalSizeMB(frees[key])
		total, totalOK := canonicalSizeMB(totals[key])
		if !freeOK || !totalOK {
			continue
		}
		list, _ := d["DRIVES"].([]map[string]any)
		d["DRIVES"] = append(list, map[string]any{"VOLUMN": name, "FREE": free, "TOTAL": total})
	}
}

// laHasProcess reports whether a running process named exactly name is present
// (hrSWRunName walk).
func laHasProcess(g SNMPGetter, name string) bool {
	run, _ := g.Walk(laHrSWRunName)
	for _, v := range run {
		if strings.TrimSpace(v) == name {
			return true
		}
	}
	return false
}

// laHasInstalled reports whether an installed software name matches re
// (hrSWInstalledName walk).
func laHasInstalled(g SNMPGetter, re *regexp.Regexp) bool {
	installed, _ := g.Walk(laHrSWInstalledName)
	for _, v := range installed {
		if re.MatchString(strings.TrimSpace(v)) {
			return true
		}
	}
	return false
}

// anyValueMatch reports whether any walk value matches re.
func anyValueMatch(walk map[string]string, re *regexp.Regexp) bool {
	for _, v := range walk {
		if re.MatchString(strings.TrimSpace(v)) {
			return true
		}
	}
	return false
}

// laEngineBytes turns the snmpEngineID SNMP value into its bytes, mirroring the
// Perl double hex2char handling: a "0x"/colon-hex octet string is decoded, and a
// result that is itself an even-length hex string is decoded once more.
func laEngineBytes(s string) []byte {
	b := octetBytes(s)
	if laIsEvenHexBytes(b) {
		if decoded, err := hex.DecodeString(string(b)); err == nil {
			b = decoded
		}
	}
	return b
}

// octetBytes reconstructs the raw bytes of an SNMP octet string rendered as
// colon-hex ("aa:bb"), "0x"-hex, or plain text.
func octetBytes(s string) []byte {
	s = strings.TrimSpace(s)
	if s == "" {
		return nil
	}
	if strings.Contains(s, ":") {
		parts := strings.Split(s, ":")
		b := make([]byte, 0, len(parts))
		ok := true
		for _, p := range parts {
			n, err := strconv.ParseUint(p, 16, 8)
			if len(p) != 2 || err != nil {
				ok = false
				break
			}
			b = append(b, byte(n))
		}
		if ok {
			return b
		}
	}
	if strings.HasPrefix(s, "0x") || strings.HasPrefix(s, "0X") {
		if d, err := hex.DecodeString(s[2:]); err == nil {
			return d
		}
	}
	return []byte(s)
}

func laIsEvenHexBytes(b []byte) bool { return laIsEvenHex(string(b)) }

// laIsEvenHex reports whether s is a non-empty even-length hex string.
func laIsEvenHex(s string) bool {
	if len(s) == 0 || len(s)%2 != 0 {
		return false
	}
	for _, c := range s {
		if !((c >= '0' && c <= '9') || (c >= 'a' && c <= 'f') || (c >= 'A' && c <= 'F')) {
			return false
		}
	}
	return true
}

// canonicalSizeMB converts a "<n> bytes" value to mebibytes (base 1000, as
// getCanonicalSize does for the Synology volumes). It returns ok=false when the
// value is empty or unparsable.
func canonicalSizeMB(raw string) (int, bool) {
	raw = strings.TrimSpace(raw)
	if raw == "" {
		return 0, false
	}
	n, err := strconv.ParseInt(raw, 10, 64)
	if err != nil {
		return 0, false
	}
	return int(n / (1000 * 1000)), true
}

// canonicalDiskManufacturer derives a manufacturer from a disk model string,
// the disk-relevant subset of Tools.pm::getCanonicalManufacturer.
func canonicalDiskManufacturer(model string) string {
	if m := regexp.MustCompile(`(?i)(lg|broadcom|compaq|dell|epson|fujitsu|hitachi|ibm|intel|kingston|matshita|maxtor|nvidia|nec|pioneer|samsung|sony|supermicro|toshiba|transcend)`).FindStringSubmatch(model); m != nil {
		return strings.Title(strings.ToLower(m[1]))
	}
	prefixes := []struct {
		name string
		re   *regexp.Regexp
	}{
		{"Apple", regexp.MustCompile(`(?i)^APPLE`)},
		{"Hewlett-Packard", regexp.MustCompile(`^(hp|HPE?|(?i)hewlett[ -]packard|MM)`)},
		{"Hitachi", regexp.MustCompile(`^(HD|IC|HU|HGST)`)},
		{"Seagate", regexp.MustCompile(`^(ST|(?i)seagate)`)},
		{"Sony", regexp.MustCompile(`(?i)^OPTIARC`)},
		{"Western Digital", regexp.MustCompile(`^(WDC?|(?i)western)`)},
		{"Crucial", regexp.MustCompile(`^CT`)},
		{"PNY", regexp.MustCompile(`^PNY`)},
	}
	for _, p := range prefixes {
		if p.re.MatchString(model) {
			return p.name
		}
	}
	return ""
}
