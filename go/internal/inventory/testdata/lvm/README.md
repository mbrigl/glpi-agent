<!-- SPDX-License-Identifier: GPL-2.0-only -->

# LVM fixtures

Real `pvs` / `vgs` / `lvs` capture files **vendored verbatim from the upstream
GLPI Agent** test suite (`resources/lvm/linux/` in
[glpi-project/glpi-agent](https://github.com/glpi-project/glpi-agent), GPL-2.0,
the same licence as this project), captured with the exact `-o` column order the
Go `ParsePVS` / `ParseVGS` / `ParseLVS` parsers expect.
`lvm_fixtures_test.go` pins the parsed record counts and the first record's
fields against them.

Replaying these surfaced a real parser bug: `ParsePVS` required all 8 columns,
but a physical volume not assigned to any volume group has an empty trailing
`vg_uuid`, so its row carries only 7 fields — those PVs were silently dropped.
The parser now accepts 7-field rows and treats `VG_UUID` as optional.
