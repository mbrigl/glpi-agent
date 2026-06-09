<!-- SPDX-License-Identifier: GPL-2.0-only -->

# dmidecode fixtures

These are **real `dmidecode` capture files vendored verbatim from the upstream
GLPI Agent** test suite (`resources/generic/dmidecode/` in
[glpi-project/glpi-agent](https://github.com/glpi-project/glpi-agent), GPL-2.0,
the same licence as this project). They are used by
`dmidecode_fixtures_test.go` to exercise the Go `ParseDmidecode` /
`BuildMemories` / `BuildSlots` / `BuildPorts` parsers against the same
real-world inputs the Perl agent is tested on, rather than only synthetic
samples.

A representative spread is vendored (servers, a laptop, a VM, *BSD, Windows),
including edge cases such as a host with no port connectors. To add more, copy
the desired files from the upstream `resources/generic/dmidecode/` directory and
extend the table in the test.
