<!-- SPDX-License-Identifier: GPL-2.0-only -->

# AntiVirus fixtures

Real antivirus command-output captures **vendored verbatim from the upstream
GLPI Agent** test suite (`resources/linux/antivirus/` in
[glpi-project/glpi-agent](https://github.com/glpi-project/glpi-agent), GPL-2.0,
the same licence as this project). `antivirus_fixtures_test.go` feeds them to the
Go parsers and pins the ANTIVIRUS fields against the upstream
`t/tasks/inventory/linux/antivirus/*.t` expectations:

- `bduitool-7.0.3.2239`  → ParseBitdefender
- `sentinelone-30.1.1.10` → ParseSentinelOne
