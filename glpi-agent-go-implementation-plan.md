<!-- SPDX-License-Identifier: GPL-2.0-only -->

# GLPI Agent — Go Implementation Plan (dual-track)

**Status:** proposed · branch `feat/go-implementation`
**Goal:** a second, independent implementation of the GLPI agent in **Go**, in the
same repository, developed in parallel to the Rust workspace so the two can be
compared and one chosen later.
**Scope:** same functional scope as the Rust agent — feature parity with the
upstream GLPI agent `1.17` line (the pin in [UPSTREAM.md](UPSTREAM.md)).

**Reference source:** the Go track is derived **exclusively from the upstream
Perl agent** (`github.com/glpi-project/glpi-agent`, pinned in
[UPSTREAM.md](UPSTREAM.md), cloned locally under `/.upstream/glpi-agent-perl`).
The Perl `lib/GLPI/Agent/**` is the authoritative specification; the Perl `t/`
tests and `resources/**` fixtures are the acceptance gate. **The Rust workspace
under [`crates/`](crates/) is not a template** — it may be consulted only as a
secondary plausibility cross-check, never copied or treated as the source of
truth.

This plan is the Go counterpart to [glpi-agent-rust-migration-plan.md](glpi-agent-rust-migration-plan.md).
It does **not** duplicate the domain analysis there; it covers how the Go track is
structured, what maps to which Perl module, and how the two tracks are kept
comparable and a decision is reached.

---

## 0. Guiding principles (read first)

1. **Derive from Perl, share the contract.** Each Go module is implemented from
   its upstream **Perl** counterpart in `/.upstream/glpi-agent-perl/lib/GLPI/Agent/**`
   and validated against the upstream **Perl `t/` tests and `resources/**`
   fixtures** — the same primary-asset principle the Rust plan uses (§7/§13).
   Both binaries share only the **GLPI wire format** and those upstream fixtures,
   *not* source. Passing the same conformance suite makes the tracks comparable
   apples-to-apples; a Rust↔Go output diff (§5) is a convenience guard, with the
   Perl fixtures — not the Rust output — as the reference truth.

2. **Process boundary, not FFI.** Rust↔Go in one binary (cgo + embedded Go
   runtime) is a known trap. If a capability is ever shared across the two tracks,
   it is over a **subprocess / IPC** seam (the agent already has the task-fork
   worker model), never FFI. FFI stays reserved for Rust↔C (`libiec61850`).

3. **Same version, same pin.** The Go binary mirrors the upstream GLPI version
   the same way Rust does — major bumped to 2 (GLPI `1.17` → `2.17.x`), per
   [UPSTREAM.md](UPSTREAM.md). Both tracks track the same upstream commit; the
   module mapping in [docs/UPSTREAM-MAPPING.md](docs/UPSTREAM-MAPPING.md) gains a
   **Go** status column alongside Rust.

4. **Front-load the decisive slice.** Don't build everything twice before
   deciding. Implement the parts where Go is expected to win first
   (vSphere via `govmomi`, the SSH stack, cross-compile/packaging), get a real
   signal early, then commit or stop (§8).

---

## 1. Repository layout

The Cargo workspace and a Go module coexist with separate toolchains:

```
/Cargo.toml, /crates/…          ← Rust track (existing)
/go/                            ← Go track (this plan)
  go.mod                        module path: github.com/glpi-project/glpi-agent/go
  cmd/glpi-agent/main.go        single binary, subcommands
  internal/
    cli/                        arg parsing + dispatch  (≈ bin/glpi-agent + Config)
    version/                    version string (mirrors 2.17.0; ≈ GLPI::Agent::Version)
    config/                     layered config           (≈ GLPI::Agent::Config)
    logging/                    structured logging slog  (≈ GLPI::Agent::Logger)
    content/                    GLPI content model + submit (≈ GLPI::Agent::Protocol::*, XML.pm)
    transport/                  GLPI HTTP client, OAuth/TLS (≈ GLPI::Agent::HTTP::Client::*)
    inventory/                  local inventory + categories (≈ GLPI::Agent::Task::Inventory)
      linux/  windows/  macos/  per-OS collectors  (≈ Inventory/{Linux,Win32,MacOS,Generic})
    discovery/                  NetDiscovery + NetInventory + MIBs (≈ Task::{NetDiscovery,NetInventory})
      mib/                      standard + vendor MIB support (≈ GLPI::Agent::SNMP::MibSupport)
    remote/                     SSH / WinRM remote inventory (≈ GLPI::Agent::Task::RemoteInventory)
    vsphere/                    ESX/vCenter (≈ GLPI::Agent::Task::ESX + SOAP)
    collect/  deploy/  wakeonlan/  scheduler/  httpd/
                                (≈ Task::{Collect,Deploy,WakeOnLan}, Daemon, HTTP::Server)
/docs, /packaging, /tests       ← shared
```

Module path `github.com/glpi-project/glpi-agent/go` keeps imports stable even
though the build is local; internal packages are imported as
`…/glpi-agent/go/internal/<pkg>`.

## 2. Toolchain & dependencies

- **Go:** pinned via `go.mod` (currently `go 1.25.0`, raised from the initial
  1.23 by the `govmomi` SDK requirement), mirroring how `rust-toolchain.toml`
  pins Rust.
- **License compatibility:** every dependency must be GPL-2.0-compatible (same
  bar as the Rust side — avoid Apache-2.0-only where it would conflict; most of
  the libraries below are BSD/MIT).

| Domain | Go library | Notes |
| --- | --- | --- |
| CLI | `spf13/cobra` | subcommand ergonomics ≈ clap |
| SNMP v1/v2c/v3 | `gosnmp/gosnmp` | full auth/priv matrix; replaces `snmp2` |
| SSH | `golang.org/x/crypto/ssh` | stable; avoids the rc.* crypto churn Rust hit |
| WinRM | `masterzen/winrm` | Windows remote inventory |
| WMI (Windows) | `go-ole/go-ole` (+ `microsoft/wmi`) | syscall-based, no cgo |
| vSphere | `vmware/govmomi` | **official** VMware SDK — key Go advantage |
| HTTP server | stdlib `net/http` | control server + proxy |
| Config | hand-rolled layering (+ `x/sys/windows/registry`) | match the documented precedence incl. the Windows registry source Rust still lacks |
| Logging | stdlib `log/slog` | structured; stderr/file/syslog backends |
| WoL / ping | stdlib `net` / `x/net/icmp` | |
| IEC 61850 | cgo → `libiec61850` | behind a build tag, **off by default** (cgo undermines cross-compile, so keep it optional exactly like the Rust feature) |

Keep the dependency set lean; prefer the stdlib where it is enough (HTTP, JSON,
XML, WoL).

## 3. Perl module → Go package mapping

The reference is the upstream Perl tree at `/.upstream/glpi-agent-perl`. Each Go
package is implemented from the listed Perl module(s); the Rust crate is named
only for orientation (secondary cross-check, not a source — §0).

| Upstream Perl module(s) | Go package | Rust crate (cross-check only) | Notes |
| --- | --- | --- | --- |
| `Protocol/**`, `XML.pm`, `Inventory.pm` | `internal/content` | `glpi-core` | GLPI content/section model with exact JSON/XML keys |
| `Config.pm`, `Config/**` | `internal/config` | `glpi-core` | layered config + documented precedence (incl. Windows registry) |
| `Logger.pm`, `Logger/**` | `internal/logging` | `glpi-core` | stderr/file/syslog backends |
| `HTTP/Client.pm`, `HTTP/Client/**` | `internal/transport` | `glpi-transport` | GLPI HTTP client; user-agent / `versionclient` = `GLPI-Agent_v2.17.0` (Perl `$AGENT_STRING` = `$PROVIDER-Agent_v$VERSION`) |
| `Task/Inventory.pm`, `Task/Inventory/**` | `internal/inventory` | `glpi-inventory-local` | per-OS subpackages (`Linux/`, `Win32/`, `MacOS/`, `Generic/`, `Virtualization/`) |
| `Task/NetDiscovery*`, `Task/NetInventory*`, `SNMP/**` | `internal/discovery` (+ `mib/`) | `glpi-discovery` | NetDiscovery/NetInventory; `SNMP/MibSupport/**` (~80 vendor MIBs) → `mib/` |
| `Task/RemoteInventory*` | `internal/remote` | `glpi-inventory-remote` | SSH (x/crypto/ssh) + WinRM |
| `Task/ESX*`, `SOAP/**` | `internal/vsphere` | `glpi-vsphere` | govmomi |
| `Task/Collect*` / `Task/Deploy*` | `internal/{collect,deploy}` | `glpi-collect` / `glpi-deploy` | |
| `Task/WakeOnLan*` | `internal/wakeonlan` | `glpi-wakeonlan` | |
| `Daemon*`, `HTTP/Server*`, `Target*` | `internal/{scheduler,httpd}` | `glpi-scheduler` / `glpi-http` / `glpi-plugins` | daemon + control server |
| `bin/glpi-agent` (+ `bin/glpi-*`) | `cmd/glpi-agent` + `internal/cli` | `glpi-cli` | same subcommands |
| `IEC61850/**` | `internal/iec61850` (build tag) | `glpi-iec61850` | optional, cgo |

## 4. Phased plan

Phases mirror the Rust phase numbers (see the README phase table) so progress
cross-references cleanly. **Acceptance for every phase: the Go output passes the
same golden fixtures as the Rust track** (§5), and the migrated upstream tests
for that module pass — a module is "done" only then (same gate as Rust, §7 of the
Rust plan).

- **Phase 1 — Foundation.** `go.mod`, `cmd/glpi-agent` skeleton, `internal/cli`
  with all subcommands wired (most returning "not implemented" initially),
  `internal/content` (the GLPI section model with exact JSON keys), `internal/transport`
  (GLPI HTTP client, `versionclient`/user-agent), `internal/config` + `internal/logging`.
  Deliverable: `glpi-agent --version`, `inventory` emitting a minimal valid
  document, `wakeup` and `inject` fully working (both are self-contained).
- **Phase 2–3 — NetDiscovery + NetInventory.** `gosnmp`; standard MIBs first,
  then the vendor `mib/` tail (port from upstream Perl
  `lib/GLPI/Agent/SNMP/MibSupport/**`, ~80 modules).
- **Phase 4 — IEC 61850.** scan + SNMP merge; the cgo `libiec61850` path behind a
  build tag, off by default.
- **Phase 5 — CLI + daemon + HTTP control server + plugins.** `internal/{scheduler,httpd}`.
- **Phase 6 — Local inventory.** all categories per OS (`internal/inventory/{linux,windows,macos}`);
  WMI via go-ole on Windows. Front candidate to also close gaps Rust has (DRIVES,
  LVM, INPUTS/PORTS/SLOTS, …) — see [docs/UPSTREAM-MAPPING.md](docs/UPSTREAM-MAPPING.md).
- **Phase 7 — Remote inventory.** SSH (x/crypto/ssh) modes + WinRM; delta + cleanup.
- **Phase 8 — vSphere/ESX.** `govmomi` (expected to be markedly simpler than the
  hand-rolled Rust path).
- **Phase 9 — Collect, Deploy, WakeOnLan.**
- **Phase 10 — Stabilization + packaging.** Go's static cross-compile (`GOOS/GOARCH`)
  for the same target matrix; deb/rpm/msi/pkg + snap/flatpak.

**Bake-off ordering (§8):** to get a decision signal fast, do Phase 1, then a
vertical slice of **Phase 8 (vSphere)**, the **Phase 7 SSH** path, and the
**Phase 10 packaging** spike *before* grinding through the long category/MIB tail.

## 5. Conformance & testing

- The reference truth is the upstream **Perl** behaviour. Upstream `t/` tests and
  `resources/**` fixtures (from `/.upstream/glpi-agent-perl`) are reused verbatim
  as Go test data — same primary-asset principle as the Rust plan §7/§13. A
  module is "done" only when its migrated upstream test passes.
- The Go track adds a thin harness that runs the Go binary against those inputs
  and diffs the normalized JSON/XML against the upstream fixtures.
- A normalized-diff check comparing **Go output ↔ Rust output** on the shared
  fixtures is kept as a *convenience* regression guard, but it is **not**
  authoritative: where the two disagree, the upstream Perl fixture decides, and a
  Go/Rust mismatch is a flag to investigate which (possibly both) deviates from
  Perl.

## 6. CI & packaging

- A `go` job runs `go build ./...`, `go vet`, `gofmt -l`, `go test ./...` on
  Linux/Windows/macOS — mirroring [.github/workflows/ci.yml](.github/workflows/ci.yml).
- A parallel release track builds the Go binaries for the same target matrix.
  Go's cgo-free static builds make the cross-compile and packaging materially
  simpler than the Rust pipeline — itself a data point for the decision.

## 7. Governance / keeping the tracks in sync

- [docs/UPSTREAM-MAPPING.md](docs/UPSTREAM-MAPPING.md) gains a **Go** status
  column next to Rust, so parity of both tracks against upstream is visible.
- The version (2.x mirror) and the upstream pin are bumped for **both** tracks in
  the same PR (already the pin-bump checklist in [UPSTREAM.md](UPSTREAM.md)).

## 8. Decision criteria & deadline (the actual point of the dual track)

The dual track exists to make a **decision**, not to ship two agents forever. Set
the checkpoint now:

- **Decision checkpoint:** after Phase 1 + the bake-off slice (vSphere + SSH +
  packaging) is complete on at least Linux + one of Windows/macOS.
- **Measured criteria:**
  1. build & packaging effort (CI complexity, cross-compile friction),
  2. binary size and idle-daemon RAM,
  3. implementation velocity (time/LOC to reach the slice),
  4. library friction (did `govmomi`/`gosnmp`/`x/crypto/ssh` remove the
     vSphere/SNMP/SSH pain points the Rust side hit?),
  5. contributor onboarding,
  6. conformance: identical golden output to Rust.
- **Outcome:** either commit to the Go track and wind the Rust one down, keep
  Rust and archive the Go spike, or (only with explicit justification) keep both.
  Capture the decision as an ADR under [docs/adr/](docs/adr/), updating
  [ADR-001](docs/adr/ADR-001-use-rust-for-glpi-agent-rewrite.md).

## 9. Risks

| Risk | Mitigation |
| --- | --- |
| Two half-finished agents, no decision | Hard checkpoint + criteria (§8); front-load the decisive slice, not the long tail |
| Team split too thin across stacks | Time-box the Go spike; don't mirror the whole Rust tree before the checkpoint |
| Output drift between tracks | Shared golden fixtures + a Rust==Go diff check (§5) |
| cgo (IEC 61850) erodes Go's cross-compile edge | Keep it an optional, build-tagged, off-by-default module |
| Dependency licensing | GPL-2.0-compatibility gate on every module (§2) |
| Mapping rot | Go column in UPSTREAM-MAPPING.md reconciled on every pin bump |
