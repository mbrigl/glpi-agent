// SPDX-License-Identifier: GPL-2.0-only

package discovery

import (
	_ "embed"
	"strings"
	"sync"
)

// sysobjectIDsData is the upstream classification database
// (share/sysobject.ids): tab-separated "<id>\t<manufacturer>\t<type>\t<model>
// [\t<module>]", where <id> is the OID below the enterprises arc
// (.1.3.6.1.4.1). Embedded so the agent stays a single static binary.
//
//go:embed data/sysobject.ids
var sysobjectIDsData string

// classification is one sysObjectID database entry.
type classification struct {
	Manufacturer string
	Type         string
	Model        string
}

var (
	sysobjectOnce sync.Once
	sysobjectDB   map[string]classification
)

func sysobjectDatabase() map[string]classification {
	sysobjectOnce.Do(func() {
		sysobjectDB = make(map[string]classification, 10000)
		for _, line := range strings.Split(sysobjectIDsData, "\n") {
			if line == "" || strings.HasPrefix(line, "#") {
				continue
			}
			f := strings.Split(line, "\t")
			if len(f) < 3 {
				continue
			}
			entry := classification{Manufacturer: f[1], Type: f[2]}
			if len(f) >= 4 {
				entry.Model = f[3]
			}
			sysobjectDB[f[0]] = entry
		}
	})
	return sysobjectDB
}

// enterprisePrefixes are the textual forms a sysObjectID may carry before the
// enterprise arc, mirroring the prefix regex in Hardware.pm::_getSysObjectIDInfo.
var enterprisePrefixes = []string{
	"SNMPv2-SMI::enterprises.",
	"iso.3.6.1.4.1.",
	".1.3.6.1.4.1.",
	"1.3.6.1.4.1.",
}

// parseEnterprise splits a sysObjectID into its manufacturer id (first number
// under the enterprises arc) and the remaining device id.
func parseEnterprise(sysObjectID string) (manufacturerID, deviceID string, ok bool) {
	rest := ""
	for _, p := range enterprisePrefixes {
		if strings.HasPrefix(sysObjectID, p) {
			rest = strings.TrimPrefix(sysObjectID, p)
			break
		}
	}
	if rest == "" {
		return "", "", false
	}
	if i := strings.IndexByte(rest, '.'); i >= 0 {
		return rest[:i], rest[i+1:], true
	}
	return rest, "", true
}

// classifyBySysObjectID reproduces Hardware.pm::_getSysObjectIDInfo: a full
// match on "<manufacturer>.<device>", then progressively shorter device-id
// prefixes, then the manufacturer id alone.
func classifyBySysObjectID(sysObjectID string) (classification, bool) {
	mid, did, ok := parseEnterprise(strings.TrimSpace(sysObjectID))
	if !ok || mid == "" {
		return classification{}, false
	}
	db := sysobjectDatabase()

	if did != "" {
		if m, ok := db[mid+"."+did]; ok {
			return m, true
		}
		for {
			i := strings.LastIndexByte(did, '.')
			if i < 0 {
				break
			}
			did = did[:i]
			if m, ok := db[mid+"."+did]; ok {
				return m, true
			}
		}
	}
	if m, ok := db[mid]; ok {
		return m, true
	}
	return classification{}, false
}
