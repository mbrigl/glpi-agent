// SPDX-License-Identifier: GPL-2.0-only

package httpd

import (
	"net/http"
	"net/http/httptest"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/glpi-project/glpi-agent/go/internal/agent"
	"github.com/glpi-project/glpi-agent/go/internal/logging"
)

// fakeAgent is a Controller for the tests.
type fakeAgent struct {
	status   string
	targets  []agent.TargetInfo
	runNowed int
}

func (f *fakeAgent) Status() string              { return f.status }
func (f *fakeAgent) Targets() []agent.TargetInfo { return f.targets }
func (f *fakeAgent) RunNow()                     { f.runNowed++ }

func testServer(t *testing.T, ctrl Controller, trust []string) *Server {
	log := logging.New(logging.Options{Backends: []string{"File"}, Logfile: filepath.Join(t.TempDir(), "log")})
	return New(ctrl, log, trust)
}

func do(s *Server, method, target, remoteAddr string) *httptest.ResponseRecorder {
	req := httptest.NewRequest(method, target, nil)
	req.RemoteAddr = remoteAddr
	rec := httptest.NewRecorder()
	s.ServeHTTP(rec, req)
	return rec
}

func TestStatusEndpoint(t *testing.T) {
	s := testServer(t, &fakeAgent{status: "waiting"}, nil)
	rec := do(s, http.MethodGet, "/status", "127.0.0.1:5000")
	if rec.Code != 200 {
		t.Fatalf("code = %d", rec.Code)
	}
	if rec.Body.String() != "status: waiting" {
		t.Errorf("body = %q", rec.Body.String())
	}
	if ct := rec.Header().Get("Content-Type"); ct != "text/plain" {
		t.Errorf("content-type = %q", ct)
	}
}

func TestNowTrusted(t *testing.T) {
	fa := &fakeAgent{status: "waiting"}
	s := testServer(t, fa, []string{"10.0.0.0/8"})
	rec := do(s, http.MethodGet, "/now", "10.1.2.3:4000")
	if rec.Code != 200 {
		t.Fatalf("code = %d (trusted /now should succeed)", rec.Code)
	}
	if fa.runNowed != 1 {
		t.Errorf("RunNow called %d times, want 1", fa.runNowed)
	}
}

func TestNowUntrusted(t *testing.T) {
	fa := &fakeAgent{status: "waiting"}
	s := testServer(t, fa, []string{"10.0.0.0/8"})
	rec := do(s, http.MethodGet, "/now", "192.168.1.5:4000")
	if rec.Code != http.StatusForbidden {
		t.Errorf("code = %d, want 403 for untrusted /now", rec.Code)
	}
	if fa.runNowed != 0 {
		t.Errorf("RunNow should not run for an untrusted client")
	}
}

func TestNowEmptyTrustDenied(t *testing.T) {
	fa := &fakeAgent{}
	s := testServer(t, fa, nil) // no trust configured -> nobody trusted
	rec := do(s, http.MethodGet, "/now", "127.0.0.1:4000")
	if rec.Code != http.StatusForbidden {
		t.Errorf("code = %d, want 403 when no trust configured", rec.Code)
	}
}

func TestRootPage(t *testing.T) {
	next := time.Now().Add(30 * time.Minute)
	fa := &fakeAgent{status: "running: srv", targets: []agent.TargetInfo{{Name: "https://glpi.example/", NextRun: next}}}
	s := testServer(t, fa, nil)
	rec := do(s, http.MethodGet, "/", "127.0.0.1:4000")
	if rec.Code != 200 {
		t.Fatalf("code = %d", rec.Code)
	}
	body := rec.Body.String()
	if !strings.Contains(body, "running: srv") || !strings.Contains(body, "glpi.example") {
		t.Errorf("root page missing status/target: %s", body)
	}
}

func TestMethodNotGet(t *testing.T) {
	s := testServer(t, &fakeAgent{}, []string{"127.0.0.1"})
	if rec := do(s, http.MethodPost, "/status", "127.0.0.1:1"); rec.Code != http.StatusBadRequest {
		t.Errorf("POST /status code = %d, want 400", rec.Code)
	}
}

func TestUnknownPath(t *testing.T) {
	s := testServer(t, &fakeAgent{}, nil)
	if rec := do(s, http.MethodGet, "/bogus", "127.0.0.1:1"); rec.Code != http.StatusNotFound {
		t.Errorf("unknown path code = %d, want 404", rec.Code)
	}
}

// TestTrustSingleIP checks a bare IP trust entry (host route).
func TestTrustSingleIP(t *testing.T) {
	fa := &fakeAgent{}
	s := testServer(t, fa, []string{"127.0.0.1"})
	if rec := do(s, http.MethodGet, "/now", "127.0.0.1:9"); rec.Code != 200 {
		t.Errorf("trusted single IP /now code = %d, want 200", rec.Code)
	}
	if rec := do(s, http.MethodGet, "/now", "127.0.0.2:9"); rec.Code != http.StatusForbidden {
		t.Errorf("other IP /now code = %d, want 403", rec.Code)
	}
}
