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
│   └── glpi-plugins/             # Plugin system
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
| Phase 2 | ✅ Complete | NetDiscovery Core |
| Phase 3 | 🟡 In Progress | NetInventory + MIBs |
| Phase 4 | ⏳ Pending | Platform-Specific Features |
| Phase 5 | 🟡 Partially Complete | CLI + Daemon |
| Phase 6 | 🟡 Linux Complete | Local Inventory |
| Phase 7 | ⏳ Pending | Remote Inventory |

**Priority**: Contributions to **Phase 3 (MIBs)** and **Phase 6 (Windows/macOS inventory)** are currently most valuable.

## Common Contribution Areas

### 1. Vendor MIB Implementation
Help port vendor-specific MIBs from the Perl agent:

1. Find the Perl MIB module in `GLPI::Agent::SNMP::MibSupport::*`
2. Create a new file in `crates/glpi-discovery/src/snmp/mib/vendor/`
3. Follow the pattern from existing MIBs (e.g., `xerox.rs`)
4. Add tests using `WalkSession` with fixture data
5. Submit PR with `[MIB] Vendor: Add support for X`

**High-priority vendors**: HP, Brother, Lexmark, Dell Networking, Aruba, Palo Alto

### 2. Windows Inventory Categories
Implement Windows-specific inventory categories:

1. Read the Perl implementation in `Task/Inventory/`
2. Create file in `crates/glpi-inventory-local/src/categories/`
3. Use `#[cfg(windows)]` and WMI/COM via the `windows` crate
4. Follow the pattern from existing categories

**Note**: Windows code must run on a dedicated COM worker thread (not `Send`).

### 3. macOS Inventory Categories
Similar to Windows, but using macOS-specific APIs:
- System Profiler
- IOKit
- Keychain
- CoreFoundation

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
