// SPDX-License-Identifier: GPL-2.0-only

// Package config is the layered agent configuration.
//
// Derived from the upstream Perl module lib/GLPI/Agent/Config.pm: the default
// set (%default), the precedence (defaults < configuration file < command-line
// options), the agent.cfg file syntax (key = value, # comments, quotes,
// include) and the _checkContent normalisation (logfile implies the File logger
// backend, comma-split multi-value options, absolute paths, conf-reload-interval
// clamping). The Windows registry backend (_loadFromRegistry) is deferred.
package config

import (
	"bufio"
	"fmt"
	"os"
	"path/filepath"
	"regexp"
	"strconv"
	"strings"

	"github.com/glpi-project/glpi-agent/go/internal/logging"
)

// confReloadIntervalMin mirrors $confReloadIntervalMinValue.
const confReloadIntervalMin = 60

// defaults mirrors %default in Config.pm. nil represents Perl undef; lists start
// empty; numbers keep their default value.
func defaults() map[string]any {
	return map[string]any{
		"additional-content":      nil,
		"backend-collect-timeout": 180,
		"ca-cert-dir":             nil,
		"ca-cert-file":            nil,
		"color":                   nil,
		"conf-reload-interval":    0,
		"debug":                   nil,
		"delaytime":               3600,
		"esx-itemtype":            nil,
		"glpi-version":            nil,
		"itemtype":                nil,
		"remote-scheduling":       0,
		"remote-workers":          1,
		"force":                   nil,
		"html":                    nil,
		"json":                    nil,
		"lazy":                    nil,
		"local":                   nil,
		"logger":                  "Stderr",
		"logfile":                 nil,
		"logfacility":             "LOG_USER",
		"logfile-maxsize":         nil,
		"no-category":             []string{},
		"no-httpd":                nil,
		"no-ssl-check":            nil,
		"no-compression":          nil,
		"no-task":                 []string{},
		"no-p2p":                  nil,
		"oauth-client-id":         nil,
		"oauth-client-secret":     nil,
		"password":                nil,
		"proxy":                   nil,
		"httpd-ip":                nil,
		"httpd-port":              62354,
		"httpd-trust":             []string{},
		"httpd-ssl-cert-file":     nil,
		"httpd-ssl-key-file":      nil,
		"listen":                  nil,
		"remote":                  nil,
		"scan-homedirs":           nil,
		"scan-profiles":           nil,
		"server":                  nil,
		"ssl-cert-file":           nil,
		"ssl-key-file":            nil,
		"ssl-fingerprint":         nil,
		"ssl-keystore":            nil,
		"tag":                     nil,
		"tasks":                   nil,
		"timeout":                 180,
		"user":                    nil,
		"vardir":                  nil,
		"assetname-support":       1,
		"full-inventory-postpone": 14,
		"required-category":       []string{},
		"snmp-retries":            0,
	}
}

// listOptions are the comma-separated multi-value options split by _checkContent.
var listOptions = map[string]bool{
	"logger":            true,
	"local":             true,
	"server":            true,
	"httpd-trust":       true,
	"no-task":           true,
	"no-category":       true,
	"required-category": true,
	"tasks":             true,
	"ssl-fingerprint":   true,
}

// pathOptions are normalised to absolute paths by _checkContent.
var pathOptions = []string{"ca-cert-file", "ca-cert-dir", "ssl-cert-file", "logfile", "vardir"}

// Config holds the resolved configuration values keyed exactly as the Perl
// option names.
type Config struct {
	values map[string]any
}

// New returns a Config initialised with the defaults only.
func New() *Config {
	return &Config{values: defaults()}
}

var lineRE = regexp.MustCompile(`^\s*([\w-]+)\s*=\s*(.*)$`)

// LoadFile applies an agent.cfg-style file over the current values, mirroring
// loadFromFile: only keys present in the default set are accepted, quotes and
// trailing # comments are stripped, and an `include` directive pulls in another
// file (relative to the including file's directory).
func (c *Config) LoadFile(path string) error {
	return c.loadFile(path, map[string]bool{})
}

func (c *Config) loadFile(path string, loaded map[string]bool) error {
	abs, _ := filepath.Abs(path)
	if loaded[abs] {
		return nil // avoid include loops, like $self->{loadedConfs}
	}
	loaded[abs] = true

	f, err := os.Open(path)
	if err != nil {
		return fmt.Errorf("Config: Failed to open %s: %w", path, err)
	}
	defer f.Close()

	known := defaults()
	scanner := bufio.NewScanner(f)
	for scanner.Scan() {
		line := scanner.Text()
		m := lineRE.FindStringSubmatch(line)
		if m != nil {
			key, val := m[1], cleanValue(m[2])
			switch {
			case keyExists(known, key):
				c.values[key] = val
			case strings.EqualFold(key, "include"):
				c.include(val, path, loaded)
			default:
				fmt.Fprintf(os.Stderr, "Config: unknown configuration directive %s\n", key)
			}
			continue
		}
		if inc := matchInclude(line); inc != "" {
			c.include(cleanValue(inc), path, loaded)
		}
	}
	return scanner.Err()
}

func (c *Config) include(spec, fromFile string, loaded map[string]bool) {
	if !filepath.IsAbs(spec) {
		spec = filepath.Join(filepath.Dir(fromFile), spec)
	}
	info, err := os.Stat(spec)
	if err != nil {
		return
	}
	if info.IsDir() {
		entries, _ := os.ReadDir(spec)
		for _, e := range entries {
			if !e.IsDir() && strings.HasSuffix(e.Name(), ".cfg") {
				_ = c.loadFile(filepath.Join(spec, e.Name()), loaded)
			}
		}
		return
	}
	_ = c.loadFile(spec, loaded)
}

// Apply overlays command-line options (highest precedence), mirroring
// _loadUserParams. Only non-empty entries override.
func (c *Config) Apply(options map[string]any) {
	for k, v := range options {
		if v == nil {
			continue
		}
		c.values[k] = v
	}
}

// Check runs the _checkContent normalisation and returns an error for the
// antagonistic-option cases Perl dies on.
func (c *Config) Check() error {
	// A logfile implies a File logger backend.
	if s := c.String("logfile"); s != "" {
		c.values["logger"] = c.String("logger") + ",File"
	}
	// ca-cert-file and ca-cert-dir are mutually exclusive.
	if c.String("ca-cert-file") != "" && c.String("ca-cert-dir") != "" {
		return fmt.Errorf("Config: use either 'ca-cert-file' or 'ca-cert-dir' option, not both")
	}
	// A file logger backend needs a logfile.
	if logger := c.String("logger"); logger != "" && strings.Contains(strings.ToLower(logger), "file") && c.String("logfile") == "" {
		return fmt.Errorf("Config: usage of 'file' logger backend makes 'logfile' option mandatory")
	}

	// Split comma-separated multi-value options.
	for opt := range listOptions {
		c.values[opt] = splitList(c.values[opt])
	}

	// Normalise path options to absolute.
	for _, opt := range pathOptions {
		if s := c.String(opt); s != "" {
			if abs, err := filepath.Abs(s); err == nil {
				c.values[opt] = abs
			}
		}
	}

	// Clamp conf-reload-interval.
	if iv := c.Int("conf-reload-interval"); iv != 0 {
		switch {
		case iv < 0:
			c.values["conf-reload-interval"] = 0
		case iv < confReloadIntervalMin:
			c.values["conf-reload-interval"] = confReloadIntervalMin
		}
	}
	return nil
}

// LoggerOptions maps the logging-relevant keys to logging.Options, mirroring
// Config::logger().
func (c *Config) LoggerOptions() logging.Options {
	return logging.Options{
		Debug:          c.Int("debug"),
		Backends:       c.List("logger"),
		Logfile:        c.String("logfile"),
		LogfileMaxSize: c.Int("logfile-maxsize"),
		Color:          c.Bool("color"),
		Facility:       c.String("logfacility"),
	}
}

// String returns a string-valued option ("" for undef).
func (c *Config) String(key string) string {
	switch v := c.values[key].(type) {
	case nil:
		return ""
	case string:
		return v
	case int:
		return strconv.Itoa(v)
	default:
		return fmt.Sprintf("%v", v)
	}
}

// Int returns an int-valued option (0 for undef/unparseable).
func (c *Config) Int(key string) int {
	switch v := c.values[key].(type) {
	case int:
		return v
	case string:
		n, _ := strconv.Atoi(strings.TrimSpace(v))
		return n
	default:
		return 0
	}
}

// Bool reports whether an option is set to a truthy value, matching Perl's
// notion (undef/empty/"0" are false).
func (c *Config) Bool(key string) bool {
	switch v := c.values[key].(type) {
	case nil:
		return false
	case bool:
		return v
	case int:
		return v != 0
	case string:
		return v != "" && v != "0"
	default:
		return true
	}
}

// List returns a multi-value option as a string slice.
func (c *Config) List(key string) []string {
	return splitList(c.values[key])
}

func keyExists(m map[string]any, key string) bool {
	_, ok := m[key]
	return ok
}

// cleanValue strips a quoted value or a trailing # comment, mirroring the value
// handling in loadFromFile.
func cleanValue(val string) string {
	val = strings.TrimRight(val, " \t")
	if len(val) >= 2 && (val[0] == '\'' || val[0] == '"') {
		q := val[0]
		if i := strings.IndexByte(val[1:], q); i >= 0 {
			return val[1 : 1+i]
		}
	}
	if i := strings.Index(val, "#"); i >= 0 {
		val = strings.TrimRight(val[:i], " \t")
	}
	return val
}

var includeRE = regexp.MustCompile(`^\s*include\s+(.+)$`)

func matchInclude(line string) string {
	if m := includeRE.FindStringSubmatch(line); m != nil {
		return m[1]
	}
	return ""
}

// splitList coerces a value into a string slice, splitting scalars on runs of
// commas (Perl split /,+/) and dropping empties.
func splitList(v any) []string {
	switch val := v.(type) {
	case nil:
		return []string{}
	case []string:
		return val
	case string:
		if val == "" {
			return []string{}
		}
		parts := regexp.MustCompile(`,+`).Split(val, -1)
		out := make([]string, 0, len(parts))
		for _, p := range parts {
			if p != "" {
				out = append(out, p)
			}
		}
		return out
	default:
		return []string{}
	}
}
