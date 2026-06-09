// SPDX-License-Identifier: GPL-2.0-only

package discovery

import (
	"bufio"
	"regexp"
	"strconv"
	"strings"
)

// walk_test.go provides a loader for the upstream `snmpwalk -On` capture files
// (resources/walks/*.walk), so the SNMP/MibSupport code can be exercised against
// the same real device dumps the Perl agent is tested on. parseWalk renders each
// value the same way the live gosnmp layer does (pduString), and walkGetter
// serves the flat OID map through the SNMPGetter interface exactly like the real
// client (Get = exact match, Walk = subtree by suffix).

// walkGetter is an SNMPGetter backed by a flat OID->value map parsed from a
// `.walk` fixture.
type walkGetter struct {
	values map[string]string
}

func (w *walkGetter) Get(oids []string) (map[string]string, error) {
	out := map[string]string{}
	for _, oid := range oids {
		if v, ok := w.values[strings.TrimPrefix(oid, ".")]; ok {
			out[oid] = v
		}
	}
	return out, nil
}

func (w *walkGetter) Walk(base string) (map[string]string, error) {
	base = strings.TrimPrefix(base, ".")
	prefix := base + "."
	out := map[string]string{}
	for oid, v := range w.values {
		if oid == base {
			out[""] = v
		} else if strings.HasPrefix(oid, prefix) {
			out[strings.TrimPrefix(oid, prefix)] = v
		}
	}
	return out, nil
}

func (w *walkGetter) Close() error { return nil }

// parseWalk parses `snmpwalk -On` output into a flat OID->value map. OIDs are
// normalised to the dotless numeric form (the `iso` alias becomes `1`); values
// are converted to the same string form the live SNMP layer produces.
func parseWalk(content string) map[string]string {
	out := map[string]string{}
	sc := bufio.NewScanner(strings.NewReader(content))
	sc.Buffer(make([]byte, 0, 64*1024), 1024*1024) // some rows are long
	for sc.Scan() {
		line := sc.Text()
		eq := strings.Index(line, " = ")
		if eq < 0 {
			continue
		}
		out[normalizeWalkOID(line[:eq])] = convertWalkValue(line[eq+3:])
	}
	return out
}

func normalizeWalkOID(s string) string {
	s = strings.TrimSpace(s)
	s = strings.TrimPrefix(s, ".")
	switch {
	case s == "iso":
		return "1"
	case strings.HasPrefix(s, "iso."):
		return "1." + s[len("iso."):]
	}
	return s
}

var intEnumRE = regexp.MustCompile(`\((-?\d+)\)\s*$`)

// convertWalkValue renders a `TYPE: value` token the way pduString renders a
// live gosnmp PDU.
func convertWalkValue(rest string) string {
	rest = strings.TrimSpace(rest)
	typ, val := "", rest
	if i := strings.Index(rest, ":"); i >= 0 {
		typ = rest[:i]
		val = strings.TrimSpace(rest[i+1:])
	}
	switch typ {
	case "STRING":
		return strings.Trim(val, `"`)
	case "Hex-STRING":
		b := parseHexBytes(val)
		if isPrintable(b) {
			return strings.TrimRight(string(b), "\x00")
		}
		return hexColon(b)
	case "Timeticks":
		if m := regexp.MustCompile(`\((\d+)\)`).FindStringSubmatch(val); m != nil {
			return m[1]
		}
		return val
	case "INTEGER", "Gauge32", "Gauge64", "Counter32", "Counter64":
		// Strip an enum annotation like "up(1)" down to the number.
		if m := intEnumRE.FindStringSubmatch(val); m != nil {
			return m[1]
		}
		return val
	default: // OID, IpAddress, or a bare value
		return val
	}
}

// parseHexBytes parses space-separated hex pairs ("00 1B 44") into bytes.
func parseHexBytes(s string) []byte {
	fields := strings.Fields(s)
	b := make([]byte, 0, len(fields))
	for _, f := range fields {
		n, err := strconv.ParseUint(f, 16, 8)
		if err != nil {
			return b
		}
		b = append(b, byte(n))
	}
	return b
}
