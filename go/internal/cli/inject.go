// SPDX-License-Identifier: GPL-2.0-only

package cli

import (
	"bytes"
	"compress/zlib"
	"crypto/rand"
	"flag"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"regexp"
	"strings"

	"github.com/glpi-project/glpi-agent/go/internal/transport"
)

// uuidInFilename matches an embedded UUID in an inventory filename, mirroring
// the regex used by bin/glpi-injector to recover an agent id from the name.
var uuidInFilename = regexp.MustCompile(`(?i)([0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12})\.(?:json|data)$`)

// runInject implements the `inject` subcommand, derived from bin/glpi-injector.
// Phase 1 covers file/directory/stdin sources, zlib (de)compression, agent-id
// derivation and the POST; the OAuth and pending-proxy paths are deferred.
func runInject(args []string, stdout, stderr io.Writer) int {
	fs := flag.NewFlagSet("inject", flag.ContinueOnError)
	fs.SetOutput(stderr)
	var (
		file       = fs.String("file", "", "load a specific file")
		directory  = fs.String("directory", "", "load every inventory file from a directory")
		recursive  = fs.Bool("recursive", false, "recurse into subdirectories")
		stdin      = fs.Bool("stdin", false, "read data from STDIN")
		url        = fs.String("url", "", "server URL")
		useragent  = fs.String("useragent", "GLPI-Injector", "HTTP User-Agent for POST")
		noComp     = fs.Bool("no-compression", false, "don't compress sent inventories")
		noSSLCheck = fs.Bool("no-ssl-check", false, "do not check server SSL certificate")
		remove     = fs.Bool("remove", false, "remove successfully injected files")
		verbose    = fs.Bool("verbose", false, "verbose mode")
	)
	// Short aliases mirroring bin/glpi-injector's Getopt bundling.
	fs.StringVar(file, "f", "", "alias for --file")
	fs.StringVar(directory, "d", "", "alias for --directory")
	fs.StringVar(url, "u", "", "alias for --url")
	fs.BoolVar(recursive, "R", false, "alias for --recursive")
	fs.BoolVar(noComp, "C", false, "alias for --no-compression")
	fs.BoolVar(remove, "r", false, "alias for --remove")
	fs.BoolVar(verbose, "v", false, "alias for --verbose")
	fs.Usage = func() {
		fmt.Fprintln(stderr, "Usage: glpi-agent inject (-f <file> | -d <dir> | --stdin) -u <url>")
		fs.PrintDefaults()
	}
	if err := fs.Parse(args); err != nil {
		return 2
	}
	if *url == "" {
		fmt.Fprintln(stderr, "no server URL given (-u/--url), aborting")
		return 2
	}

	inj := &injector{
		url: *url, useragent: *useragent, noComp: *noComp,
		noSSLCheck: *noSSLCheck, remove: *remove, verbose: *verbose,
		stdout: stdout, stderr: stderr,
	}

	var failed []string
	switch {
	case *stdin:
		failed = inj.loadStdin()
	case *file != "":
		failed = inj.loadFile(*file)
	case *directory != "":
		failed = inj.loadDirectory(*directory, *recursive)
	default:
		fs.Usage()
		return 2
	}

	if len(failed) > 0 {
		fmt.Fprintln(stderr, "These elements were not sent:")
		for _, f := range failed {
			fmt.Fprintln(stderr, f)
		}
		return 1
	}
	return 0
}

type injector struct {
	url, useragent                      string
	noComp, noSSLCheck, remove, verbose bool
	stdout, stderr                      io.Writer
}

func (inj *injector) loadFile(path string) []string {
	info, err := os.Stat(path)
	if err != nil || info.IsDir() {
		fmt.Fprintf(inj.stderr, "file %s does not exist\n", path)
		return []string{path}
	}
	if inj.verbose {
		fmt.Fprintf(inj.stdout, "Loading %s...", path)
	}
	data, err := os.ReadFile(path)
	if err != nil {
		fmt.Fprintf(inj.stderr, "can't open file %s: %v\n", path, err)
		return []string{path}
	}

	agentID := agentIDForFile(path)
	if inj.send(data, agentID) {
		if inj.remove {
			_ = os.Remove(path)
		}
		return nil
	}
	return []string{path}
}

func (inj *injector) loadDirectory(dir string, recursive bool) []string {
	entries, err := os.ReadDir(dir)
	if err != nil {
		fmt.Fprintf(inj.stderr, "can't open directory %s: %v\n", dir, err)
		return []string{dir}
	}
	var failed []string
	for _, e := range entries {
		full := filepath.Join(dir, e.Name())
		if e.IsDir() {
			if recursive {
				failed = append(failed, inj.loadDirectory(full, recursive)...)
			}
			continue
		}
		if hasInventoryExt(e.Name()) {
			failed = append(failed, inj.loadFile(full)...)
		}
	}
	return failed
}

func (inj *injector) loadStdin() []string {
	data, err := io.ReadAll(os.Stdin)
	if err != nil {
		return []string{"STDIN DATA"}
	}
	var agentID string
	if bytes.HasPrefix(bytes.TrimSpace(data), []byte("{")) {
		agentID = newAgentID()
	}
	if inj.send(data, agentID) {
		return nil
	}
	return []string{"STDIN DATA"}
}

// send mirrors sendContent: transparently uncompress already-zlib content, then
// POST it. Returns true on success.
func (inj *injector) send(content []byte, agentID string) bool {
	if dec, ok := tryUncompress(content); ok {
		content = dec
	}
	err := transport.Inject(transport.InjectRequest{
		URL:           inj.url,
		Content:       content,
		UserAgent:     inj.useragent,
		AgentID:       agentID,
		NoCompression: inj.noComp,
		NoSSLCheck:    inj.noSSLCheck,
	})
	if err != nil {
		fmt.Fprintf(inj.stderr, "ERROR: %v\n", err)
		return false
	}
	if inj.verbose {
		fmt.Fprintln(inj.stdout, "OK")
	}
	return true
}

// agentIDForFile derives the agent id from a filename like bin/glpi-injector:
// a UUID embedded in the name, or a fresh UUID for .json/.data files; XML files
// get none.
func agentIDForFile(path string) string {
	base := filepath.Base(path)
	if m := uuidInFilename.FindStringSubmatch(base); m != nil {
		return strings.ToLower(m[1])
	}
	lower := strings.ToLower(base)
	if strings.HasSuffix(lower, ".json") || strings.HasSuffix(lower, ".data") {
		return newAgentID()
	}
	return ""
}

func hasInventoryExt(name string) bool {
	lower := strings.ToLower(name)
	for _, ext := range []string{".data", ".json", ".ocs", ".xml"} {
		if strings.HasSuffix(lower, ext) {
			return true
		}
	}
	return false
}

// tryUncompress returns the zlib-inflated content if the input is a valid zlib
// stream, mirroring the leading uncompress() call in sendContent.
func tryUncompress(data []byte) ([]byte, bool) {
	zr, err := zlib.NewReader(bytes.NewReader(data))
	if err != nil {
		return nil, false
	}
	defer zr.Close()
	out, err := io.ReadAll(zr)
	if err != nil {
		return nil, false
	}
	return out, true
}

// newAgentID returns a lowercase RFC-4122 v4 UUID, matching Data::UUID usage in
// bin/glpi-injector.
func newAgentID() string {
	var b [16]byte
	_, _ = rand.Read(b[:])
	b[6] = (b[6] & 0x0f) | 0x40
	b[8] = (b[8] & 0x3f) | 0x80
	return fmt.Sprintf("%x-%x-%x-%x-%x", b[0:4], b[4:6], b[6:8], b[8:10], b[10:16])
}
