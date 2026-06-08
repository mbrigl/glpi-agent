// SPDX-License-Identifier: GPL-2.0-only

package target

import "testing"

func TestCanonicalURL(t *testing.T) {
	cases := map[string]string{
		"https://glpi.example/front/inventory.php": "https://glpi.example/front/inventory.php",
		"http://glpi.example":                      "http://glpi.example",
		"glpi.example":                             "http://glpi.example",
		"glpi.example/front/inventory.php":         "http://glpi.example/front/inventory.php",
	}
	for in, want := range cases {
		s, err := NewServer(in)
		if err != nil {
			t.Fatalf("%s: %v", in, err)
		}
		if s.URL != want {
			t.Errorf("%s -> %s, want %s", in, s.URL, want)
		}
	}
}

func TestCanonicalURLRejectsBadScheme(t *testing.T) {
	if _, err := NewServer("ftp://glpi.example"); err == nil {
		t.Error("expected an error for a non-http(s) scheme")
	}
	if _, err := NewServer(""); err == nil {
		t.Error("expected an error for an empty URL")
	}
}

func TestSubdir(t *testing.T) {
	s, _ := NewServer("https://glpi.example/front/inventory.php")
	// "/" -> "_", trailing underscore trimmed; userinfo dropped.
	if got := s.Subdir(); got != "https:__glpi.example_front_inventory.php" {
		t.Errorf("Subdir = %q", got)
	}
	withUser, _ := NewServer("https://user:pass@glpi.example/")
	if got := withUser.Subdir(); got != "https:__glpi.example" {
		t.Errorf("Subdir with userinfo = %q (should drop credentials)", got)
	}
}
