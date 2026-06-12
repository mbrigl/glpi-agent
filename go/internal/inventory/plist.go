// SPDX-License-Identifier: GPL-2.0-only

package inventory

import (
	"bytes"
	"encoding/xml"
	"strings"
)

// parsePlist decodes an Apple property-list XML document (as produced by
// `system_profiler -xml`) into nested Go values: a <dict> becomes a
// map[string]any, an <array> a []any, and scalar elements (string/integer/real/
// date/data) their text content; <true/>/<false/> become "true"/"false". It
// returns the value nested directly under <plist>.
func parsePlist(data []byte) (any, error) {
	dec := xml.NewDecoder(bytes.NewReader(data))
	dec.Strict = false
	for {
		tok, err := dec.Token()
		if err != nil {
			return nil, err
		}
		if se, ok := tok.(xml.StartElement); ok && se.Name.Local == "plist" {
			return plistValue(dec)
		}
	}
}

// plistValue reads the next value element from the decoder.
func plistValue(dec *xml.Decoder) (any, error) {
	for {
		tok, err := dec.Token()
		if err != nil {
			return nil, err
		}
		switch t := tok.(type) {
		case xml.StartElement:
			return plistElement(dec, t)
		case xml.EndElement:
			return nil, nil
		}
	}
}

// plistElement parses a started value element by its kind.
func plistElement(dec *xml.Decoder, start xml.StartElement) (any, error) {
	switch start.Name.Local {
	case "dict":
		return plistDict(dec)
	case "array":
		return plistArray(dec)
	case "true", "false":
		plistText(dec)
		return start.Name.Local, nil
	default:
		// string / integer / real / date / data: take the text content.
		return plistText(dec), nil
	}
}

// plistDict reads a <dict> of alternating <key> + value elements.
func plistDict(dec *xml.Decoder) (map[string]any, error) {
	out := map[string]any{}
	var key string
	for {
		tok, err := dec.Token()
		if err != nil {
			return nil, err
		}
		switch t := tok.(type) {
		case xml.StartElement:
			if t.Name.Local == "key" {
				key = plistText(dec)
				continue
			}
			v, err := plistElement(dec, t)
			if err != nil {
				return nil, err
			}
			out[key] = v
		case xml.EndElement:
			return out, nil
		}
	}
}

// plistArray reads an <array> of value elements.
func plistArray(dec *xml.Decoder) ([]any, error) {
	var out []any
	for {
		tok, err := dec.Token()
		if err != nil {
			return nil, err
		}
		switch t := tok.(type) {
		case xml.StartElement:
			v, err := plistElement(dec, t)
			if err != nil {
				return nil, err
			}
			out = append(out, v)
		case xml.EndElement:
			return out, nil
		}
	}
}

// plistText accumulates character data until the current element closes.
func plistText(dec *xml.Decoder) string {
	var sb strings.Builder
	for {
		tok, err := dec.Token()
		if err != nil {
			return sb.String()
		}
		switch t := tok.(type) {
		case xml.CharData:
			sb.Write(t)
		case xml.EndElement:
			return strings.TrimSpace(sb.String())
		}
	}
}

// plistDictArray returns the named dict (default "_items") array from the first
// dict directly under <plist>, mirroring Tools/MacOS.pm _getDict.
func plistDictArray(root any, key string) []any {
	if key == "" {
		key = "_items"
	}
	arr, ok := root.([]any)
	if !ok || len(arr) == 0 {
		return nil
	}
	first, ok := arr[0].(map[string]any)
	if !ok {
		return nil
	}
	items, _ := first[key].([]any)
	return items
}
