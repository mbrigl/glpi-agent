// SPDX-License-Identifier: GPL-2.0-only

package discovery

import "testing"

// TestParseProcNetArp checks the /proc/net/arp parse: complete entries are kept,
// incomplete (flags 0x0 / all-zero MAC) ones are dropped.
func TestParseProcNetArp(t *testing.T) {
	const content = `IP address       HW type     Flags       HW address            Mask     Device
192.0.2.1        0x1         0x2         aa:bb:cc:dd:ee:ff     *        eth0
192.0.2.2        0x1         0x0         00:00:00:00:00:00     *        eth0
192.0.2.3        0x1         0x2         11:22:33:44:55:66     *        eth0
`
	table := ParseProcNetArp(content)
	if len(table) != 2 {
		t.Fatalf("got %d entries, want 2: %v", len(table), table)
	}
	if table["192.0.2.1"] != "aa:bb:cc:dd:ee:ff" {
		t.Errorf("192.0.2.1 -> %q", table["192.0.2.1"])
	}
	if _, ok := table["192.0.2.2"]; ok {
		t.Error("incomplete entry 192.0.2.2 should be skipped")
	}
}

// TestParseArpCommand covers the BSD `arp`, `ip neighbor` and Windows `arp -a`
// output forms.
func TestParseArpCommand(t *testing.T) {
	cases := []struct {
		name, out, mac, host string
	}{
		{"bsd", "router (192.0.2.1) at 0:1b:2c:3d:4e:5f on en0", "0:1b:2c:3d:4e:5f", "router"},
		{"bsd-unknown", "? (192.0.2.1) at aa:bb:cc:dd:ee:ff on en0", "aa:bb:cc:dd:ee:ff", ""},
		{"neigh", "192.0.2.1 dev eth0 lladdr aa:bb:cc:dd:ee:ff REACHABLE", "aa:bb:cc:dd:ee:ff", ""},
		{"win", "  192.0.2.1           aa-bb-cc-dd-ee-ff     dynamic", "aa:bb:cc:dd:ee:ff", ""},
	}
	for _, c := range cases {
		t.Run(c.name, func(t *testing.T) {
			mac, host := ParseArpCommand(c.out)
			if mac != c.mac {
				t.Errorf("mac = %q, want %q", mac, c.mac)
			}
			if host != c.host {
				t.Errorf("host = %q, want %q", host, c.host)
			}
		})
	}
}

// TestBuildNBStatRequest checks the header and trailer of the NBSTAT query.
func TestBuildNBStatRequest(t *testing.T) {
	pkt := BuildNBStatRequest(0x1234)
	if pkt[0] != 0x12 || pkt[1] != 0x34 {
		t.Errorf("txid = %02x%02x, want 1234", pkt[0], pkt[1])
	}
	if pkt[5] != 0x01 {
		t.Errorf("qdcount = %d, want 1", pkt[5])
	}
	// Header(12) + length(1) + 32 encoded bytes + terminator(1) + qtype(2) + qclass(2).
	if len(pkt) != 12+1+32+1+2+2 {
		t.Fatalf("packet length = %d", len(pkt))
	}
	qtype := uint16(pkt[len(pkt)-4])<<8 | uint16(pkt[len(pkt)-3])
	if qtype != 0x0021 {
		t.Errorf("qtype = %#04x, want 0x0021 (NBSTAT)", qtype)
	}
}

// TestParseNBStatResponse builds a synthetic NBSTAT reply and verifies the
// netbios name, workgroup and adapter MAC are extracted.
func TestParseNBStatResponse(t *testing.T) {
	var pkt []byte
	// Header: txid, flags(response), qdcount=1, ancount=1.
	pkt = append(pkt, 0x12, 0x34, 0x84, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00)
	// Question name: a 32-byte encoded label + terminator, then qtype + qclass.
	pkt = append(pkt, 0x20)
	for i := 0; i < 32; i++ {
		pkt = append(pkt, 'A')
	}
	pkt = append(pkt, 0x00)
	pkt = append(pkt, 0x00, 0x21, 0x00, 0x01) // qtype NBSTAT, qclass IN
	// Answer name: a compression pointer (skipNBName handles it).
	pkt = append(pkt, 0xC0, 0x0C)
	// type NBSTAT, class IN, ttl, rdlength (rdlength value unused by the parser).
	pkt = append(pkt, 0x00, 0x21, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00)
	// Name table: 2 names.
	pkt = append(pkt, 0x02)
	pkt = append(pkt, nbName("WORKSTATION", 0x00, 0x0400)...) // UNIQUE -> netbios name
	pkt = append(pkt, nbName("MYGROUP", 0x00, 0x8400)...)     // GROUP  -> workgroup
	// Adapter MAC.
	pkt = append(pkt, 0xde, 0xad, 0xbe, 0xef, 0x00, 0x01)

	h, err := ParseNBStatResponse(pkt)
	if err != nil {
		t.Fatal(err)
	}
	if h.NetbiosName != "WORKSTATION" {
		t.Errorf("netbios name = %q", h.NetbiosName)
	}
	if h.Workgroup != "MYGROUP" {
		t.Errorf("workgroup = %q", h.Workgroup)
	}
	if h.MAC != "de:ad:be:ef:00:01" {
		t.Errorf("mac = %q", h.MAC)
	}
}

// nbName builds a 18-byte name-table entry: 15-byte padded name, 1 suffix byte,
// 2 flag bytes.
func nbName(name string, suffix byte, flags uint16) []byte {
	b := make([]byte, 18)
	copy(b[:15], name)
	for i := len(name); i < 15; i++ {
		b[i] = ' '
	}
	b[15] = suffix
	b[16] = byte(flags >> 8)
	b[17] = byte(flags)
	return b
}
