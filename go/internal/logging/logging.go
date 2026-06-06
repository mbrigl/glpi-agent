// SPDX-License-Identifier: GPL-2.0-only

// Package logging is the agent logger.
//
// Derived from the upstream Perl modules lib/GLPI/Agent/Logger.pm and its
// backends lib/GLPI/Agent/Logger/{Stderr,File,Syslog}.pm. Levels, the
// verbosity-from-debug mapping, the per-level gating and the backend output
// formats (including the Stderr ANSI colours) mirror the Perl code.
package logging

import (
	"fmt"
	"io"
	"os"
	"strings"
	"sync"
	"time"
)

// Level mirrors the numeric levels in GLPI::Agent::Logger.
type Level int

const (
	LevelNone    Level = 0
	LevelError   Level = 1
	LevelWarning Level = 2
	LevelInfo    Level = 3
	LevelDebug   Level = 4
	LevelDebug2  Level = 5
)

// name returns the lowercase level label used in log output, matching the
// Perl level strings ('error', 'warning', 'info', 'debug', 'debug2').
func (l Level) name() string {
	switch l {
	case LevelError:
		return "error"
	case LevelWarning:
		return "warning"
	case LevelInfo:
		return "info"
	case LevelDebug:
		return "debug"
	case LevelDebug2:
		return "debug2"
	default:
		return "info"
	}
}

// VerbosityFromDebug maps the `debug` count to a verbosity level, mirroring the
// constructor in GLPI::Agent::Logger: debug>=2 -> DEBUG2, debug==1 -> DEBUG,
// otherwise INFO.
func VerbosityFromDebug(debug int) Level {
	switch {
	case debug >= 2:
		return LevelDebug2
	case debug == 1:
		return LevelDebug
	default:
		return LevelInfo
	}
}

// Backend consumes a single formatted log message, mirroring the addMessage
// contract of GLPI::Agent::Logger::Backend.
type Backend interface {
	AddMessage(level Level, message string)
}

// Options configures a Logger, corresponding to the subset of config keys
// returned by Config::logger() (debug, logger, logfacility, logfile,
// logfile-maxsize, color).
type Options struct {
	Debug          int
	Backends       []string // logger backends, e.g. ["Stderr"], ["File"]
	Logfile        string
	LogfileMaxSize int // megabytes; 0 disables rotation, as in Perl
	Color          bool
	Facility       string // syslog facility, e.g. "LOG_USER"
}

// Logger fans a message out to its backends if the message level passes the
// configured verbosity, mirroring GLPI::Agent::Logger.
type Logger struct {
	verbosity Level
	prefix    string
	backends  []Backend
}

// New builds a Logger from Options, instantiating one backend per name
// (defaulting to Stderr, as Perl does when no backend is configured).
func New(opts Options) *Logger {
	names := opts.Backends
	if len(names) == 0 {
		names = []string{"Stderr"}
	}

	l := &Logger{verbosity: VerbosityFromDebug(opts.Debug)}
	seen := map[string]bool{}
	for _, raw := range names {
		name := strings.ToLower(strings.TrimSpace(raw))
		if name == "" || seen[name] {
			continue
		}
		seen[name] = true
		switch name {
		case "stderr":
			l.backends = append(l.backends, &stderrBackend{color: opts.Color, w: os.Stderr})
		case "file":
			l.backends = append(l.backends, &fileBackend{
				path:    opts.Logfile,
				maxSize: int64(opts.LogfileMaxSize) * 1024 * 1024,
			})
		default:
			fmt.Fprintf(os.Stderr, "Failed to load Logger backend %s: not implemented\n", raw)
		}
	}
	return l
}

// SetPrefix sets a prefix prepended to every message, mirroring $self->{prefix}.
func (l *Logger) SetPrefix(prefix string) { l.prefix = prefix }

// Verbosity returns the current verbosity level.
func (l *Logger) Verbosity() Level { return l.verbosity }

func (l *Logger) log(level Level, message string) {
	if message == "" {
		return
	}
	if l.prefix != "" {
		message = l.prefix + message
	}
	message = strings.TrimRight(message, "\n")
	for _, b := range l.backends {
		b.AddMessage(level, message)
	}
}

// Debug2 logs at the debug2 level (only if verbosity allows).
func (l *Logger) Debug2(message string) {
	if l.verbosity >= LevelDebug2 {
		l.log(LevelDebug2, message)
	}
}

// Debug logs at the debug level.
func (l *Logger) Debug(message string) {
	if l.verbosity >= LevelDebug {
		l.log(LevelDebug, message)
	}
}

// Info logs at the info level.
func (l *Logger) Info(message string) {
	if l.verbosity >= LevelInfo {
		l.log(LevelInfo, message)
	}
}

// Warning logs at the warning level.
func (l *Logger) Warning(message string) {
	if l.verbosity >= LevelWarning {
		l.log(LevelWarning, message)
	}
}

// Error logs at the error level.
func (l *Logger) Error(message string) {
	if l.verbosity >= LevelError {
		l.log(LevelError, message)
	}
}

// stderrBackend mirrors GLPI::Agent::Logger::Stderr, including its ANSI colour
// formats keyed by level.
type stderrBackend struct {
	color bool
	w     io.Writer
	mu    sync.Mutex
}

// colorFormats are the exact per-level format strings from Logger/Stderr.pm.
var colorFormats = map[Level]string{
	LevelWarning: "\033[1;35m[%s] %s\033[0m\n",
	LevelError:   "\033[1;31m[%s] %s\033[0m\n",
	LevelInfo:    "\033[1;34m[%s]\033[0m %s\n",
	LevelDebug:   "\033[1;1m[%s]\033[0m %s\n",
	LevelDebug2:  "\033[1;36m[%s]\033[0m %s\n",
}

func (b *stderrBackend) AddMessage(level Level, message string) {
	format := "[%s] %s\n"
	if b.color {
		if f, ok := colorFormats[level]; ok {
			format = f
		}
	}
	b.mu.Lock()
	defer b.mu.Unlock()
	fmt.Fprintf(b.w, format, level.name(), message)
}

// fileBackend mirrors GLPI::Agent::Logger::File: append "[localtime][level]
// message", truncating the file first when it exceeds logfile-maxsize.
type fileBackend struct {
	path    string
	maxSize int64
	mu      sync.Mutex
}

func (b *fileBackend) AddMessage(level Level, message string) {
	if b.path == "" {
		return
	}
	b.mu.Lock()
	defer b.mu.Unlock()

	flags := os.O_CREATE | os.O_WRONLY | os.O_APPEND
	if b.maxSize > 0 {
		if info, err := os.Stat(b.path); err == nil && info.Size() > b.maxSize {
			flags = os.O_CREATE | os.O_WRONLY | os.O_TRUNC
		}
	}
	f, err := os.OpenFile(b.path, flags, 0o644)
	if err != nil {
		fmt.Fprintf(os.Stderr, "can't open %s: %v\n", b.path, err)
		return
	}
	defer f.Close()

	// Perl uses scalar localtime() ("Mon Jun  6 17:14:42 2026").
	stamp := time.Now().Format("Mon Jan  2 15:04:05 2006")
	fmt.Fprintf(f, "[%s][%s] %s\n", stamp, level.name(), message)
}
