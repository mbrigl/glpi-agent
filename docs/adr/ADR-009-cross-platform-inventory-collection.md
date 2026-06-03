# ADR-009: Cross-Platform Local Inventory via System Tools and Pure Parsers

## Status

🟢 Accepted

## Context and Problem Statement

Phase 6 must collect a computer's inventory (OS, CPU, memory, BIOS/hardware,
storage, software, network, processes, users, printers, video, sound, USB,
batteries, PCI controllers, monitors, antivirus) on **Linux, Windows and
macOS**. Each platform exposes this data through different mechanisms:

- **Linux** — `/proc`, `/sys`, `dmidecode`, `lsblk`, `lspci`, `lsusb`, `ip`,
  `ps`, `who`, CUPS.
- **Windows** — WMI / CIM (classically via COM), the registry, EDID in the
  registry, the Security Center.
- **macOS** — `system_profiler`, `sysctl`, `ioreg`, `sw_vers`, `ifconfig`,
  `ps`, `who`, CUPS.

The upstream Perl agent reads Windows WMI through a dedicated COM worker thread
(COM apartments are not thread-safe to share) and macOS data through IOKit.
Replicating that natively in Rust means a Windows-only `wmi`/`windows` crate
dependency, `unsafe` COM glue and a worker-thread architecture — all of which is
hard to unit-test on the Linux CI host and risks shipping unverifiable code.

We needed an approach that (a) covers all three platforms, (b) keeps the
collection logic testable on any host, and (c) avoids heavy, unverifiable
platform-only dependencies.

## Decision Options

1. **Native platform APIs** — the `wmi`/`windows` crate on a COM worker thread
   (Windows) and IOKit/CoreFoundation bindings (macOS).
2. **System tools + pure parsers** — a thin, platform-gated collector runs a
   system tool and captures its (preferably JSON) output, which a pure parser
   turns into the typed category struct.
3. **A mix** — native where cheap, tools where not.

## Decision

We chose **system tools + pure parsers** (option 2), with a consistent seam per
category:

- A pure `parse_*` function takes the tool's output (`&str`) and returns the
  category struct. It is compiled on every platform and **unit-tested on Linux**
  against captured fixtures.
- A `#[cfg(target_os = "…")]` `collect()` runs the platform tool and feeds the
  parser; a final `#[cfg(not(any(linux, macos, windows)))]` stub returns empty.
- Windows uses PowerShell `Get-CimInstance <class> | ConvertTo-Json` (plus the
  registry for software/EDID and the Security Center for antivirus); macOS uses
  `system_profiler -json`, `sysctl`, `ioreg`, `sw_vers` and `ifconfig`. Both
  emit UTF-8 (JSON), so a `serde_json` helper (`crate::jsonutil`) reads fields
  uniformly and the parsers stay platform-agnostic.
- The same `parse_win_*` parsers are **reused for remote inventory over WinRM**
  (`RemoteInventory::collect_windows`): the WinRM session runs the identical
  PowerShell queries on the remote Windows host.

`Get-CimInstance` returns the same WMI data as the native COM API, so this is a
data-equivalent choice, not a reduced one.

## Consequences

### Positive

- **Testable**: every parser runs and is asserted on the Linux CI host; the
  collectors compile-check for `x86_64-pc-windows-gnu` and `x86_64-apple-darwin`
  (`cargo clippy --target …`).
- **No heavy platform dependencies**: the core crate needs only `serde_json`; no
  COM/`unsafe` for inventory, no Windows-only build for the common path.
- **Code reuse**: macOS shares the Linux parsers where the tool is the same
  (`ps`, `who`, CUPS `lpstat`); WinRM remote inventory reuses the Windows
  parsers wholesale.
- **Uniform shape**: one `parse_win_*` / `parse_macos_*` per category mirrors the
  existing Linux parsers.

### Negative

- **Runtime dependency on system tools**: PowerShell / `system_profiler` must be
  present (they ship with the OS).
- **Process-spawn overhead**: one short-lived process per query, vs. one
  in-process WMI session. Acceptable for an inventory run; revisit if profiling
  shows it matters.
- **Some detail fields are best-effort** where a tool does not expose them
  (e.g. macOS per-DIMM memory falls back to the `hw.memsize` total).

## Alternatives Considered

- **Native WMI on a COM worker thread (Windows) + IOKit (macOS).** Deferred, not
  rejected: it is a performance / no-PowerShell-dependency optimization that adds
  the same data this ADR already collects, at the cost of `unsafe` COM glue and a
  Windows-only build path that the Linux CI cannot exercise. It can be added
  later behind the same parser seam (the live `glpi-collect` Windows registry
  reader, built with `winreg`, is a first step in that direction).
- **Certificate inventory** (Windows CNG store / macOS Keychain) is part of the
  SSL/transport surface, not a local-inventory category, and is tracked there.
