<!-- SPDX-License-Identifier: GPL-2.0-only -->

# Test-suite parity audit

This is the Phase 10 parity map required by the migration plan (§13.5): every
Perl `t/` test family in the upstream
[GLPI Agent](https://github.com/glpi-project/glpi-agent) maps to its Rust
counterpart here, or is listed as **intentionally dropped** with a reason.

It is a living document — update it whenever a test family is migrated or a new
module lands. The audit is "green" when every upstream family is either
**migrated** or **dropped (justified)**, with nothing left **pending** that
covers shipping functionality.

Legend: ✅ migrated · 🟡 partial (platform/feature gap tracked) · ⬜ pending ·
🚫 intentionally dropped.

> **Scope.** This audit currently maps the upstream `t/` families to the **Rust**
> track. The parallel **Go** track ([ADR-011](../docs/adr/ADR-011-go-dual-track-evaluation.md))
> ships its own unit/end-to-end tests (e.g. the `vcsim` ESX test, the in-process
> SSH-server test) but has **not** yet migrated the upstream `t/` families; its
> per-module status lives in the Go column/notes of
> [docs/UPSTREAM-MAPPING.md](../docs/UPSTREAM-MAPPING.md). When the Go track begins
> replaying upstream `t/`+`resources/**` fixtures, add a Go column here.

## Agent core

| Perl source | Rust target | Status | Notes |
| ----------- | ----------- | ------ | ----- |
| `t/agent/config*.t` | `glpi-core` `config::{options,sources,mod}` tests | ✅ | layered precedence, `conf.d`, `GLPI_AGENT_*`. Windows-registry source 🟡 (deferred to Phase 6b). |
| `t/agent/inventory.t` | `glpi-inventory-local::content` + `tests/glpi_schema.rs` | ✅ | content assembly + schema parity. |
| `t/agent/http/*` | `glpi-http` server tests | ✅ | `/status`, `/now` query parsing, `httpd-trust`. ToolBox UI pages ⬜ (Phase 5 tail). |
| `t/agent/tools/*.t` | parser unit tests across `glpi-core` / `glpi-inventory-local` | ✅ | normalization helpers live next to their consumers. |
| `t/agent/protocol*.t` | `glpi-core::protocol` + `tests/golden.rs` | ✅ | native JSON `contact`/`inventory`; FusionInventory XML round-trip. |

## NetDiscovery / NetInventory / SNMP

| Perl source | Rust target | Status | Notes |
| ----------- | ----------- | ------ | ----- |
| `t/agent/snmp/*.t`, `mock.t` | `glpi-discovery::snmp::walk` tests + `tests/scanner.rs` | ✅ | `WalkSession` replays `snmpwalk -On` captures. |
| `t/tasks/netdiscovery*.t` | `glpi-discovery::tasks::net_discovery` tests | ✅ | range expansion, probe merge, classification. |
| `t/tasks/netinventory*.t` | `glpi-discovery::tasks::net_inventory` + `glpi-agent-tests` | ✅ | registry-driven device build; SNMP+IEC 61850 merge. |
| per-vendor MIB device cases | `glpi-discovery::snmp::mib::vendor::*` module tests | 🟡 | 8 standard + 69 vendor MIBs shipped; the long vendor tail keeps growing (a MIB is not merged without its walk fixture + golden output). |
| SNMPv3 RFC 3414/7860 crypto vectors | delegated to `snmp2`; agent-side live v3 round-trip | ⬜ | needs a v3 target; see plan §0.1 risk note. |

## IEC 61850

| Perl source | Rust target | Status | Notes |
| ----------- | ----------- | ------ | ----- |
| `GLPI::Agent::IEC61850::{Protocol,Device}` cases | `glpi-iec61850` device/mock tests | ✅ | mock IED responses; nameplate scan. |
| SNMP + IEC 61850 merge output | `glpi-discovery::tasks::net_inventory::merge_ied_identity` tests | ✅ | golden merge (SNMP precedence + IED firmwares). |

## Local inventory

| Perl source | Rust target | Status | Notes |
| ----------- | ----------- | ------ | ----- |
| `t/tasks/inventory/generic/**` | `glpi-inventory-local` category tests + `tests/fixtures.rs` | ✅ | dmidecode, EDID, CUPS printers, … |
| `t/tasks/inventory/linux/**` | `glpi-inventory-local` `categories::*` tests | ✅ | networks, storage, distro, packages. |
| `t/tasks/inventory/win32/**` | all `…/categories/*` (Windows path) | 🟡 | Via `Get-CimInstance … \| ConvertTo-Json`, registry uninstall + EDID keys, `Win32_PnPEntity`, SecurityCenter2 (parsers tested on Linux); some detail fields best-effort. |
| `t/tasks/inventory/macos/**` | all `…/categories/*` (macOS path) | 🟡 | Via `sw_vers`/`sysctl`/`system_profiler -json`/`ifconfig`/`ioreg`/`ps`/`who`/CUPS; some detail fields best-effort. |
| `t/tasks/inventory/{hpux,aix,solaris}/**` | exotic-platform collectors | ⬜ | Phase 6c. |
| `t/tasks/inventory/virtualization/**` | virtualization detection | ⬜ | Phase 6 (medium). |

## Remote / vSphere

| Perl source | Rust target | Status | Notes |
| ----------- | ----------- | ------ | ----- |
| `t/tasks/remoteinventory.t` | `glpi-inventory-remote` tests (`MockSession`) | ✅ | SSH modes 1–3, WinRM (incl. Windows WMI via PowerShell), delta state files + 30-day cleanup. |
| ESX `*-hostfullinfo.dump` cases | `glpi-vsphere` tests + `glpi-agent-tests` | ✅ | `--dumpfile` offline replay; mock SOAP flow; golden JSON. Uses a typed dump (no Perl `Data::Dumper` fixtures available). |

## Collect / Deploy / WakeOnLan

| Perl source | Rust target | Status | Notes |
| ----------- | ----------- | ------ | ----- |
| `t/tasks/deploy/**` (incl. CheckProcessor `FileSHA512`/`FileSHA512Mismatch`) | `glpi-deploy::checks` tests | ✅ | also multipart SHA-512 assembly, executor ret-code/output matching, P2P peer enumeration. |
| Collect file/registry/WMI checks | `glpi-collect` tests | ✅ | `findFile`, checksum filters, registry/WMI seams. |
| WakeOnLan packet construction | `glpi-wakeonlan::magic_packet` tests | ✅ | 102-byte magic packet byte assertion. |

## Cross-cutting (Phase 10)

| Area | Rust target | Status | Notes |
| ---- | ----------- | ------ | ----- |
| Mock GLPI server round-trip | `glpi-transport/tests/client.rs`, `glpi-agent-tests` | ✅ | `contact`/`inventory` via wiremock. |
| Mock vSphere SOAP flow | `glpi-vsphere` + `glpi-agent-tests` | ✅ | connect → login → retrieve → logout → submit. |
| JSON/XML schema parity | golden tests across crates | ✅ | normalized `serde_json::Value` diffs against committed fixtures. |
| Performance (scan speed, RAM) | `glpi-agent-tests/tests/performance.rs` | 🟡 | CPU-bound throughput smoke test; the ≥2× and <50 MB targets need a live NetDiscovery benchmark against a real network. |

## Intentionally dropped

| Perl source | Reason |
| ----------- | ------ |
| `t/` harness/bootstrap files (`00compile.t`, author/release tests) | Rust's `cargo test` + `cargo clippy`/`fmt` provide the equivalent gate; no per-file port. |
| Perl-specific module-loading and `Test::*` plumbing | Not applicable to a Rust workspace. |
| `Data::Dumper` ESX fixture format | Re-expressed as typed JSON dumps (`glpi-vsphere` `HostInfo`); upstream binary dumps were not available to import. |
