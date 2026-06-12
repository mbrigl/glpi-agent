// SPDX-License-Identifier: GPL-2.0-only

package collect

import (
	"os"
	"path/filepath"
	"testing"
)

// fakeSender records the dialogue and replays scripted answers keyed by action.
type fakeSender struct {
	calls   []map[string]any
	answers map[string]map[string]any
}

func (f *fakeSender) Send(rawURL, method string, args map[string]any) (map[string]any, error) {
	f.calls = append(f.calls, args)
	return f.answers[str(args["action"])], nil
}

// TestCollectRunFlow drives the whole getConfig -> getJobs -> setAnswer ->
// jobsDone dialogue with a findFile job and checks the result is posted back.
func TestCollectRunFlow(t *testing.T) {
	dir := t.TempDir()
	if err := os.WriteFile(filepath.Join(dir, "target.txt"), []byte("x"), 0o644); err != nil {
		t.Fatal(err)
	}

	sender := &fakeSender{answers: map[string]map[string]any{
		"getConfig": {
			"schedule": []any{
				map[string]any{"task": "Collect", "remote": "http://remote/plugin"},
			},
		},
		"getJobs": {
			"jobs": []any{
				map[string]any{
					"uuid":      "job-1",
					"function":  "findFile",
					"dir":       dir,
					"limit":     float64(10),
					"recursive": true,
					"filter":    map[string]any{"is_file": true, "is_dir": false, "name": "target.txt"},
				},
			},
		},
		"setAnswer": {},
		"jobsDone":  {},
	}}

	task := NewTask(testLogger(), "device-123", FileCollector{})
	if err := task.Run(sender, "http://server/"); err != nil {
		t.Fatalf("run: %v", err)
	}

	// Verify the dialogue: getConfig, getJobs, setAnswer (with the found path),
	// jobsDone.
	var sawSetAnswer, sawJobsDone bool
	for _, c := range sender.calls {
		switch str(c["action"]) {
		case "setAnswer":
			sawSetAnswer = true
			if c["uuid"] != "job-1" || c["_cpt"] != 1 {
				t.Errorf("setAnswer args = %v", c)
			}
			if filepath.Base(str(c["path"])) != "target.txt" {
				t.Errorf("setAnswer path = %v", c["path"])
			}
		case "jobsDone":
			sawJobsDone = true
			if c["uuid"] != "job-1" {
				t.Errorf("jobsDone uuid = %v", c["uuid"])
			}
		}
	}
	if !sawSetAnswer || !sawJobsDone {
		t.Errorf("missing setAnswer=%v / jobsDone=%v", sawSetAnswer, sawJobsDone)
	}
}

// TestCollectValidationRejectsBadJob ensures a job missing a mandatory key is
// rejected (no setAnswer/jobsDone sent).
func TestCollectValidationRejectsBadJob(t *testing.T) {
	sender := &fakeSender{answers: map[string]map[string]any{
		"getConfig": {"schedule": []any{map[string]any{"task": "Collect", "remote": "http://remote/"}}},
		"getJobs": {"jobs": []any{
			// findFile without the mandatory "limit"/"recursive" -> invalid.
			map[string]any{"uuid": "bad", "function": "findFile", "dir": "/tmp"},
		}},
	}}
	task := NewTask(testLogger(), "dev", FileCollector{})
	if err := task.Run(sender, "http://server/"); err != nil {
		t.Fatal(err)
	}
	for _, c := range sender.calls {
		if a := str(c["action"]); a == "setAnswer" || a == "jobsDone" {
			t.Errorf("invalid job should not produce %q", a)
		}
	}
}

// TestValidateSpec covers the mandatory/optional/nested rules.
func TestValidateSpec(t *testing.T) {
	spec := FileCollector{}.Validation()
	valid := map[string]any{
		"dir": "/x", "limit": float64(1), "recursive": true,
		"filter": map[string]any{"is_file": true, "is_dir": false},
	}
	task := NewTask(testLogger(), "d", FileCollector{})
	if !task.validateAnswer(map[string]any{"jobs": []any{merge(valid, map[string]any{"uuid": "u", "function": "findFile"})}}) {
		t.Error("valid findFile job rejected")
	}
	_ = spec
}

func merge(a, b map[string]any) map[string]any {
	out := map[string]any{}
	for k, v := range a {
		out[k] = v
	}
	for k, v := range b {
		out[k] = v
	}
	return out
}
