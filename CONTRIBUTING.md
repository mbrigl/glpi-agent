# Contributing to GLPI Agent Rust

Thank you for contributing to the **GLPI Agent Rust** rewrite! This document provides guidelines for **human developers** and serves as a complement to [AGENTS.md](AGENTS.md) (which is targeted at AI coding agents).

## Getting Started

### Prerequisites

- **Rust**: 1.96.0 (pinned in `rust-toolchain.toml`)
- **Docker**: For the development environment
- **Git**: For version control

### Setup

```bash
# Clone the repository
git clone https://github.com/glpi-project/glpi-agent-rust.git
cd glpi-agent-rust

# Build all crates
cargo build --workspace

# Run all tests
cargo test --workspace

# Check formatting
cargo fmt --check

# Run clippy
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

## Development Environment

The project uses a **Docker-based devcontainer** for consistent development across platforms. This is the **recommended** way to work on the project.

### Using VS Code with Devcontainers

1. Install the [Remote - Containers](https://marketplace.visualstudio.com/items?itemName=ms-vscode-remote.remote-containers) extension
2. Open the project in VS Code
3. When prompted, "Reopen in Container" or run:
   - `Ctrl+Shift+P` > "Remote-Containers: Reopen in Container"

### Using Docker Compose Manually

```bash
cd .devcontainer

# Start the full stack (GLPI + MySQL)
docker compose up -d

# Start the Perl agent (for reference)
docker compose up -d agent

# View logs
docker compose logs -f agent

# Stop everything
docker compose down
```

**Services**:
- **GLPI**: http://localhost:8080
- **MySQL**: localhost:3306 (credentials in `.devcontainer/.env`)
- **Perl Agent**: Runs `glpi-agent --no-fork --debug` in a loop

**Important**: Before agents can report, enable inventory in the GLPI UI under **Setup / Settings → Inventory**.

## Project Structure

```
glpi-agent/
├── Cargo.toml                    # Workspace manifest
├── rust-toolchain.toml           # Rust version pin
│
├── crates/                       # Cargo workspace members
│   ├── glpi-core/                # Foundation: types, protocol, config, logging
│   ├── glpi-transport/           # HTTP client, glpi-injector
│   │
│   ├── glpi-discovery/           # Network discovery + SNMP inventory
│   ├── glpi-inventory-local/     # Local system inventory
│   ├── glpi-inventory-remote/    # SSH, WinRM, remote inventory
│   ├── glpi-vsphere/             # VMware ESX/vCenter support
│   ├── glpi-iec61850/            # IEC 61850/OT devices
│   │
│   ├── glpi-cli/                 # CLI binary (glpi-agent)
│   ├── glpi-scheduler/           # Daemon scheduling
│   ├── glpi-http/                # Embedded HTTP server (ToolBox)
│   ├── glpi-collect/             # Collect task
│   ├── glpi-deploy/              # Deploy task
│   ├── glpi-wakeonlan/           # WakeOnLan task
│   ├── glpi-plugins/             # Plugin system
│   └── glpi-agent-tests/         # Cross-crate integration & parity tests
│
├── docs/
│   └── adr/                      # Architecture Decision Records
│
├── .agents/                      # AI agent configurations
│   ├── CLAUDE.md                # Claude Code instructions
│   ├── COPILOT.md               # GitHub Copilot instructions
│   ├── CURSOR.md                # Cursor instructions
│   ├── DEVSTRAL.md              # Devstral/Mistral instructions
│   └── VSCODE.md                # VS Code AI instructions
│
├── .devcontainer/                # Development environment
│   ├── devcontainer.json
│   ├── Dockerfile
│   └── docker-compose.yml
│
├── AGENTS.md                     # AI agent guidelines
└── CONTRIBUTING.md               # This file
```

## Coding Guidelines

### Language
- **All code and documentation must be in English** (see [AGENTS.md](AGENTS.md#language-policy))

### Code Style
- Follow [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- Use `cargo fmt` for formatting
- All public items must have doc comments (`///` or `//!`)
- Use `snake_case` for variables and functions
- Use `PascalCase` for types, traits, and enums
- Use `SCREAMING_SNAKE_CASE` for constants and statics

### Error Handling
- Use `glpi_core::error::Result` and `AgentError` for most errors
- Use `anyhow::Result` for binary entry points and top-level errors
- Provide context in error messages (avoid generic errors)

### Logging
- Use the `tracing` crate macros: `info!`, `debug!`, `warn!`, `error!`
- Log at appropriate levels:
  - `error!`: Fatal errors, unrecoverable conditions
  - `warn!`: Recoverable errors, deprecated features
  - `info!`: High-level progress, significant events
  - `debug!`: Detailed diagnostic information
  - `trace!`: Very verbose, low-level details

### Testing
- Write tests for all new functionality
- Use `rstest` for parameterized tests
- Use `insta` for snapshot/golden-file tests
- Use `wiremock` for HTTP mock tests
- Place tests in the same file as the code they test (for unit tests) or in a `tests/` directory (for integration tests)

### Documentation
- Document all public APIs with `///` doc comments
- Use Markdown in doc comments (supports code blocks, lists, etc.)
- Include examples in doc comments where helpful
- Keep `README.md` and `docs/` up to date

## Pull Request Guidelines

### Before Submitting
1. Run `cargo test --workspace` (all tests pass)
2. Run `cargo fmt --check` (formatting is correct)
3. Run `cargo clippy --workspace --all-targets -- -D warnings` (no clippy warnings)
4. Add tests for new functionality
5. Update documentation if needed

### PR Description
- Use [Conventional Commits](https://www.conventionalcommits.org/) format for the title
- Include:
  - What was changed and why
  - Any breaking changes
  - References to relevant issues or ADRs
  - Screenshots or examples if applicable
- For AI-generated changes, include `Generated by Mistral Vibe.`

### Review Process
1. All CI checks must pass
2. At least one maintainer must approve
3. Address all review comments
4. Squash and merge (or rebase and merge for clean history)

## Phase-Based Contributions

The project follows a **phased migration strategy** (see [ADR-006](docs/adr/ADR-006-phased-migration-strategy.md)). Contributions should align with the current phase priorities:

| Phase | Status | Focus Areas |
|-------|--------|-------------|
| Phase 1 | ✅ Complete | Foundation (glpi-core, glpi-transport) |
| Phase 2 | ✅ Complete | NetDiscovery core + SNMP stack |
| Phase 3 | ✅ Core complete | NetInventory + MIBs (vendor-MIB tail grows) |
| Phase 4 | ✅ Complete | IEC 61850 (libiec61850 FFI behind a feature) |
| Phase 5 | ✅ Complete | CLI + daemon + HTTP (ToolBox UI pages pending) |
| Phase 6 | ✅ Linux/Windows/macOS | Local inventory (exotic Unix pending) |
| Phase 7 | ✅ Near complete | Remote inventory (SSH 1–3, WinRM incl. Windows WMI) |
| Phase 8 | ✅ Complete | vSphere / ESX |
| Phase 9 | ✅ Complete | Collect, Deploy, WakeOnLan |
| Phase 10 | 🟡 In Progress | Stabilization + packaging (installers shipped) |

**Priority**: the most valuable open work is the **ToolBox HTTP UI pages**
(Phase 5 tail, incl. the IEC 61850 config page), **exotic-Unix inventory**
(Solaris/HP-UX/AIX/FreeBSD, Phase 6), more **vendor MIBs** (Phase 3), and the
**Phase 10 audit/docs tail** (live SNMPv3 round-trip, security audit, man pages,
coverage gate). See [AGENTS.md](AGENTS.md) for the current per-crate breakdown.

## Common Contribution Areas

### 1. Vendor MIB Implementation
Help port vendor-specific MIBs from the Perl agent:

1. Find the Perl MIB module in `GLPI::Agent::SNMP::MibSupport::*`
2. Create a new file in `crates/glpi-discovery/src/snmp/mib/vendor/`
3. Follow the pattern from existing MIBs (e.g., `xerox.rs`)
4. Add tests using `WalkSession` with fixture data
5. Submit PR with `[MIB] Vendor: Add support for X`

**High-priority vendors**: HP, Brother, Lexmark, Dell Networking, Aruba, Palo Alto

### 2. Inventory Collectors (the seam)

Linux, Windows and macOS collectors already exist for every category. New work
follows the seam in [ADR-009](docs/adr/ADR-009-cross-platform-inventory-collection.md):
a `#[cfg(target_os = "…")]` `collect()` runs a system tool and feeds a **pure
`parse_*` function** that is unit-tested on Linux against captured fixtures.

1. Add a `parse_win_*` / `parse_macos_*` function (pure, with a fixture test).
2. Add the platform-gated `collect()` that runs the tool
   (Windows: `crate::sys::powershell("… | ConvertTo-Json")`; macOS:
   `crate::sys::output("system_profiler"/"sysctl"/"ioreg", …)`).
3. Compile-check both targets:
   `cargo clippy --target x86_64-pc-windows-gnu` and `… x86_64-apple-darwin`.

Open areas: **exotic Unix** (Solaris/HP-UX/AIX/FreeBSD) base inventory, and —
should the PowerShell dependency need removing — native WMI on a COM worker
thread (COM is not `Send`) and macOS IOKit, behind the same parser seam.

### 3. Certificate / OT and other tasks
- Certificate inventory (Windows CNG store / macOS Keychain) — SSL/transport.
- IEC 61850: extend `glpi-iec61850` or wire a real MMS transport behind the
  `IedProtocol` seam (`libiec61850` feature).

### 4. Testing Improvements
- Add more golden-file tests
- Improve fixture coverage
- Add integration tests with mock servers

### 5. Documentation
- Improve existing docs
- Add more ADRs for architectural decisions
- Update README with usage examples

## Reporting Issues

When reporting issues:

1. Check existing issues for duplicates
2. Include:
   - Rust version (`rustc --version`)
   - OS and architecture
   - Steps to reproduce
   - Expected vs. actual behavior
   - Relevant logs (with `RUST_LOG=debug` if applicable)

## Community

- **Discussions**: [GitHub Discussions](https://github.com/glpi-project/glpi-agent-rust/discussions)
- **Issues**: [GitHub Issues](https://github.com/glpi-project/glpi-agent-rust/issues)
- **GLPI Project**: [glpi-project.org](https://glpi-project.org)

## License

By contributing to this project, you agree to license your contributions under the **GNU General Public License v2.0** (GPL-2.0-only), matching the project's existing license.

All contributions must include the SPDX license identifier:
```rust
// SPDX-License-Identifier: GPL-2.0-only
```

## Code of Conduct

This project follows the [GLPI Project Code of Conduct](https://glpi-project.org/en/conduct/). Be respectful, inclusive, and collaborative.
