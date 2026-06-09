// SPDX-License-Identifier: GPL-2.0-only

// Package httpd implements the agent's HTTP control server, a Go port of the
// core of lib/GLPI/Agent/HTTP/Server.pm: the /status, /now and root endpoints,
// gated by the httpd-trust IP allowlist. The web GUI (ToolBox), the proxy and
// deploy plugins are intentionally not ported.
package httpd

import (
	"fmt"
	"html"
	"net"
	"net/http"
	"strings"
	"time"

	"github.com/glpi-project/glpi-agent/go/internal/agent"
	"github.com/glpi-project/glpi-agent/go/internal/logging"
)

// Controller is the agent surface the control server needs (satisfied by
// *agent.Agent), kept as an interface so the server is testable with a fake.
type Controller interface {
	Status() string
	Targets() []agent.TargetInfo
	RunNow()
}

// Server is the HTTP control server.
type Server struct {
	agent Controller
	log   *logging.Logger
	trust []*net.IPNet // parsed httpd-trust entries
}

// New builds a control server over the agent, parsing the httpd-trust entries
// (single IPs or CIDR ranges; hostnames are skipped with a warning).
func New(ctrl Controller, log *logging.Logger, trust []string) *Server {
	return &Server{agent: ctrl, log: log, trust: parseTrust(trust, log)}
}

// ServeHTTP dispatches the supported control endpoints, mirroring the SWITCH of
// HTTP/Server.pm::_handle.
func (s *Server) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	path := r.URL.Path
	clientIP := clientIP(r)
	s.log.Debug(fmt.Sprintf("[http server] %s request %s from client %s", r.Method, path, clientIP))

	switch {
	case path == "/":
		s.onlyGET(w, r, func() { s.handleRoot(w) })
	case path == "/status":
		s.onlyGET(w, r, func() { s.handleStatus(w) })
	case path == "/now" || strings.HasPrefix(path, "/now/"):
		s.onlyGET(w, r, func() { s.handleNow(w, clientIP) })
	default:
		http.Error(w, "Not Found", http.StatusNotFound)
	}
}

// onlyGET runs h for GET requests, answering 400 otherwise (as the Perl server
// does for an unexpected method).
func (s *Server) onlyGET(w http.ResponseWriter, r *http.Request, h func()) {
	if r.Method != http.MethodGet {
		http.Error(w, "Bad Request", http.StatusBadRequest)
		return
	}
	h()
}

// handleStatus answers "status: <status>" (HTTP/Server.pm::_handle_status).
func (s *Server) handleStatus(w http.ResponseWriter) {
	w.Header().Set("Content-Type", "text/plain")
	fmt.Fprintf(w, "status: %s", s.agent.Status())
}

// handleNow triggers an immediate run of all targets when the client is trusted,
// mirroring the run-now behaviour of HTTP/Server.pm::_handle_now (without the
// CORS / event machinery).
func (s *Server) handleNow(w http.ResponseWriter, clientIP string) {
	if !s.isTrusted(clientIP) {
		s.log.Debug("[http server] /now denied: untrusted address " + clientIP)
		http.Error(w, "Access denied", http.StatusForbidden)
		return
	}
	s.log.Info("[http server] rescheduling next contact for all targets right now")
	s.agent.RunNow()
	w.Header().Set("Content-Type", "text/html")
	fmt.Fprint(w, "<html><body><h1>OK</h1></body></html>")
}

// handleRoot renders a minimal status page listing the targets and their next
// run times (the index page of HTTP/Server.pm::_handle_root).
func (s *Server) handleRoot(w http.ResponseWriter) {
	w.Header().Set("Content-Type", "text/html")
	var b strings.Builder
	b.WriteString("<html><head><title>GLPI Agent</title></head><body>")
	b.WriteString("<h1>GLPI Agent</h1>")
	fmt.Fprintf(&b, "<p>Status: %s</p>", html.EscapeString(s.agent.Status()))
	b.WriteString("<table><tr><th>Target</th><th>Next run</th></tr>")
	for _, t := range s.agent.Targets() {
		next := "now"
		if t.NextRun.After(time.Now()) {
			next = t.NextRun.Format(time.RFC3339)
		}
		fmt.Fprintf(&b, "<tr><td>%s</td><td>%s</td></tr>", html.EscapeString(t.Name), html.EscapeString(next))
	}
	b.WriteString("</table></body></html>")
	_, _ = w.Write([]byte(b.String()))
}

// isTrusted reports whether the client address is in the httpd-trust allowlist
// (HTTP/Server.pm::_isTrusted). An empty allowlist trusts nobody.
func (s *Server) isTrusted(clientIP string) bool {
	ip := net.ParseIP(clientIP)
	if ip == nil {
		return false
	}
	for _, n := range s.trust {
		if n.Contains(ip) {
			return true
		}
	}
	return false
}

// parseTrust turns httpd-trust entries into IP networks; single IPs become a
// host route (/32 or /128). Hostnames are skipped (not resolved here).
func parseTrust(entries []string, log *logging.Logger) []*net.IPNet {
	var nets []*net.IPNet
	for _, e := range entries {
		e = strings.TrimSpace(e)
		if e == "" {
			continue
		}
		if strings.Contains(e, "/") {
			if _, n, err := net.ParseCIDR(e); err == nil {
				nets = append(nets, n)
				continue
			}
		} else if ip := net.ParseIP(e); ip != nil {
			bits := 32
			if ip.To4() == nil {
				bits = 128
			}
			nets = append(nets, &net.IPNet{IP: ip, Mask: net.CIDRMask(bits, bits)})
			continue
		}
		if log != nil {
			log.Debug("[http server] ignoring unsupported httpd-trust entry: " + e)
		}
	}
	return nets
}

// clientIP extracts the client IP from the request's remote address.
func clientIP(r *http.Request) string {
	host, _, err := net.SplitHostPort(r.RemoteAddr)
	if err != nil {
		return r.RemoteAddr
	}
	return host
}
