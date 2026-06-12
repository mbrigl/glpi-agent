// SPDX-License-Identifier: GPL-2.0-only

//go:build windows

package collect

import (
	"fmt"
	"strconv"
	"strings"

	"golang.org/x/sys/windows/registry"

	"github.com/glpi-project/glpi-agent/go/internal/logging"
)

// RegistryCollector implements "getFromRegistry" (Collect/Registry.pm): read a
// registry value (or all values of a key when the path ends in "*").
type RegistryCollector struct{}

func (RegistryCollector) Function() string { return "getFromRegistry" }

func (RegistryCollector) Validation() map[string]any {
	return map[string]any{"path": Mandatory}
}

var registryHives = map[string]registry.Key{
	"HKEY_LOCAL_MACHINE":  registry.LOCAL_MACHINE,
	"HKEY_CURRENT_USER":   registry.CURRENT_USER,
	"HKEY_CLASSES_ROOT":   registry.CLASSES_ROOT,
	"HKEY_USERS":          registry.USERS,
	"HKEY_CURRENT_CONFIG": registry.CURRENT_CONFIG,
}

// Results reads the registry path and returns a single record mapping each value
// name to its (binary-as-hex-encoded) value, mirroring Collect/Registry.pm.
func (RegistryCollector) Results(job map[string]any, log *logging.Logger) []map[string]any {
	path := strings.ReplaceAll(str(job["path"]), "\\", "/")
	segments := strings.Split(path, "/")
	if len(segments) < 2 {
		return nil
	}
	hive, ok := registryHives[segments[0]]
	if !ok {
		log.Error("getFromRegistry: path must start with HKEY_*")
		return nil
	}
	valueName := segments[len(segments)-1]
	subkey := strings.Join(segments[1:len(segments)-1], `\`)

	key, err := registry.OpenKey(hive, subkey, registry.QUERY_VALUE)
	if err != nil {
		return nil
	}
	defer key.Close()

	result := map[string]any{}
	if valueName == "*" {
		names, err := key.ReadValueNames(-1)
		if err != nil {
			return nil
		}
		for _, name := range names {
			result[name] = readRegistryValue(key, name)
		}
	} else {
		result[valueName] = readRegistryValue(key, valueName)
	}
	if len(result) == 0 {
		return nil
	}
	return []map[string]any{result}
}

// readRegistryValue reads a value and formats it the way
// _encodeRegistryValueForCollect does: binary/resource types as hex bytes,
// multi-strings joined with commas, numbers as decimal.
func readRegistryValue(key registry.Key, name string) string {
	_, valType, err := key.GetValue(name, nil)
	if err != nil {
		return ""
	}
	switch valType {
	case registry.SZ, registry.EXPAND_SZ:
		s, _, _ := key.GetStringValue(name)
		return s
	case registry.DWORD, registry.QWORD:
		n, _, _ := key.GetIntegerValue(name)
		return strconv.FormatUint(n, 10)
	case registry.MULTI_SZ:
		ss, _, _ := key.GetStringsValue(name)
		return strings.Join(ss, ",")
	default:
		// REG_BINARY (3) / REG_RESOURCE_LIST (8) and friends -> hex bytes.
		b, _, _ := key.GetBinaryValue(name)
		parts := make([]string, len(b))
		for i, by := range b {
			parts[i] = fmt.Sprintf("%02x", by)
		}
		return strings.Join(parts, " ")
	}
}
