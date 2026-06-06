// SPDX-License-Identifier: GPL-2.0-only

// Package version holds the agent version and the derived version/user-agent
// strings.
//
// Derived from the upstream Perl modules lib/GLPI/Agent/Version.pm and the
// _versionString / $AGENT_STRING / $VERSION_STRING logic at the top of
// lib/GLPI/Agent.pm. The numeric version mirrors upstream with the major bumped
// to 2 (GLPI 1.17 line -> 2.17.0), per ../../UPSTREAM.md.
package version

import "regexp"

const (
	// Version mirrors the upstream GLPI agent version with the major bumped to
	// 2 to separate it from the Perl 1.x line (UPSTREAM.md). Upstream pin is the
	// 1.17 line, so this is 2.17.0.
	Version = "2.17.0"

	// Provider matches Perl $GLPI::Agent::Version::PROVIDER. Packagers may
	// override it; kept as a const here until packaging needs otherwise.
	Provider = "GLPI"
)

// Comments mirrors Perl $GLPI::Agent::Version::COMMENTS: free-form lines printed
// after the version string by `--version` (and logged). Empty for a release.
var Comments []string

// devReleasePattern mirrors the regex in GLPI::Agent::_versionString that marks
// a version as a development release.
var devReleasePattern = regexp.MustCompile(`^\d+\.\d+\.(99\d\d|\d+-dev|.*-build-?\d+)$`)

// String mirrors Perl $VERSION_STRING: "<PROVIDER> Agent (<VERSION>)".
func String() string {
	return Provider + " Agent (" + Version + ")"
}

// AgentString mirrors Perl $AGENT_STRING: "<PROVIDER>-Agent_v<VERSION>". This is
// the HTTP User-Agent and the inventory VERSIONCLIENT value (see
// lib/GLPI/Agent/HTTP/Client.pm and lib/GLPI/Agent/Inventory.pm).
func AgentString() string {
	return Provider + "-Agent_v" + Version
}

// IsDevRelease reports whether Version is a development release, matching the
// check in GLPI::Agent::_versionString that prepends a dev-release notice.
func IsDevRelease() bool {
	return devReleasePattern.MatchString(Version)
}

// VersionLines returns the lines `--version` prints: the version string, a
// dev-release banner if applicable (prepended, as Perl does), then any
// build comments. Mirrors the `--version` branch in bin/glpi-agent.
func VersionLines() []string {
	lines := []string{String()}
	comments := Comments
	if IsDevRelease() {
		comments = append([]string{"** THIS IS A DEVELOPMENT RELEASE **"}, comments...)
	}
	return append(lines, comments...)
}
