// SPDX-License-Identifier: GPL-2.0-only

package inventory

import "strings"

// BuildEnvs maps environment variables to the ENVS section (KEY/VAL), mirroring
// Generic/Environment.pm.
func BuildEnvs(environ []string) []map[string]any {
	var envs []map[string]any
	for _, kv := range environ {
		key, val, ok := strings.Cut(kv, "=")
		if !ok || key == "" {
			continue
		}
		envs = append(envs, map[string]any{"KEY": key, "VAL": val})
	}
	return envs
}
