// SPDX-License-Identifier: GPL-2.0-only

package inventory

import (
	"regexp"
	"strings"
)

// canonicalManufacturerMap holds the exact-match CPU-vendor normalisations from
// Tools.pm getCanonicalManufacturer.
var canonicalManufacturerMap = map[string]string{
	"GenuineIntel": "Intel",
	"AuthenticAMD": "AMD",
	"TMx86":        "Transmeta",
	"TransmetaCPU": "Transmeta",
	"CyrixInstead": "Cyrix",
	"CentaurHauls": "VIA",
	"HygonGenuine": "Hygon",
}

// canonicalManufacturerWordRE matches a known brand word anywhere in the string.
var canonicalManufacturerWordRE = regexp.MustCompile(`(?i)(\blg\b|broadcom|compaq|dell|epson|fujitsu|hitachi|ibm|intel|kingston|matshita|maxtor|nvidia|\bnec\b|pioneer|samsung|sony|supermicro|toshiba|transcend)`)

// canonicalManufacturerPrefixes are the prefix patterns, in a fixed evaluation
// order (the upstream uses hash order; a single match is the norm).
var canonicalManufacturerPrefixes = []struct {
	name string
	re   *regexp.Regexp
}{
	{"Apple", regexp.MustCompile(`(?i)^APPLE`)},
	{"Hewlett-Packard", regexp.MustCompile(`^(hp|HPE?|(?i:hewlett[ -]packard)|MM)`)},
	{"Hitachi", regexp.MustCompile(`^(HD|IC|HU|HGST)`)},
	{"Seagate", regexp.MustCompile(`^(ST|(?i:seagate))`)},
	{"Sony", regexp.MustCompile(`(?i)^OPTIARC`)},
	{"Western Digital", regexp.MustCompile(`^(WDC?|(?i:western))`)},
	{"Crucial", regexp.MustCompile(`^CT`)},
	{"PNY", regexp.MustCompile(`^PNY`)},
}

// getCanonicalManufacturer normalises a device/vendor string to a canonical
// brand, mirroring Tools.pm getCanonicalManufacturer: exact CPU-vendor map, then
// a known-brand word match (returned title-cased), then a prefix match.
func getCanonicalManufacturer(s string) string {
	if s == "" {
		return ""
	}
	if v, ok := canonicalManufacturerMap[s]; ok {
		return v
	}
	if m := canonicalManufacturerWordRE.FindStringSubmatch(s); m != nil {
		return ucfirstLower(m[1])
	}
	for _, p := range canonicalManufacturerPrefixes {
		if p.re.MatchString(s) {
			return p.name
		}
	}
	return s
}

// ucfirstLower lower-cases a word and upper-cases its first letter (Perl
// ucfirst(lc(...))).
func ucfirstLower(s string) string {
	s = strings.ToLower(s)
	if s == "" {
		return ""
	}
	return strings.ToUpper(s[:1]) + s[1:]
}
