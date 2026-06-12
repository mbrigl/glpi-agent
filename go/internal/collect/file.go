// SPDX-License-Identifier: GPL-2.0-only

package collect

import (
	"crypto/sha256"
	"crypto/sha512"
	"encoding/hex"
	"errors"
	"hash"
	"io"
	"os"
	"path/filepath"
	"regexp"
	"strings"

	"github.com/glpi-project/glpi-agent/go/internal/logging"
)

// FileCollector implements the "findFile" function (Collect/File.pm): walk a
// directory tree and return {size, path} for files matching the job filter.
type FileCollector struct{}

func (FileCollector) Function() string { return "findFile" }

func (FileCollector) Validation() map[string]any {
	return map[string]any{
		"dir":       Mandatory,
		"limit":     Mandatory,
		"recursive": Mandatory,
		"filter": map[string]any{
			"regex":          Optional,
			"sizeEquals":     Optional,
			"sizeGreater":    Optional,
			"sizeLower":      Optional,
			"checkSumSHA512": Optional,
			"checkSumSHA2":   Optional,
			"name":           Optional,
			"iname":          Optional,
			"is_file":        Mandatory,
			"is_dir":         Mandatory,
		},
	}
}

// Results walks job["dir"] applying the filter and returns up to job["limit"]
// {size, path} records, mirroring Collect/File.pm::results.
func (FileCollector) Results(job map[string]any, log *logging.Logger) []map[string]any {
	dir := str(job["dir"])
	if dir == "" {
		return nil
	}
	if info, err := os.Stat(dir); err != nil || !info.IsDir() {
		return nil
	}
	limit := intOf(job["limit"])
	recursive := truthy(job["recursive"])
	filter, _ := job["filter"].(map[string]any)
	if filter == nil {
		filter = map[string]any{}
	}

	var (
		nameRE   *regexp.Regexp
		wantName = str(filter["name"])
		wantINam = str(filter["iname"])
	)
	if r := str(filter["regex"]); r != "" {
		nameRE, _ = regexp.Compile(r)
	}
	wantSHA512 := strings.ToLower(str(filter["checkSumSHA512"]))
	wantSHA256 := strings.ToLower(strFirst(filter["checkSumSHA256"], filter["checkSumSHA2"]))

	var results []map[string]any
	walkErr := filepath.Walk(dir, func(path string, info os.FileInfo, err error) error {
		if err != nil {
			return nil
		}
		if !recursive && info.IsDir() && path != dir {
			return filepath.SkipDir
		}

		// is_dir without a checksum -> directories only.
		if truthy(filter["is_dir"]) && wantSHA512 == "" && wantSHA256 == "" {
			if !info.IsDir() {
				return nil
			}
		}
		if truthy(filter["is_file"]) && !info.Mode().IsRegular() {
			return nil
		}

		base := filepath.Base(path)
		if wantName != "" && base != wantName {
			return nil
		}
		if wantINam != "" && !strings.EqualFold(base, wantINam) {
			return nil
		}
		if nameRE != nil && !nameRE.MatchString(path) {
			return nil
		}

		size := info.Size()
		if v, ok := numOf(filter["sizeEquals"]); ok && size != v {
			return nil
		}
		if v, ok := numOf(filter["sizeGreater"]); ok && size < v {
			return nil
		}
		if v, ok := numOf(filter["sizeLower"]); ok && size > v {
			return nil
		}

		if wantSHA512 != "" && fileHashHex(path, sha512.New()) != wantSHA512 {
			return nil
		}
		if wantSHA256 != "" && fileHashHex(path, sha256.New()) != wantSHA256 {
			return nil
		}

		log.Debug2("Found file: " + path)
		results = append(results, map[string]any{"size": size, "path": path})
		if len(results) >= limit {
			return errStopWalk
		}
		return nil
	})
	if walkErr != nil && !errors.Is(walkErr, errStopWalk) {
		log.Debug("findFile walk error: " + walkErr.Error())
	}
	return results
}

// errStopWalk stops filepath.Walk once the result limit is reached.
var errStopWalk = errors.New("collect: result limit reached")

// fileHashHex returns the lowercase hex digest of a file using the given hash.
func fileHashHex(path string, h hash.Hash) string {
	f, err := os.Open(path)
	if err != nil {
		return ""
	}
	defer f.Close()
	if _, err := io.Copy(h, f); err != nil {
		return ""
	}
	return hex.EncodeToString(h.Sum(nil))
}

func intOf(v any) int {
	n, _ := numOf(v)
	return int(n)
}

// numOf parses a numeric (JSON number or numeric string) value.
func numOf(v any) (int64, bool) {
	switch t := v.(type) {
	case float64:
		return int64(t), true
	case int:
		return int64(t), true
	case int64:
		return t, true
	case string:
		if t == "" {
			return 0, false
		}
		var n int64
		for _, c := range t {
			if c < '0' || c > '9' {
				return 0, false
			}
			n = n*10 + int64(c-'0')
		}
		return n, true
	default:
		return 0, false
	}
}

func truthy(v any) bool {
	switch t := v.(type) {
	case bool:
		return t
	case float64:
		return t != 0
	case string:
		return t != "" && t != "0" && t != "false"
	case nil:
		return false
	default:
		return true
	}
}

func strFirst(vs ...any) string {
	for _, v := range vs {
		if s := str(v); s != "" {
			return s
		}
	}
	return ""
}
