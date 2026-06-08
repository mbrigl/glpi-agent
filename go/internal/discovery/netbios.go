// SPDX-License-Identifier: GPL-2.0-only

package discovery

import (
	"fmt"
	"strings"
)

// BuildNBStatRequest builds a NetBIOS adapter-status (NBSTAT) query for the
// wildcard name "*", the request _scanAddressByNetbios sends to UDP/137.
func BuildNBStatRequest(txid uint16) []byte {
	pkt := []byte{
		byte(txid >> 8), byte(txid), // transaction id
		0x00, 0x00, // flags
		0x00, 0x01, // qdcount = 1
		0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // an/ns/ar count
	}
	// Question name: "*" padded to 16 bytes, first-level encoded.
	name := make([]byte, 16)
	name[0] = '*'
	pkt = append(pkt, 0x20) // length of the encoded label (32)
	for _, b := range name {
		pkt = append(pkt, 'A'+(b>>4), 'A'+(b&0x0f))
	}
	pkt = append(pkt, 0x00)       // end of name
	pkt = append(pkt, 0x00, 0x21) // qtype = NBSTAT
	pkt = append(pkt, 0x00, 0x01) // qclass = IN
	return pkt
}

// ParseNBStatResponse parses a NBSTAT response into the NetBIOS name, workgroup
// and adapter MAC, mirroring the suffix/flags logic of _scanAddressByNetbios
// (suffix 0x00 GROUP -> workgroup, suffix 0x00 UNIQUE -> netbios name).
func ParseNBStatResponse(data []byte) (HostInfo, error) {
	var h HostInfo
	if len(data) < 12 {
		return h, fmt.Errorf("short NBSTAT response")
	}
	pos := 12
	pos = skipNBName(data, pos) // question name
	pos += 4                    // qtype + qclass
	pos = skipNBName(data, pos) // answer name
	pos += 2 + 2 + 4 + 2        // type, class, ttl, rdlength
	if pos >= len(data) {
		return h, fmt.Errorf("malformed NBSTAT response")
	}

	numNames := int(data[pos])
	pos++
	for i := 0; i < numNames; i++ {
		if pos+18 > len(data) {
			return h, fmt.Errorf("truncated NBSTAT name table")
		}
		name := strings.TrimRight(string(data[pos:pos+15]), " \x00")
		suffix := data[pos+15]
		flags := uint16(data[pos+16])<<8 | uint16(data[pos+17])
		group := flags&0x8000 != 0
		pos += 18

		switch {
		case suffix == 0x00 && group && h.Workgroup == "":
			h.Workgroup = name
		case suffix == 0x00 && !group && h.NetbiosName == "":
			h.NetbiosName = name
		}
	}
	// The 6-byte adapter address follows the name table.
	if pos+6 <= len(data) {
		mac := data[pos : pos+6]
		if !allZero(mac) {
			h.MAC = hexColon(mac)
		}
	}
	return h, nil
}

// skipNBName advances past a NetBIOS name (a compression pointer or a label
// sequence terminated by a zero length byte).
func skipNBName(data []byte, pos int) int {
	for pos < len(data) {
		b := data[pos]
		if b&0xC0 == 0xC0 { // compression pointer
			return pos + 2
		}
		if b == 0x00 {
			return pos + 1
		}
		pos += int(b) + 1
	}
	return pos
}

func allZero(b []byte) bool {
	for _, c := range b {
		if c != 0 {
			return false
		}
	}
	return true
}
