// SPDX-License-Identifier: GPL-2.0-only

package inventory

import (
	"encoding/json"
	"regexp"
	"strings"
)

// ParseDefenderHealth builds the Microsoft Defender ANTIVIRUS entry from
// `mdatp health --output json`, mirroring Linux/AntiVirus/Defender.pm. It
// returns nil when the product is not healthy.
func ParseDefenderHealth(data []byte) map[string]any {
	var info struct {
		Healthy            bool   `json:"healthy"`
		AppVersion         string `json:"appVersion"`
		DefinitionsVersion string `json:"definitionsVersion"`
		DefinitionsStatus  struct {
			Type string `json:"$type"`
		} `json:"definitionsStatus"`
		RealTimeProtectionEnabled struct {
			Value bool `json:"value"`
		} `json:"realTimeProtectionEnabled"`
	}
	if err := json.Unmarshal(data, &info); err != nil || !info.Healthy {
		return nil
	}

	av := map[string]any{
		"COMPANY":  "Microsoft",
		"NAME":     "Microsoft Defender",
		"ENABLED":  boolToInt(info.RealTimeProtectionEnabled.Value),
		"UPTODATE": boolToInt(info.DefinitionsStatus.Type == "upToDate"),
	}
	setIf(av, "VERSION", info.AppVersion)
	setIf(av, "BASE_VERSION", info.DefinitionsVersion)
	return av
}

// kvLine splits a "key: value" line (optionally a leading "- " bullet),
// mirroring the key/value parsing used by several AntiVirus modules.
func kvLine(line string) (key, value string, ok bool) {
	line = strings.TrimSpace(line)
	line = strings.TrimPrefix(line, "- ")
	k, v, found := strings.Cut(line, ":")
	if !found {
		return "", "", false
	}
	return strings.TrimSpace(k), strings.TrimSpace(v), true
}

var biosDateRE = regexp.MustCompile(`^(\d{4}-\d+-\d+)`)

// ParseBitdefender builds the Bitdefender BEST entry from `bduitool get ps`,
// mirroring Linux/AntiVirus/Bitdefender.pm.
func ParseBitdefender(out string) map[string]any {
	av := map[string]any{
		"NAME": "Bitdefender Endpoint Security Tools (BEST) for Linux", "COMPANY": "Bitdefender",
		"ENABLED": 0, "UPTODATE": 1,
	}
	for _, line := range strings.Split(out, "\n") {
		k, v, ok := kvLine(line)
		if !ok {
			continue
		}
		switch {
		case k == "Product version":
			av["VERSION"] = v
		case k == "Engines version":
			av["BASE_VERSION"] = v
		case k == "Antimalware status":
			av["ENABLED"] = boolToInt(v == "On")
		case strings.Contains(k, "available") && v != "no":
			av["UPTODATE"] = 0
		case k == "Last security content update":
			if m := biosDateRE.FindStringSubmatch(v); m != nil {
				av["BASE_CREATION"] = m[1]
			}
		}
	}
	return av
}

// ParseSentinelOne builds the SentinelOne entry from the combined sentinelctl
// output, mirroring Linux/AntiVirus/Sentinelone.pm.
func ParseSentinelOne(out string) map[string]any {
	av := map[string]any{"NAME": "SentinelAgent", "COMPANY": "SentinelOne", "ENABLED": 0, "UPTODATE": 0}
	for _, line := range strings.Split(out, "\n") {
		k, v, ok := kvLine(line)
		if !ok {
			// fall back to a 2+ space separator
			if f := regexp.MustCompile(`\s{2,}`).Split(strings.TrimSpace(line), 2); len(f) == 2 {
				k, v = strings.TrimSpace(f[0]), strings.TrimSpace(f[1])
			} else {
				continue
			}
		}
		switch k {
		case "Agent version":
			av["VERSION"] = v
		case "DFI library version":
			av["BASE_VERSION"] = v
		case "Agent state":
			av["ENABLED"] = boolToInt(v == "Enabled")
		case "Connectivity":
			av["UPTODATE"] = boolToInt(v == "On")
		}
	}
	return av
}

var (
	cortexVersionRE = regexp.MustCompile(`(?m)^Cortex XDR .* ([0-9.]+)$`)
	cortexContentRE = regexp.MustCompile(`(?mi)^Content Version:\s+(\S+)$`)
	drwebVersionRE  = regexp.MustCompile(`([0-9][0-9.]+)`)
	eeaVersionRE    = regexp.MustCompile(`\(eea\)\s*([0-9.]+)`)
	eeaLicenseRE    = regexp.MustCompile(`License Validity:\s*(\d{4}-\d{2}-\d{2})`)
	eeaBaseRE       = regexp.MustCompile(`(?m)EM002\s*(\d+\s*\(\d+\))\s*Detection engine$`)
	keslVersionRE   = regexp.MustCompile(`(?m)^Version:\s+([\d.]+)`)
	keslExpireRE    = regexp.MustCompile(`(?i)license expiration date:\s+([\d-]+)`)
	keslBaseRE      = regexp.MustCompile(`(?m)^Last release date of databases:\s+([\d-]+)`)
)

// ParseCortex builds the Cortex XDR entry from `cytool info` (+ `info query`),
// mirroring Linux/AntiVirus/Cortex.pm.
func ParseCortex(info, query string) map[string]any {
	av := map[string]any{"NAME": "Cortex XDR", "COMPANY": "Palo Alto Networks", "ENABLED": 0}
	if m := cortexVersionRE.FindStringSubmatch(info); m != nil {
		av["VERSION"] = m[1]
	}
	if m := cortexContentRE.FindStringSubmatch(query); m != nil {
		av["BASE_VERSION"] = m[1]
	}
	return av
}

// ParseDrWeb builds the Dr.Web entry from `drweb-ctl --version`, the configd
// service state and `drweb-ctl baseinfo`, mirroring Linux/AntiVirus/DrWeb.pm.
func ParseDrWeb(version, serviceState, baseinfo string) map[string]any {
	av := map[string]any{"NAME": "Dr.Web", "COMPANY": "Doctor Web", "ENABLED": 0, "UPTODATE": 0}
	if m := drwebVersionRE.FindStringSubmatch(version); m != nil {
		av["VERSION"] = m[1]
	}
	av["ENABLED"] = boolToInt(strings.TrimSpace(serviceState) == "active")
	if m := regexp.MustCompile(`(?m)([0-9]{4}-[0-9]{2}-[0-9]{2})`).FindStringSubmatch(baseinfo); m != nil {
		av["BASE_VERSION"] = m[1]
	}
	return av
}

// ParseEEA builds the ESET Endpoint Antivirus entry, mirroring
// Linux/AntiVirus/EEA.pm (upd -version, eset service, lic --status, modules).
func ParseEEA(updVersion, serviceState, licStatus, modules string) map[string]any {
	av := map[string]any{"NAME": "ESET Endpoint Antivirus", "COMPANY": "ESET", "ENABLED": 0, "UPTODATE": 0}
	if m := eeaVersionRE.FindStringSubmatch(updVersion); m != nil {
		av["VERSION"] = m[1]
	}
	av["ENABLED"] = boolToInt(strings.TrimSpace(serviceState) == "active")
	if m := eeaLicenseRE.FindStringSubmatch(licStatus); m != nil {
		av["EXPIRATION"] = m[1]
	}
	if m := eeaBaseRE.FindStringSubmatch(modules); m != nil {
		av["BASE_VERSION"] = strings.Join(strings.Fields(m[1]), " ")
	}
	return av
}

// ParseKESL builds the Kaspersky Endpoint Security for Linux entry from the
// kesl service state and `kesl-control --app-info`, mirroring
// Linux/AntiVirus/KESL.pm.
func ParseKESL(serviceState, appInfo string) map[string]any {
	av := map[string]any{"NAME": "Kaspersky Endpoint Security for Linux", "COMPANY": "Kaspersky Lab", "ENABLED": 0, "UPTODATE": 0}
	av["ENABLED"] = boolToInt(strings.TrimSpace(serviceState) == "active")
	if m := keslVersionRE.FindStringSubmatch(appInfo); m != nil {
		av["VERSION"] = m[1]
	}
	if m := keslExpireRE.FindStringSubmatch(appInfo); m != nil {
		av["EXPIRATION"] = m[1]
	}
	if m := keslBaseRE.FindStringSubmatch(appInfo); m != nil {
		av["BASE_VERSION"] = m[1]
	}
	return av
}

var crowdStrikeVersionRE = regexp.MustCompile(`version\s*=\s*([0-9.]+[0-9]+)`)

// ParseCrowdStrikeVersion builds the CrowdStrike Falcon ANTIVIRUS entry from
// `/opt/CrowdStrike/falconctl -g --version`, mirroring
// Linux/AntiVirus/CrowdStrike.pm: ENABLED is assumed once a version is found.
func ParseCrowdStrikeVersion(out string) map[string]any {
	m := crowdStrikeVersionRE.FindStringSubmatch(out)
	if m == nil {
		return nil
	}
	return map[string]any{
		"NAME":    "CrowdStrike Falcon Sensor",
		"COMPANY": "CrowdStrike",
		"ENABLED": 1,
		"VERSION": m[1],
	}
}
