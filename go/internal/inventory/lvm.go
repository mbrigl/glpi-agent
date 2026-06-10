// SPDX-License-Identifier: GPL-2.0-only

package inventory

import (
	"bufio"
	"strconv"
	"strings"
)

// LVM sections, derived from Linux/LVM.pm. The collectors run
// `lvs/pvs/vgs --noheading --nosuffix --units M -o <columns>`; with leading
// whitespace stripped, strings.Fields index i equals the Perl $infos[i+1].

// ParseLVS builds LOGICAL_VOLUMES from `lvs -a -o lv_name,vg_uuid,lv_attr,
// lv_size,lv_uuid,seg_count`.
func ParseLVS(out string) []map[string]any {
	var volumes []map[string]any
	eachFields(out, func(f []string) {
		if len(f) < 6 {
			return
		}
		v := map[string]any{
			"LV_NAME":   f[0],
			"VG_UUID":   f[1],
			"ATTR":      f[2],
			"SIZE":      truncMB(f[3]),
			"LV_UUID":   f[4],
			"SEG_COUNT": f[5],
		}
		volumes = append(volumes, v)
	})
	return volumes
}

// ParsePVS builds PHYSICAL_VOLUMES from `pvs -o pv_name,pv_fmt,pv_attr,pv_size,
// pv_free,pv_uuid,pv_pe_count,vg_uuid`.
func ParsePVS(out string) []map[string]any {
	var volumes []map[string]any
	eachFields(out, func(f []string) {
		// vg_uuid (the last column) is empty for a PV not assigned to any volume
		// group, so the row may carry only 7 fields.
		if len(f) < 7 {
			return
		}
		size := truncMB(f[3])
		peCount, _ := strconv.Atoi(f[6])
		v := map[string]any{
			"DEVICE":      f[0],
			"FORMAT":      f[1],
			"ATTR":        f[2],
			"SIZE":        size,
			"FREE":        truncMB(f[4]),
			"PV_UUID":     f[5],
			"PV_PE_COUNT": f[6],
		}
		if len(f) >= 8 {
			v["VG_UUID"] = f[7]
		}
		if peCount > 0 {
			v["PE_SIZE"] = size / peCount
		}
		volumes = append(volumes, v)
	})
	return volumes
}

// ParseVGS builds VOLUME_GROUPS from `vgs -o vg_name,pv_count,lv_count,vg_attr,
// vg_size,vg_free,vg_uuid,vg_extent_size`.
func ParseVGS(out string) []map[string]any {
	var groups []map[string]any
	eachFields(out, func(f []string) {
		if len(f) < 8 {
			return
		}
		g := map[string]any{
			"VG_NAME":        f[0],
			"PV_COUNT":       f[1],
			"LV_COUNT":       f[2],
			"ATTR":           f[3],
			"SIZE":           truncMB(f[4]),
			"FREE":           truncMB(f[5]),
			"VG_UUID":        f[6],
			"VG_EXTENT_SIZE": f[7],
		}
		groups = append(groups, g)
	})
	return groups
}

// eachFields calls fn with the whitespace-split fields of each non-empty line.
func eachFields(out string, fn func([]string)) {
	scanner := bufio.NewScanner(strings.NewReader(out))
	for scanner.Scan() {
		if f := strings.Fields(scanner.Text()); len(f) > 0 {
			fn(f)
		}
	}
}

// truncMB parses an LVM "--units M --nosuffix" size (a float like "1024.00") and
// truncates it to an int, mirroring the Perl int($size||0).
func truncMB(s string) int {
	f, err := strconv.ParseFloat(s, 64)
	if err != nil {
		return 0
	}
	return int(f)
}
