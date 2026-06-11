// SPDX-License-Identifier: GPL-2.0-only
//
// Go implementation of the GLPI agent. Derived exclusively from the upstream
// Perl agent (see ../UPSTREAM.md and ../glpi-agent-go-implementation-plan.md);
// the Rust workspace under ../crates is not a source for this module.
module github.com/glpi-project/glpi-agent/go

go 1.25.0

require (
	github.com/google/uuid v1.6.0
	github.com/gosnmp/gosnmp v1.43.2
	github.com/vmware/govmomi v0.54.1
	golang.org/x/crypto v0.52.0
)

require golang.org/x/sys v0.45.0
