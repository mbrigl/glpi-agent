<!-- SPDX-License-Identifier: GPL-2.0-only -->

# SNMP walk fixtures

Real `snmpwalk -On` capture files **vendored verbatim from the upstream GLPI
Agent** test suite (`resources/walks/` in
[glpi-project/glpi-agent](https://github.com/glpi-project/glpi-agent), GPL-2.0,
the same licence as this project). They let the Go SNMP / MibSupport code be
exercised against the same real device dumps the Perl agent is tested on.

`walk_test.go` parses these with `parseWalk` (rendering each value as the live
gosnmp layer would) and serves them through `walkGetter`, an `SNMPGetter` backed
by the flat OID map. `walks_fixtures_test.go` then drives the relevant modules:

- `force10s.walk` → the `Force10S` getComponents accessor, asserting the same
  33 components (8 stack chassis + 24 ports + root) that the upstream
  `t/tasks/netinventory/mibsupport/force10s.t` expects.

To add coverage, copy more captures from the upstream `resources/walks/` and
drive the matching module in a new test.
