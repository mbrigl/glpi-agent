// SPDX-License-Identifier: GPL-2.0-only

// Package collect implements the GLPI Collect task (Task/Collect.pm): a
// server-driven task that fetches a list of collection jobs (findFile /
// getFromRegistry / getFromWMI) over the legacy "Fusion" plugin protocol, runs
// them locally and POSTs the results back.
package collect

import (
	"strconv"

	"github.com/glpi-project/glpi-agent/go/internal/logging"
)

// Version is the Collect task version advertised to the server (Collect/Version.pm).
const Version = "3.0"

// Validation sentinels for a module's JSON validation spec (Collect/Common.pm).
const (
	Optional  = 0
	Mandatory = 1
)

// Module is one collection function (findFile, getFromRegistry, getFromWMI).
type Module interface {
	// Function is the server-facing function name.
	Function() string
	// Validation is the JSON validation spec for a job of this function.
	Validation() map[string]any
	// Results runs the collection for a validated job and returns the result
	// records (each a flat map of scalar values).
	Results(job map[string]any, log *logging.Logger) []map[string]any
}

// FusionSender is the subset of the Fusion HTTP client the task needs.
type FusionSender interface {
	Send(rawURL, method string, args map[string]any) (map[string]any, error)
}

// Task orchestrates the Collect dialogue against a server target.
type Task struct {
	log      *logging.Logger
	deviceID string
	modules  map[string]Module
}

// NewTask builds a Collect task with the given collection modules.
func NewTask(log *logging.Logger, deviceID string, modules ...Module) *Task {
	m := make(map[string]Module, len(modules))
	for _, mod := range modules {
		m[mod.Function()] = mod
	}
	return &Task{log: log, deviceID: deviceID, modules: m}
}

// Run performs the getConfig handshake against the server target and processes
// every enabled Collect job, mirroring Task/Collect.pm::run.
func (t *Task) Run(client FusionSender, targetURL string) error {
	config, err := client.Send(targetURL, "GET", map[string]any{
		"action":    "getConfig",
		"machineid": t.deviceID,
		"task":      map[string]string{"Collect": Version},
	})
	if err != nil {
		return err
	}
	if config == nil {
		t.log.Info("Collect task not supported by server")
		return nil
	}
	schedule, ok := config["schedule"].([]any)
	if !ok || len(schedule) == 0 {
		t.log.Info("No Collect job enabled or Collect support disabled server side.")
		return nil
	}

	runJobs := 0
	for _, s := range schedule {
		job, ok := s.(map[string]any)
		if !ok || str(job["task"]) != "Collect" {
			continue
		}
		remote := str(job["remote"])
		if remote == "" {
			continue
		}
		if err := t.processRemote(client, remote); err != nil {
			t.log.Error("Collect remote failed: " + err.Error())
		}
		runJobs++
	}
	if runJobs == 0 {
		t.log.Info("No Collect job found in server jobs list.")
	}
	return nil
}

// processRemote runs every job offered by one remote endpoint, mirroring
// Task/Collect.pm::_processRemote (getJobs → setAnswer per result → jobsDone),
// including the CSRF-token handling.
func (t *Task) processRemote(client FusionSender, remoteURL string) error {
	answer, err := client.Send(remoteURL, "GET", map[string]any{
		"action":    "getJobs",
		"machineid": t.deviceID,
	})
	if err != nil {
		return err
	}
	if answer == nil || len(answer) == 0 {
		t.log.Debug("Nothing to do")
		return nil
	}
	if !t.validateAnswer(answer) {
		return nil
	}

	jobs, _ := answer["jobs"].([]any)
	method := "GET"
	if str(answer["postmethod"]) == "POST" {
		method = "POST"
	}
	token := str(answer["token"])
	hasCSRF := token != ""
	jobsDone := map[string]bool{}

	for _, j := range jobs {
		job, ok := j.(map[string]any)
		if !ok {
			continue
		}
		uuid := str(job["uuid"])
		if uuid == "" {
			t.log.Error("UUID key missing")
			continue
		}
		function := str(job["function"])
		module := t.modules[function]
		if module == nil {
			t.log.Error("Bad function '" + function + "'")
			continue
		}

		t.log.Debug2("Collect job has uuid: " + uuid)
		results := module.Results(job, t.log)
		count := len(results)
		if count == 0 {
			// Send one answer with _cpt=0 so the server knows the job ran.
			results = []map[string]any{{}}
		}

		csrfFailed := false
		remaining := count
		for _, result := range results {
			if count != 0 && len(result) == 0 {
				continue
			}
			result["uuid"] = uuid
			result["action"] = "setAnswer"
			result["_cpt"] = count
			if token != "" {
				result["_glpi_csrf_token"] = token
			}
			if sid, ok := job["_sid"]; ok {
				result["_sid"] = sid
			}
			a, err := client.Send(remoteURL, method, result)
			if err != nil {
				return err
			}
			token = ""
			if a != nil {
				token = str(a["token"])
			}
			remaining--

			if hasCSRF && token == "" {
				t.log.Error("Bad answer: CSRF checking is failing")
				_, _ = client.Send(remoteURL, "GET", map[string]any{"uuid": uuid, "action": "setAnswer"})
				_, _ = client.Send(remoteURL, "GET", map[string]any{"uuid": uuid, "action": "setAnswer", "csrf_failure": 1})
				csrfFailed = true
				break
			}
		}
		if csrfFailed {
			break
		}
		jobsDone[uuid] = true
	}

	for uuid := range jobsDone {
		if _, err := client.Send(remoteURL, "GET", map[string]any{"action": "jobsDone", "uuid": uuid}); err != nil {
			t.log.Debug2("Got no response on " + uuid + " jobsDone action")
		}
	}
	return nil
}

// validateAnswer checks the getJobs answer structure + each job against its
// module's validation spec, mirroring Collect/Common.pm::validateAnswer.
func (t *Task) validateAnswer(answer map[string]any) bool {
	jobs, ok := answer["jobs"].([]any)
	if !ok {
		t.log.Debug("Bad JSON: Missing jobs")
		return false
	}
	for _, j := range jobs {
		job, ok := j.(map[string]any)
		if !ok {
			return false
		}
		for _, k := range []string{"uuid", "function"} {
			if _, ok := job[k]; !ok {
				t.log.Debug("Bad JSON: Missing key '" + k + "' in job")
				return false
			}
		}
		module := t.modules[str(job["function"])]
		if module == nil {
			t.log.Debug("Bad JSON: not supported 'function' key value in job")
			return false
		}
		for attr, spec := range module.Validation() {
			if !validateSpec(job, attr, spec) {
				t.log.Debug("Bad JSON: '" + str(job["function"]) + "' job JSON format is not valid")
				return false
			}
		}
	}
	return true
}

// validateSpec checks one validation rule (mirrors Collect/Common.pm::_validateSpec).
func validateSpec(base map[string]any, key string, spec any) bool {
	switch s := spec.(type) {
	case map[string]any:
		sub, ok := base[key].(map[string]any)
		if !ok {
			return false
		}
		for attr, as := range s {
			if !validateSpec(sub, attr, as) {
				return false
			}
		}
		return true
	case int:
		if s == Mandatory {
			_, ok := base[key]
			return ok
		}
		return true
	default:
		return true
	}
}

// str renders a JSON scalar value as a string.
func str(v any) string {
	switch t := v.(type) {
	case nil:
		return ""
	case string:
		return t
	case float64:
		if t == float64(int64(t)) {
			return strconv.FormatInt(int64(t), 10)
		}
		return strconv.FormatFloat(t, 'g', -1, 64)
	case bool:
		if t {
			return "1"
		}
		return "0"
	default:
		return ""
	}
}
