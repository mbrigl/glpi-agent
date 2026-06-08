// SPDX-License-Identifier: GPL-2.0-only

package discovery

import (
	"testing"

	"github.com/gosnmp/gosnmp"
)

func TestConfigureV3AuthPriv(t *testing.T) {
	client := &gosnmp.GoSNMP{}
	cred := Credential{
		Version: "3", Username: "monitor",
		AuthProtocol: "sha256", AuthPassword: "authpass",
		PrivProtocol: "aes256", PrivPassword: "privpass",
	}
	if err := configureV3(client, cred); err != nil {
		t.Fatal(err)
	}
	if client.Version != gosnmp.Version3 || client.MsgFlags != gosnmp.AuthPriv {
		t.Errorf("version/flags = %v/%v", client.Version, client.MsgFlags)
	}
	usm := client.SecurityParameters.(*gosnmp.UsmSecurityParameters)
	if usm.UserName != "monitor" || usm.AuthenticationProtocol != gosnmp.SHA256 || usm.PrivacyProtocol != gosnmp.AES256 {
		t.Errorf("usm = %+v", usm)
	}
}

func TestConfigureV3MsgFlags(t *testing.T) {
	// Auth only -> AuthNoPriv.
	c1 := &gosnmp.GoSNMP{}
	if err := configureV3(c1, Credential{Username: "u", AuthProtocol: "sha", AuthPassword: "p"}); err != nil {
		t.Fatal(err)
	}
	if c1.MsgFlags != gosnmp.AuthNoPriv {
		t.Errorf("auth-only flags = %v, want AuthNoPriv", c1.MsgFlags)
	}

	// Neither -> NoAuthNoPriv.
	c2 := &gosnmp.GoSNMP{}
	if err := configureV3(c2, Credential{Username: "u"}); err != nil {
		t.Fatal(err)
	}
	if c2.MsgFlags != gosnmp.NoAuthNoPriv {
		t.Errorf("no-auth flags = %v, want NoAuthNoPriv", c2.MsgFlags)
	}

	// No username -> error.
	if err := configureV3(&gosnmp.GoSNMP{}, Credential{}); err == nil {
		t.Error("expected an error without a username")
	}
	// Unknown protocol -> error.
	if err := configureV3(&gosnmp.GoSNMP{}, Credential{Username: "u", AuthPassword: "p", AuthProtocol: "bogus"}); err == nil {
		t.Error("expected an error for an unknown auth protocol")
	}
}
