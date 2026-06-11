// SPDX-License-Identifier: GPL-2.0-only

package inventory

import (
	"bytes"
	"regexp"
	"sort"
	"strings"
)

// adobeCipher is the per-position substitution table used to decode an Adobe
// serial number (Tools/License.pm _decodeAdobeKey, after Brandon Mulcahy).
var adobeCipher = []string{
	"0000000001", "5038647192", "1456053789", "2604371895",
	"4753896210", "8145962073", "0319728564", "7901235846",
	"7901235846", "0319728564", "8145962073", "4753896210",
	"2604371895", "1426053789", "5038647192", "3267408951",
	"5038647192", "2604371895", "8145962073", "7901235846",
	"3267408951", "1426053789", "4753896210", "0319728564",
}

// decodeAdobeKey decodes a 24-digit Adobe serial into the
// "NNNN-NNNN-NNNN-NNNN-NNNN-NNNN" form, mirroring Tools/License.pm
// _decodeAdobeKey: digit i selects a character from the i-th cipher row.
func decodeAdobeKey(encrypted string) string {
	if encrypted == "" {
		return ""
	}
	out := make([]byte, 0, len(encrypted))
	for i := 0; i < len(encrypted) && i < len(adobeCipher); i++ {
		d := encrypted[i] - '0'
		if d > 9 {
			return ""
		}
		out = append(out, adobeCipher[i][d])
	}
	if len(out) != 24 {
		return ""
	}
	var sb strings.Builder
	for i := 0; i < 24; i += 4 {
		if i > 0 {
			sb.WriteByte('-')
		}
		sb.Write(out[i : i+4])
	}
	return sb.String()
}

var adobeFLMapRE = regexp.MustCompile(`1([a-zA-Z0-9.\-]+)[{|}\[a-zA-Z0-9_\-]*]?FLMap([a-zA-Z0-9.\-]{3,}).{2,3}`)

// parseAdobeLicenses extracts Adobe licenses from a (binary) Adobe PCD cache.db,
// mirroring Tools/License.pm getAdobeLicensesWithoutSqlite: an FLMap pass groups
// components under their product, then each product's 24-digit serial (SN) and
// EpicAppName are recovered and the serial decoded. Returns NAME/FULLNAME/KEY/
// COMPONENTS entries.
func parseAdobeLicenses(content []byte) []map[string]any {
	content = bytes.ReplaceAll(content, []byte{0}, nil)
	// Map each byte to a distinct rune (Latin-1) so the regex "." consumes
	// exactly one original byte, matching Perl's byte semantics on this binary
	// content rather than Go's default multi-byte UTF-8 interpretation.
	full := bytesToLatin1(content)

	// FLMap pass: collect components per product, destructively as upstream does.
	products := map[string][]string{}
	var order []string
	work := full
	for {
		loc := adobeFLMapRE.FindStringSubmatchIndex(work)
		if loc == nil {
			break
		}
		component := work[loc[2]:loc[3]]
		product := work[loc[4]:loc[5]]
		if _, ok := products[product]; !ok {
			order = append(order, product)
		}
		if !containsString(products[product], component) {
			products[product] = append(products[product], component)
		}
		work = work[:loc[0]] + work[loc[1]:]
	}

	sort.Strings(order)
	var licenses []map[string]any
	for _, product := range order {
		snRE, err := regexp.Compile(product + `\{\|\}[a-zA-Z0-9\-_]+SN([0-9]{24})`)
		if err != nil {
			continue
		}
		m := snRE.FindStringSubmatch(full)
		if m == nil {
			continue
		}
		key := decodeAdobeKey(m[1])

		fullName := product
		if nameRE, err := regexp.Compile(product + `ALM_LicInfo_EpicAppName\{\|\}[0-9]+([a-zA-Z]+[a-zA-Z0-9.\- ]+).{2}`); err == nil {
			if nm := nameRE.FindStringSubmatch(full); nm != nil && nm[1] != "" {
				fullName = nm[1]
			}
		}

		components := append([]string(nil), products[product]...)
		sort.Strings(components)
		licenses = append(licenses, map[string]any{
			"NAME":       product,
			"FULLNAME":   fullName,
			"KEY":        key,
			"COMPONENTS": strings.Join(components, "/"),
		})
	}
	return licenses
}

// bytesToLatin1 renders each byte as the rune of the same value, so a regex "."
// matches one byte and capture offsets stay byte-aligned for binary input.
func bytesToLatin1(b []byte) string {
	r := make([]rune, len(b))
	for i, c := range b {
		r[i] = rune(c)
	}
	return string(r)
}

func containsString(list []string, s string) bool {
	for _, e := range list {
		if e == s {
			return true
		}
	}
	return false
}
