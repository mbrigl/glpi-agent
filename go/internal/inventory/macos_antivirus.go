// SPDX-License-Identifier: GPL-2.0-only

package inventory

import (
	"encoding/json"
	"regexp"
	"strconv"
	"strings"
	"time"
)

// macFirstMatch returns the first capture group of the first line matching re.
func macFirstMatch(text string, re *regexp.Regexp) string {
	for _, line := range strings.Split(text, "\n") {
		if m := re.FindStringSubmatch(line); m != nil {
			return m[1]
		}
	}
	return ""
}

// macEpochMsToDate formats an epoch-milliseconds string as the local "YYYY-MM-DD".
func macEpochMsToDate(ms string) string {
	if !regexp.MustCompile(`^\d+$`).MatchString(ms) {
		return ""
	}
	n, err := strconv.ParseInt(ms, 10, 64)
	if err != nil {
		return ""
	}
	return time.Unix(n/1000, 0).Local().Format("2006-01-02")
}

// buildMacDefender maps `mdatp health --output json` to an ANTIVIRUS entry,
// mirroring MacOS/AntiVirus/Defender.pm. Returns nil when the output is not a
// healthy Defender status.
func buildMacDefender(output []byte) map[string]any {
	var infos map[string]any
	if err := json.Unmarshal(output, &infos); err != nil {
		return nil
	}
	if healthy, _ := infos["healthy"].(bool); !healthy {
		return nil
	}

	av := map[string]any{
		"COMPANY":  "Microsoft",
		"NAME":     "Microsoft Defender",
		"ENABLED":  0,
		"UPTODATE": 0,
	}
	if v, ok := infos["appVersion"].(string); ok && v != "" {
		av["VERSION"] = v
	}
	if v, ok := infos["definitionsVersion"].(string); ok && v != "" {
		av["BASE_VERSION"] = v
	}
	if ds, ok := infos["definitionsStatus"].(map[string]any); ok {
		if ds["$type"] == "upToDate" {
			av["UPTODATE"] = 1
		}
	}
	if rt, ok := infos["realTimeProtectionEnabled"].(map[string]any); ok {
		if v, ok := rt["value"].(bool); ok && v {
			av["ENABLED"] = 1
		}
	}
	if exp, ok := infos["productExpiration"].(string); ok {
		if d := macEpochMsToDate(exp); d != "" {
			av["EXPIRATION"] = d
		}
	}
	if upd, ok := infos["definitionsUpdated"].(string); ok {
		if d := macEpochMsToDate(upd); d != "" {
			av["BASE_CREATION"] = d
		}
	}
	return av
}

var (
	macCortexVersionRE = regexp.MustCompile(`^Cortex XDR .* ([0-9.]+)$`)
	macCortexContentRE = regexp.MustCompile(`(?i)^Content Version:\s+(\S+)$`)
	macCortexPmdRE     = regexp.MustCompile(`(?i)^\s*pmd\s+\S+\s+\S+\s+(\S+)\s`)
)

// buildMacCortex maps the three `cytool` command outputs to an ANTIVIRUS entry,
// mirroring MacOS/AntiVirus/Cortex.pm.
func buildMacCortex(info, infoQuery, runtimeQuery string) map[string]any {
	av := map[string]any{
		"COMPANY": "Palo Alto Networks",
		"NAME":    "Cortex XDR",
		"ENABLED": 0,
	}
	if v := macFirstMatch(info, macCortexVersionRE); v != "" {
		av["VERSION"] = v
	}
	if v := macFirstMatch(infoQuery, macCortexContentRE); v != "" {
		av["BASE_VERSION"] = v
	}
	if status := macFirstMatch(runtimeQuery, macCortexPmdRE); strings.EqualFold(status, "Running") {
		av["ENABLED"] = 1
	}
	return av
}

var (
	macSentinelVersionRE = regexp.MustCompile(`^SentinelOne.* ([0-9.]+)$`)
	macSentinelStatusRE  = regexp.MustCompile(`(?im)^\s+Protection:\s+enabled\s*$`)
)

// buildMacSentinelOne maps the `sentinelctl version`/`status` outputs to an
// ANTIVIRUS entry, mirroring MacOS/AntiVirus/SentinelOne.pm.
func buildMacSentinelOne(version, status string) map[string]any {
	av := map[string]any{
		"COMPANY": "Sentinel Labs Inc.",
		"NAME":    "SentinelOne EPP",
		"ENABLED": 0,
	}
	if v := macFirstMatch(version, macSentinelVersionRE); v != "" {
		av["VERSION"] = v
	}
	if macSentinelStatusRE.MatchString(status) {
		av["ENABLED"] = 1
	}
	return av
}

var (
	macCrowdVersionRE = regexp.MustCompile(`^\s*version:\s*([0-9.]+[0-9]+)$`)
	macCrowdOpRE      = regexp.MustCompile(`(?i)Sensor operational: true`)
)

// buildMacCrowdStrike maps `falconctl stats agent_info` output to an ANTIVIRUS
// entry, mirroring MacOS/AntiVirus/CrowdStrike.pm.
func buildMacCrowdStrike(agentInfo string) map[string]any {
	av := map[string]any{
		"COMPANY": "CrowdStrike",
		"NAME":    "CrowdStrike Falcon Sensor",
		"ENABLED": 0,
	}
	if v := macFirstMatch(agentInfo, macCrowdVersionRE); v != "" {
		av["VERSION"] = v
	}
	if macCrowdOpRE.MatchString(agentInfo) {
		av["ENABLED"] = 1
	}
	return av
}

var (
	macWithSecureVerRE  = regexp.MustCompile(`(?i)ClientSecurity\s+version\s+([\d.]+)`)
	macWithSecureBaseRE = regexp.MustCompile(`(?i)^Database\s+version:\s*(\S+)`)
)

// buildMacWithSecure maps `wsav --version` output (+ the wsavd process state) to
// an ANTIVIRUS entry, mirroring MacOS/AntiVirus/WithSecure.pm. Returns nil when
// there is no output.
func buildMacWithSecure(versionOutput string, wsavdRunning bool) map[string]any {
	if strings.TrimSpace(versionOutput) == "" {
		return nil
	}
	av := map[string]any{
		"NAME":     "WithSecure Client Security for Mac",
		"COMPANY":  "WithSecure",
		"ENABLED":  0,
		"UPTODATE": 0,
	}
	if v := macFirstMatch(versionOutput, macWithSecureVerRE); v != "" {
		av["VERSION"] = v
	}
	if v := macFirstMatch(versionOutput, macWithSecureBaseRE); v != "" {
		av["BASE_VERSION"] = v
	}
	if wsavdRunning {
		av["ENABLED"] = 1
	}
	return av
}
