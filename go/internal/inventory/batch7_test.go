// SPDX-License-Identifier: GPL-2.0-only

package inventory

import (
	"strings"
	"testing"
)

func TestBuildLoggedUsers(t *testing.T) {
	const who = "alice    pts/0        2026-06-07 09:12   .          1234 (10.0.0.5)\nroot     tty1         2026-06-07 08:00    old         987\n"
	users := BuildLoggedUsers(who)
	if len(users) != 2 || users[0]["LOGIN"] != "alice" || users[1]["LOGIN"] != "root" {
		t.Errorf("logged users = %v", users)
	}
}

func TestParsePrintersConf(t *testing.T) {
	const conf = `# CUPS printers.conf
<DefaultPrinter Office>
Info Office laser on floor 3
MakeModel HP LaserJet Series PCL 6 CUPS
DeviceURI socket://192.168.1.5:9100
State Idle
</Printer>
<Printer Label>
Info Label printer
DeviceURI usb://Zebra/GK420
</Printer>
`
	printers := ParsePrintersConf(strings.NewReader(conf))
	if len(printers) != 2 {
		t.Fatalf("printers = %d, want 2", len(printers))
	}
	office := printers[0]
	if office["NAME"] != "Office" || office["PORT"] != "socket://192.168.1.5:9100" {
		t.Errorf("office = %v", office)
	}
	if office["DESCRIPTION"] != "Office laser on floor 3" || office["DRIVER"] != "HP LaserJet Series PCL 6 CUPS" {
		t.Errorf("office detail = %v", office)
	}
	if printers[1]["NAME"] != "Label" || printers[1]["PORT"] != "usb://Zebra/GK420" {
		t.Errorf("label = %v", printers[1])
	}
}

func TestParseDefenderHealth(t *testing.T) {
	const healthy = `{"healthy":true,"appVersion":"101.98.30","definitionsVersion":"1.391.2024",` +
		`"definitionsStatus":{"$type":"upToDate"},"realTimeProtectionEnabled":{"value":true}}`
	av := ParseDefenderHealth([]byte(healthy))
	if av == nil {
		t.Fatal("expected an antivirus entry for a healthy Defender")
	}
	if av["NAME"] != "Microsoft Defender" || av["COMPANY"] != "Microsoft" {
		t.Errorf("av = %v", av)
	}
	if av["ENABLED"] != 1 || av["UPTODATE"] != 1 {
		t.Errorf("av flags = %v", av)
	}
	if av["VERSION"] != "101.98.30" || av["BASE_VERSION"] != "1.391.2024" {
		t.Errorf("av versions = %v", av)
	}

	// Not healthy -> no entry.
	if ParseDefenderHealth([]byte(`{"healthy":false}`)) != nil {
		t.Error("unhealthy Defender should yield no entry")
	}
}

func TestParseTeamViewerInfo(t *testing.T) {
	const info = "TeamViewer ID:            \x1b[0m123456789\nVersion: 15.0\n"
	rm := ParseTeamViewerInfo(info)
	if rm == nil || rm["ID"] != "123456789" || rm["TYPE"] != "teamviewer" {
		t.Errorf("remote mgmt = %v", rm)
	}
	if ParseTeamViewerInfo("no id here") != nil {
		t.Error("expected nil when no TeamViewer ID present")
	}
}
