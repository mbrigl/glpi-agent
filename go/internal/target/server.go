// SPDX-License-Identifier: GPL-2.0-only

// Package target models where an inventory is sent. The Server target points at
// a GLPI server URL, derived from lib/GLPI/Agent/Target/Server.pm.
package target

import (
	"fmt"
	"net/url"
	"strings"
)

// Server is a GLPI server target: a canonical URL plus the per-URL vardir
// subdirectory used to keep this target's state.
type Server struct {
	URL string
}

// NewServer builds a server target from a raw URL or bare hostname, mirroring
// Target/Server::_getCanonicalURL: a value without a scheme is treated as a
// (possibly host/path) HTTP location; an explicit scheme must be http or https.
func NewServer(raw string) (*Server, error) {
	canonical, err := canonicalURL(raw)
	if err != nil {
		return nil, err
	}
	return &Server{URL: canonical}, nil
}

// Subdir returns the storage subdirectory derived from the URL (the upstream
// scheme of replacing "/" with "_"), used to keep per-server state separate.
func (s *Server) Subdir() string {
	u, err := url.Parse(s.URL)
	if err != nil {
		return sanitizeSubdir(s.URL)
	}
	u.User = nil // drop any userinfo before deriving the directory
	return sanitizeSubdir(u.String())
}

// canonicalURL normalises a server URL, mirroring _getCanonicalURL.
func canonicalURL(raw string) (string, error) {
	raw = strings.TrimSpace(raw)
	if raw == "" {
		return "", fmt.Errorf("empty server URL")
	}

	u, err := url.Parse(raw)
	if err != nil {
		return "", fmt.Errorf("invalid server URL %q: %w", raw, err)
	}

	if u.Scheme == "" {
		// A bare hostname (optionally host/path); default to http.
		host, path := raw, ""
		if i := strings.IndexByte(raw, '/'); i >= 0 {
			host, path = raw[:i], raw[i+1:]
		}
		u = &url.URL{Scheme: "http", Host: host, Path: "/" + path}
		if path == "" {
			u.Path = ""
		}
		return u.String(), nil
	}

	if u.Scheme != "http" && u.Scheme != "https" {
		return "", fmt.Errorf("invalid protocol for URL: %s", raw)
	}
	return u.String(), nil
}

// sanitizeSubdir turns a URL into a filesystem-safe subdirectory name, replacing
// "/" with "_" and stripping a trailing underscore (Target/Server subdir logic).
func sanitizeSubdir(s string) string {
	s = strings.ReplaceAll(s, "/", "_")
	return strings.TrimRight(s, "_")
}
