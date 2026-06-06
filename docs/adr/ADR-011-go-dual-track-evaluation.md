# ADR-011: Go Dual-Track Evaluation (Bake-off)

## Status

🟡 Proposed

> The go/no-go itself is a maintainer decision. This ADR records the **measured
> findings** of the bake-off slice so that decision can be made on evidence; it
> does **not** unilaterally wind down the Rust track. Update the status to
> Accepted/Rejected once the maintainers decide, and reflect the outcome in
> [ADR-001](./ADR-001-use-rust-for-glpi-agent-rewrite.md).

## Context and Problem Statement

[ADR-001](./ADR-001-use-rust-for-glpi-agent-rewrite.md) chose Rust for the
rewrite. Since then a second, independent **Go** implementation was started in
the same repository (`/go`), derived **exclusively from the upstream Perl agent**
(never from the Rust code), to test — on real code, not speculation — whether Go
removes specific pain points the Rust track hit (hand-rolled vSphere SOAP, the
`russh` pre-release crypto churn, per-target cross-compile toolchains).

The [Go implementation plan](../../glpi-agent-go-implementation-plan.md) §8 set a
decision checkpoint: build **Phase 1 + the decisive slice (vSphere + SSH +
packaging)** first, then decide. That slice is now complete. This ADR captures
the result against the §8 criteria.

## What was built (the bake-off slice)

- **Phase 1 foundation:** single-binary CLI mirroring the Perl `bin/glpi-*`
  tools; `--version`; the GLPI Agent Protocol inventory document model (canonical
  UPPERCASE sections, recursive key-lowercasing per `Protocol/Message::_convert`);
  `inject` (bin/glpi-injector) and `wakeonlan` working; `config` (Config.pm
  layering + `_checkContent`) and `logging` (Logger.pm levels + Stderr/File).
- **Phase 8 vSphere/ESX** via the official **govmomi** SDK; sections/fields taken
  field-for-field from `Task/ESX` + `SOAP/VMware/Host.pm`.
- **Phase 7 SSH remote** via **`x/crypto/ssh`**; behaviour from
  `Task/RemoteInventory` + `Remote/Ssh.pm`.
- **Phase 10 cross-compile + CI** for the Rust release matrix
  (Linux/Windows/macOS × amd64/arm64).

Size of the slice: ~2,630 LOC (non-test) + ~680 LOC tests; **2 direct
dependencies** (`govmomi`, `x/crypto`); native binary ~15 MB.

## Decision Options

1. **Commit to Go**, wind the Rust track down (archive `crates/`).
2. **Keep Rust**, archive the Go spike (`/go`).
3. **Keep both** (only with explicit, ongoing justification — doubles maintenance).
4. **Defer** — keep evaluating against more of the long tail before deciding.

## Findings against the §8 criteria

| # | Criterion | Finding |
| --- | --- | --- |
| 1 | Build & packaging effort | **Go favourable.** All six targets cross-compile from one Linux host, cgo-free (`CGO_ENABLED=0`), no per-triple toolchain; CI runs the whole matrix on a single runner ([go.yml](../../.github/workflows/go.yml)). Rust installs a `rustup target` + linker per triple. OS packages (deb/rpm/msi/pkg, snap/flatpak) are **not yet** done on the Go side. |
| 2 | Binary size / idle RAM | Go binary ~15 MB (dominated by `govmomi`); Rust release binary is smaller after LTO+strip. **Idle-daemon RAM not yet measured** (Go daemon is Phase 5). |
| 3 | Implementation velocity | The decisive slice landed in 9 commits / ~2.6k LOC with 2 deps. govmomi/x-crypto mapped cleanly onto the Perl shapes (see #4). |
| 4 | Library friction | **Go favourable, the headline result.** `govmomi` exposes the same vSphere managed objects as the Perl SOAP hashes, so `Host.pm` accessors mapped field-for-field and compiled first try; **`vcsim`** gives a real end-to-end vCenter in tests with no external infra. `x/crypto/ssh` gave clean connect/auth/exec and is fully testable against an in-process SSH server. Both removed exactly the pain the Rust side hit (hand-rolled SOAP; `russh` rc.* churn). |
| 5 | Contributor onboarding | Subjective; smaller stdlib-first surface and two deps so far. Not independently assessed. |
| 6 | Conformance | Go emits the same GLPI Agent Protocol JSON (validated via unit tests and the vcsim E2E). The Perl `t/` + `resources/**` fixtures remain the reference truth; a normalized Go↔Rust diff guard is planned, **not yet wired**. |

## Recommendation (non-binding)

The bake-off met its goal: on the **decisive slice**, Go materially reduced
library and packaging friction with no conformance regressions. That is a strong
signal but **not** a complete basis to retire Rust — the long tail (local
inventory categories, the SNMP MIB set, the daemon/HTTP control server) and the
unmeasured items (idle RAM, OS packaging, onboarding) are still open.

Suggested path: **Defer the final verdict (option 4)** and extend the Go track
through one more decisive vertical — **Phase 2–3 (NetDiscovery/SNMP via
`gosnmp`)** — which is the remaining library-friction unknown, then revisit this
ADR with that data and pick option 1 or 2.

## Consequences

- While deferred, both tracks are maintained in parallel — extra cost, bounded by
  the §8 time-box and the front-loaded-slice rule (do not mirror the whole Rust
  tree before deciding).
- Parity of both tracks against upstream stays visible via the Go column/notes in
  [docs/UPSTREAM-MAPPING.md](../UPSTREAM-MAPPING.md), reconciled on every pin bump.
- When decided, this ADR moves to Accepted/Rejected and ADR-001 gains a pointer to
  the outcome.

## Alternatives Considered

See the options above. The "keep both forever" option is explicitly discouraged
by the plan (§9 risks) unless a concrete, ongoing reason exists.
