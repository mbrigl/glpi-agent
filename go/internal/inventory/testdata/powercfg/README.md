# powercfg battery-report fixtures

`windows-10-notebook.xml` and `win10-dell-xps.xml` are verbatim copies of the
upstream GLPI Agent (Perl) captures at `resources/win32/powercfg/`, used to pin
`parsePowercfgBatteries` against the expected values in the upstream
`t/tasks/inventory/windows/batteries.t` (`_getBatteriesFromPowercfg`).

Source: GLPI Agent (Perl), licensed GPL-2.0-or-later — same licence as this Go
track, so vendoring them here is fine.
