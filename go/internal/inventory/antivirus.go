// SPDX-License-Identifier: GPL-2.0-only

package inventory

import "encoding/json"

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
