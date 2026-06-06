// SPDX-License-Identifier: GPL-2.0-only

package discovery

import "testing"

// TestClassifyBySysObjectID exercises the full / partial / manufacturer-only
// matching of Hardware.pm::_getSysObjectIDInfo against the embedded database.
func TestClassifyBySysObjectID(t *testing.T) {
	// Full match (9.1.282 -> Cisco / NETWORKING / Catalyst 6506).
	if c, ok := classifyBySysObjectID(".1.3.6.1.4.1.9.1.282"); !ok ||
		c.Manufacturer != "Cisco" || c.Type != "NETWORKING" || c.Model != "Catalyst 6506" {
		t.Errorf("full match = %+v (ok=%v)", c, ok)
	}

	// Partial match: an unknown trailing component is stripped back to 9.1.282.
	if c, ok := classifyBySysObjectID(".1.3.6.1.4.1.9.1.282.7"); !ok || c.Model != "Catalyst 6506" {
		t.Errorf("partial match = %+v (ok=%v)", c, ok)
	}

	// Manufacturer-only fallback: unknown device id under enterprise 9.
	if c, ok := classifyBySysObjectID(".1.3.6.1.4.1.9.987654"); !ok ||
		c.Manufacturer != "Cisco" || c.Model != "" {
		t.Errorf("manufacturer-only = %+v (ok=%v)", c, ok)
	}

	// Unknown enterprise -> no classification.
	if _, ok := classifyBySysObjectID(".1.3.6.1.4.1.99999999.1"); ok {
		t.Error("expected no match for unknown enterprise id")
	}

	// Non-enterprise OID -> not parseable.
	if _, ok := classifyBySysObjectID(".1.3.6.1.2.1.1.2.0"); ok {
		t.Error("expected no match for a non-enterprise sysObjectID")
	}
}

// TestParseEnterprise checks the prefix stripping and id split.
func TestParseEnterprise(t *testing.T) {
	mid, did, ok := parseEnterprise(".1.3.6.1.4.1.9.1.282")
	if !ok || mid != "9" || did != "1.282" {
		t.Errorf("parseEnterprise = (%q, %q, %v)", mid, did, ok)
	}
	if mid, did, ok := parseEnterprise(".1.3.6.1.4.1.2021"); !ok || mid != "2021" || did != "" {
		t.Errorf("manufacturer-only parse = (%q, %q, %v)", mid, did, ok)
	}
}
