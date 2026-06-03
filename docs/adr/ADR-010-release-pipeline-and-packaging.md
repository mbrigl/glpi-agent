# ADR-010: Cross-Platform Release Pipeline and Installer Formats

## Status

🟢 Accepted

## Context and Problem Statement

Phase 10 must ship the `glpi-agent` binary as native installers for the three
target operating systems, for both x86_64 and aarch64, without a release
engineer running platform-specific tooling by hand. The agent is built from a
Cargo workspace and the default build is pure Rust (the `libiec61850` FFI is an
off-by-default feature), so no C library is required to produce the binaries.

## Decision Options

1. **Tarballs only** — ship a compiled binary per target; users install it
   themselves.
2. **Native installers per platform** — `.deb`/`.rpm` (Linux), `.msi` (Windows),
   `.pkg` (macOS), built in CI.
3. **A third-party release service** (e.g. cargo-dist).

## Decision

We chose **native installers built by GitHub Actions**
([`.github/workflows/release.yml`](../../.github/workflows/release.yml)),
triggered by a `v*` tag (or manual dispatch):

- **Linux** — `.deb` (cargo-deb), `.rpm` (cargo-generate-rpm), a `.tar.gz`, plus
  an **AppImage**, a **Snap** and a **Flatpak**.
- **Windows** — `.msi` via WiX 3 (cargo-wix), with a committed `wix/main.wxs`.
- **macOS** — `.pkg` via `pkgbuild`.

A `release` job collects every artifact, writes `SHA256SUMS` and publishes the
GitHub Release. Packaging metadata lives in `crates/glpi-cli/Cargo.toml`
(`[package.metadata.{deb,generate-rpm,wix}]`).

Supporting decisions made while building this:

- **Native arch runners over cross-compilation.** aarch64 builds run on native
  ARM runners (`ubuntu-24.04-arm`, `macos-latest`, `windows-11-arm`) rather than
  cross-compiling, which avoids missing-target-std and cross-linker problems.
- **The fragile leg may fail softly.** The Windows arm64 MSI (WiX under x86
  emulation) is marked `continue-on-error` so it never blocks the release of the
  other five installers.
- **Run the async runtime on a 16 MiB-stack thread.** The agent's `main` future
  embeds every subcommand's future and overflowed Windows' default 1 MiB
  main-thread stack; the runtime is started on a dedicated big-stack thread
  instead of `#[tokio::main]`.

## Consequences

### Positive

- One `git tag` produces installers for all three platforms and both
  architectures, plus checksums, with no manual steps.
- Linux users get their distro's native format **and** the universal AppImage /
  Snap / Flatpak.
- The default build needs no C toolchain or library.

### Negative

- The CI configuration is large and depends on several community packaging
  tools; the Windows/macOS packaging steps can only be fully validated when the
  workflow runs on the hosted runners (the Linux `.deb`/`.rpm`/AppImage paths are
  reproduced and verified locally).
- Windows arm64 packaging is best-effort (emulated WiX).

## Alternatives Considered

- **Tarballs only**: simplest, but pushes installation work onto every user and
  gives no system integration (PATH entry, uninstall).
- **cargo-dist**: capable, but adds a release-tooling dependency and less control
  over the per-distro Linux formats (Snap/Flatpak) we wanted.
