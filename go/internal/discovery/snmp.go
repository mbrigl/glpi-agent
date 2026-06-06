// SPDX-License-Identifier: GPL-2.0-only

// Package discovery implements NetDiscovery and NetInventory.
//
// Derived from the upstream Perl modules lib/GLPI/Agent/Task/NetDiscovery.pm and
// lib/GLPI/Agent/SNMP/Device.pm: the generic device properties come from the
// system-MIB OIDs (sysDescr/sysName/sysLocation/sysContact/sysUpTime), and a
// device is only reported when it answers SNMP (a sysDescr is present). The
// transport uses gosnmp instead of the Perl snmp2/Net-SNMP stack.
//
// The sysObjectID-based device classification (TYPE/MANUFACTURER/MODEL) and the
// vendor MibSupport tail are follow-on work; see docs/UPSTREAM-MAPPING.md.
package discovery

import (
	"fmt"
	"strings"
	"time"

	"github.com/gosnmp/gosnmp"
)

// System-MIB OIDs used for the generic device properties (SNMP/Device.pm).
const (
	oidSysDescr    = "1.3.6.1.2.1.1.1.0"
	oidSysObjectID = "1.3.6.1.2.1.1.2.0"
	oidSysUpTime   = "1.3.6.1.2.1.1.3.0"
	oidSysContact  = "1.3.6.1.2.1.1.4.0"
	oidSysName     = "1.3.6.1.2.1.1.5.0"
	oidSysLocation = "1.3.6.1.2.1.1.6.0"
)

// genericOIDs is the order of OIDs fetched to populate a device.
var genericOIDs = []string{
	oidSysDescr, oidSysObjectID, oidSysUpTime, oidSysContact, oidSysName, oidSysLocation,
}

// SNMPGetter fetches OID values from one host. It is the seam between the
// gosnmp-backed client and the tests' fake responder.
type SNMPGetter interface {
	Get(oids []string) (map[string]string, error)
	// Walk returns every value under a base OID, keyed by the index suffix that
	// follows the base (mirrors the Perl SNMP walk used to build tables).
	Walk(base string) (map[string]string, error)
	Close() error
}

// Credential is an SNMP credential, mirroring the subset of the GLPI credentials
// NetDiscovery uses (v1/v2c community; v3 is follow-on).
type Credential struct {
	ID        int
	Version   string // "1" or "2c"
	Community string
}

// gosnmpClient is the gosnmp-backed SNMPGetter.
type gosnmpClient struct {
	snmp *gosnmp.GoSNMP
}

// Dial opens an SNMP session to host:port with the given credential, mirroring
// the connection setup in NetDiscovery.
func Dial(host string, port uint16, cred Credential, timeout time.Duration) (SNMPGetter, error) {
	version := gosnmp.Version2c
	if cred.Version == "1" {
		version = gosnmp.Version1
	}
	community := cred.Community
	if community == "" {
		community = "public"
	}
	client := &gosnmp.GoSNMP{
		Target:    host,
		Port:      port,
		Community: community,
		Version:   version,
		Timeout:   timeout,
		Retries:   0, // snmp-retries default is 0 (Config.pm)
	}
	if err := client.Connect(); err != nil {
		return nil, fmt.Errorf("snmp connect to %s failed: %w", host, err)
	}
	return &gosnmpClient{snmp: client}, nil
}

func (c *gosnmpClient) Get(oids []string) (map[string]string, error) {
	// gosnmp expects OIDs with a leading dot.
	dotted := make([]string, len(oids))
	for i, oid := range oids {
		dotted[i] = "." + strings.TrimPrefix(oid, ".")
	}
	packet, err := c.snmp.Get(dotted)
	if err != nil {
		return nil, err
	}
	out := make(map[string]string, len(packet.Variables))
	for _, v := range packet.Variables {
		if v.Type == gosnmp.NoSuchObject || v.Type == gosnmp.NoSuchInstance || v.Type == gosnmp.Null {
			continue
		}
		out[strings.TrimPrefix(v.Name, ".")] = pduString(v)
	}
	return out, nil
}

// Walk returns every value under base, keyed by the index suffix following base.
// It uses GETBULK on v2c and GETNEXT on v1, mirroring the Perl walk().
func (c *gosnmpClient) Walk(base string) (map[string]string, error) {
	root := "." + strings.TrimPrefix(base, ".")
	prefix := strings.TrimPrefix(base, ".") + "."
	out := map[string]string{}
	collect := func(pdu gosnmp.SnmpPDU) error {
		if pdu.Type == gosnmp.NoSuchObject || pdu.Type == gosnmp.NoSuchInstance || pdu.Type == gosnmp.Null {
			return nil
		}
		name := strings.TrimPrefix(pdu.Name, ".")
		out[strings.TrimPrefix(name, prefix)] = pduString(pdu)
		return nil
	}
	var err error
	if c.snmp.Version == gosnmp.Version1 {
		err = c.snmp.Walk(root, collect)
	} else {
		err = c.snmp.BulkWalk(root, collect)
	}
	if err != nil {
		return nil, err
	}
	return out, nil
}

func (c *gosnmpClient) Close() error { return c.snmp.Conn.Close() }

// pduString renders an SNMP value as the Perl SNMP layer would present it: octet
// strings as text, binary octet strings (e.g. ifPhysAddress) as colon-separated
// hex, and other types as their printed form.
func pduString(v gosnmp.SnmpPDU) string {
	switch val := v.Value.(type) {
	case []byte:
		if isPrintable(val) {
			return strings.TrimRight(string(val), "\x00")
		}
		return hexColon(val)
	case string:
		return val
	default:
		return fmt.Sprintf("%v", val)
	}
}

// isPrintable reports whether the bytes are all printable ASCII (plus common
// whitespace), so they can be shown as text rather than hex.
func isPrintable(b []byte) bool {
	for _, c := range b {
		if c == 0 || c == '\t' || c == '\n' || c == '\r' {
			continue
		}
		if c < 0x20 || c > 0x7e {
			return false
		}
	}
	return true
}

// hexColon formats bytes as uppercase colon-separated hex (MAC style).
func hexColon(b []byte) string {
	parts := make([]string, len(b))
	for i, c := range b {
		parts[i] = fmt.Sprintf("%02x", c)
	}
	return strings.Join(parts, ":")
}
