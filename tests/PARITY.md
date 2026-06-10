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

> **Scope.** This audit maps the upstream `t/` families to both the **Rust** and
> the **Go** track ([ADR-011](../docs/adr/ADR-011-go-dual-track-evaluation.md)).
> The Go track mostly ships its **own** unit/end-to-end tests with **synthetic**
> fixtures (e.g. the `vcsim` ESX test, the in-process SSH-server test, the
> httptest GLPI-server dialog) rather than replaying the upstream `t/` cases. The
> Go column records that honestly per family. Replaying the real upstream
> `resources/**` captures has **begun**: the local-inventory dmidecode parser is
> now pinned against vendored upstream captures
> (`go/internal/inventory/testdata/dmidecode/`); other families follow. Go-column
> legend: ✅ own tests cover it · 🟡 own tests, partial / `resources/**` not yet
> replayed · ⬜ not tested / not implemented · 📦 real upstream fixtures vendored.

## Agent core

| Perl source | Rust target | Rust | Go | Notes |
| ----------- | ----------- | ---- | -- | ----- |
| `t/agent/config*.t` | `glpi-core` `config::{options,sources,mod}` tests | ✅ | 🟡 | Go `internal/config` own tests (layered precedence, `conf.d` include, `_checkContent`); env layer + Windows registry ⬜. |
| `t/agent/inventory.t` | `glpi-inventory-local::content` + `tests/glpi_schema.rs` | ✅ | 🟡 | Go `internal/content` own tests (envelope, lowercasing); JSON-schema parity validation ⬜. |
| `t/agent/http/*` | `glpi-http` server tests | ✅ | ✅ | Go `internal/httpd` own tests: `/status`, `/now` (trust), root page, TLS serve. CORS/event query parsing ⬜. |
| `t/agent/tools/*.t` | parser unit tests across `glpi-core` / `glpi-inventory-local` | ✅ | 🟡 | Go helpers tested next to their consumers; synthetic inputs. |
| `t/agent/protocol*.t` | `glpi-core::protocol` + `tests/golden.rs` | ✅ | 🟡 | Go `internal/protocol` own tests (CONTACT encode, answer/expiration parse); no OCS XML; golden `resources/**` not replayed. |

## NetDiscovery / NetInventory / SNMP

| Perl source | Rust target | Rust | Go | Notes |
| ----------- | ----------- | ---- | -- | ----- |
| `t/agent/snmp/*.t`, `mock.t` | `glpi-discovery::snmp::walk` tests + `tests/scanner.rs` | ✅ | 📦 | Go has a `.walk` loader (`parseWalk` + `walkGetter`, mirroring the live SNMP rendering) replaying upstream `resources/walks/*` captures (`testdata/walks/`); also a synthetic `fakeGetter` for module unit tests. More vendor walks to follow. |
| `t/tasks/netdiscovery*.t` | `glpi-discovery::tasks::net_discovery` tests | ✅ | ✅ | Go own tests: range expansion, probe merge, ARP/NetBIOS parse, threaded scan. |
| `t/tasks/netinventory*.t` | `glpi-discovery::tasks::net_inventory` + `glpi-agent-tests` | ✅ | 🟡 | Go own tests: device build, ENTITY-MIB components, port enrichment; synthetic walks. |
| per-vendor MIB device cases | `glpi-discovery::snmp::mib::vendor::*` module tests | 🟡 | 🟡 | Go ships all 78 device modules + SnmpFramework, each with a synthetic end-to-end test; **Force10S** (`force10s.walk`, 33 components vs `force10s.t`) and **Ubnt** (`sample7.walk`, WiFi radio-port SSID/band/VLAN enrichment vs `ubnt.t`) are now pinned against their real captures; the other vendor walks are still to be replayed. |
| SNMPv3 RFC 3414/7860 crypto vectors | delegated to `snmp2`; agent-side live v3 round-trip | ⬜ | 🟡 | Go `configureV3` USM mapping unit-tested; no live v3 round-trip. |

## IEC 61850

| Perl source | Rust target | Rust | Go | Notes |
| ----------- | ----------- | ---- | -- | ----- |
| `GLPI::Agent::IEC61850::{Protocol,Device}` cases | `glpi-iec61850` device/mock tests | ✅ | ⬜ | IEC 61850 is a Rust addition; not ported to Go. |
| SNMP + IEC 61850 merge output | `glpi-discovery::tasks::net_inventory::merge_ied_identity` tests | ✅ | ⬜ | n/a in Go (no IEC 61850). |

## Local inventory

| Perl source | Rust target | Rust | Go | Notes |
| ----------- | ----------- | ---- | -- | ----- |
| `t/tasks/inventory/generic/**` | `glpi-inventory-local` category tests + `tests/fixtures.rs` | ✅ | 📦 | **dmidecode, lspci and EDID now replay real upstream captures** (`testdata/{dmidecode,lspci,edid}/`, exact counts/fields vs `screen.t`/Memory/Slots expectations). Replaying lspci caught a real parser bug (header regex dropped lines with a trailing `(prog-if …)` annotation — ~⅔ of devices on some hosts). EDID pins manufacturer/caption/week-year across 6 vendors (Parse::EDID combined-serial is a noted divergence). CUPS still on synthetic samples. |
| `t/tasks/inventory/linux/**` | `glpi-inventory-local` `categories::*` tests | ✅ | 📦 | Go ~28 Linux collectors with own parser tests; **LVM, rpm and AntiVirus (Bitdefender/SentinelOne) + TeamViewer now replay the real `resources/**` captures** (`testdata/{lvm,packaging,antivirus,teamviewer}/`, pinned to the upstream `*.t` expectations). Replaying LVM caught a real bug — `ParsePVS` dropped PVs with no volume group (empty trailing `vg_uuid`, 7 fields). dpkg is not replayed (Go parses `/var/lib/dpkg/status` stanzas, upstream uses `dpkg-query`). The remaining `resources/linux/**` collectors read live /proc·sysfs and would need a fixture seam. |
| `t/tasks/inventory/win32/**` | all `…/categories/*` (Windows path) | 🟡 | ⬜ | Windows inventory not implemented in Go (hostname stub only). |
| `t/tasks/inventory/macos/**` | all `…/categories/*` (macOS path) | 🟡 | ⬜ | macOS inventory not implemented in Go. |
| `t/tasks/inventory/{hpux,aix,solaris}/**` | exotic-platform collectors | ⬜ | ⬜ | Not implemented in Go. |
| `t/tasks/inventory/virtualization/**` | virtualization detection | ⬜ | 🟡 | Go virtualization detectors (Linux hypervisors) with own tests. |

## Remote / vSphere

| Perl source | Rust target | Rust | Go | Notes |
| ----------- | ----------- | ---- | -- | ----- |
| `t/tasks/remoteinventory.t` | `glpi-inventory-remote` tests (`MockSession`) | ✅ | 🟡 | Go has an in-process SSH-server test (connect/exec/host basics); full remote inventory + WinRM ⬜. |
| ESX `*-hostfullinfo.dump` cases | `glpi-vsphere` tests + `glpi-agent-tests` | ✅ | ✅ | Go `internal/vsphere` test runs against the `vcsim` simulator (connect → retrieve → build). |

## Collect / Deploy / WakeOnLan

| Perl source | Rust target | Rust | Go | Notes |
| ----------- | ----------- | ---- | -- | ----- |
| `t/tasks/deploy/**` (incl. CheckProcessor `FileSHA512`/`FileSHA512Mismatch`) | `glpi-deploy::checks` tests | ✅ | ⬜ | Deploy task not ported to Go. |
| Collect file/registry/WMI checks | `glpi-collect` tests | ✅ | ⬜ | Collect task not ported to Go. |
| WakeOnLan packet construction | `glpi-wakeonlan::magic_packet` tests | ✅ | 🟡 | Go own test for the UDP magic packet; ethernet (raw L2) ⬜. |

## Cross-cutting (Phase 10)

| Area | Rust target | Rust | Go | Notes |
| ---- | ----------- | ---- | -- | ----- |
| Mock GLPI server round-trip | `glpi-transport/tests/client.rs`, `glpi-agent-tests` | ✅ | ✅ | Go httptest integration: CONTACT + inventory submit, agent-id persistence, zlib, error/disabled paths. |
| Mock vSphere SOAP flow | `glpi-vsphere` + `glpi-agent-tests` | ✅ | ✅ | Go runs against `vcsim`. |
| JSON/XML schema parity | golden tests across crates | ✅ | ⬜ | Go has no committed golden/schema diffs yet. |
| Performance (scan speed, RAM) | `glpi-agent-tests/tests/performance.rs` | 🟡 | ⬜ | No Go performance harness. |

## Intentionally dropped

| Perl source | Reason |
| ----------- | ------ |
| `t/` harness/bootstrap files (`00compile.t`, author/release tests) | Rust's `cargo test` + `cargo clippy`/`fmt` provide the equivalent gate; no per-file port. |
| Perl-specific module-loading and `Test::*` plumbing | Not applicable to a Rust workspace. |
| `Data::Dumper` ESX fixture format | Re-expressed as typed JSON dumps (`glpi-vsphere` `HostInfo`); upstream binary dumps were not available to import. |
