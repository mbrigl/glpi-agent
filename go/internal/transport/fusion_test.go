// SPDX-License-Identifier: GPL-2.0-only

package transport

import (
	"io"
	"net/http"
	"net/http/httptest"
	"testing"
)

// TestFusionClientGET checks the GET action-arg encoding and JSON decoding.
func TestFusionClientGET(t *testing.T) {
	var gotQuery string
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		gotQuery = r.URL.RawQuery
		w.Header().Set("Content-Type", "application/json")
		_, _ = io.WriteString(w, `{"schedule":[{"task":"Collect","remote":"http://x"}]}`)
	}))
	defer srv.Close()

	c, err := NewFusionClient(GLPIOptions{AgentID: "id"})
	if err != nil {
		t.Fatal(err)
	}
	answer, err := c.Send(srv.URL, "GET", map[string]any{
		"action":    "getConfig",
		"machineid": "dev-1",
		"task":      map[string]string{"Collect": "3.0"},
	})
	if err != nil {
		t.Fatalf("send: %v", err)
	}
	if answer == nil {
		t.Fatal("nil answer")
	}
	sched, _ := answer["schedule"].([]any)
	if len(sched) != 1 {
		t.Fatalf("schedule = %v", answer["schedule"])
	}
	// action first, machineid + nested task[Collect] present.
	for _, want := range []string{"action=getConfig", "machineid=dev-1", "task[Collect]=3.0"} {
		if !contains(gotQuery, want) {
			t.Errorf("query %q missing %q", gotQuery, want)
		}
	}
}

// TestFusionClientPOST checks the POST split: action/uuid/method on the query
// string, the args in the form body.
func TestFusionClientPOST(t *testing.T) {
	var gotQuery, gotBody, gotCT string
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		gotQuery = r.URL.RawQuery
		gotCT = r.Header.Get("Content-Type")
		b, _ := io.ReadAll(r.Body)
		gotBody = string(b)
		w.Header().Set("Content-Type", "application/json")
		_, _ = io.WriteString(w, `{"token":"t2"}`)
	}))
	defer srv.Close()

	c, _ := NewFusionClient(GLPIOptions{AgentID: "id"})
	answer, err := c.Send(srv.URL, "POST", map[string]any{
		"action": "setAnswer",
		"uuid":   "job-1",
		"path":   "/etc/hosts",
		"_cpt":   1,
	})
	if err != nil {
		t.Fatalf("send: %v", err)
	}
	if answer["token"] != "t2" {
		t.Errorf("token = %v", answer["token"])
	}
	if gotCT != "application/x-www-form-urlencoded" {
		t.Errorf("content-type = %q", gotCT)
	}
	for _, want := range []string{"action=setAnswer", "uuid=job-1", "method=POST"} {
		if !contains(gotQuery, want) {
			t.Errorf("query %q missing %q", gotQuery, want)
		}
	}
	for _, want := range []string{"path=%2Fetc%2Fhosts", "_cpt=1", "uuid=job-1"} {
		if !contains(gotBody, want) {
			t.Errorf("body %q missing %q", gotBody, want)
		}
	}
}

// TestFusionClientEmptyBody returns (nil, nil) for an empty/non-JSON body
// (upstream "nothing to do").
func TestFusionClientEmptyBody(t *testing.T) {
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {}))
	defer srv.Close()
	c, _ := NewFusionClient(GLPIOptions{AgentID: "id"})
	answer, err := c.Send(srv.URL, "GET", map[string]any{"action": "getJobs"})
	if err != nil || answer != nil {
		t.Errorf("empty body = %v / %v, want nil/nil", answer, err)
	}
}

func contains(s, sub string) bool {
	for i := 0; i+len(sub) <= len(s); i++ {
		if s[i:i+len(sub)] == sub {
			return true
		}
	}
	return false
}
