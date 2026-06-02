# AI Coding Agent Guidelines

**Universal guidelines for ALL AI coding agents** working on the GLPI Agent Rust project.

*For agent-specific instructions, see [.agents/](.agents/).*

---

## 🌍 Language Policy

**ALL code and documentation must be in English without exception.**

This applies to:
- Source code comments (`//`, `///`, `//!`)
- Log messages (`tracing::info!`, `tracing::warn!`, `tracing::debug!`, `tracing::error!`)
- Error messages and user-facing strings
- All documentation files
- Commit messages
- All new files

> **Rationale**: International open-source project. English ensures consistency, maintainability, and accessibility.

---

## 📋 Project Overview

### What This Is
This is a **Rust rewrite** of the [GLPI Agent](https://github.com/glpi-project/glpi-agent) (originally Perl).

### Key Facts
| Aspect | Detail |
|--------|--------|
| **Target** | Full feature parity with GLPI Agent 1.17 |
| **Version** | v2.0.0 (separate from Perl 1.x line) |
| **License** | GPL-2.0-only |
| **Rust** | 1.96.0 (pinned in `rust-toolchain.toml`) |
| **Repository** | `glpi-agent-rust` |

### Current Status (as of last commit)
| Component | Status | Notes |
|-----------|--------|-------|
| `glpi-core` | ✅ Complete | Foundation: types, protocol, config, logging |
| `glpi-transport` | ✅ Complete | HTTP client (reqwest), glpi-injector |
| `glpi-discovery` | ✅ Core | Scanner, methods (Ping, ARP, NetBIOS, SNMP) |
| Standard MIBs | ✅ Complete | 8 MIBs: system, if, entity, printer, bridge, lldp, cdp, ip |
| Vendor MIBs | 🟡 40+ | Cisco, Juniper, Fortinet, Mikrotik, QNAP, Sophos, Hikvision, Eaton, Dell, NetScaler, SonicWall, Ruckus, Zyxel, Raritan, Quantum, Infortrend, OKI, Epson, Canon, Pantum, Xerox, HP, Brother, Ricoh, Konica/Sindoh, Lexmark, D-Link, Intelbras, Tiesse, Aerohive, Telco, FoxGate, Nokia/Alcatel, WatchGuard, EMC, Hitachi Vantara, Radware, Bachmann, RNX, DigiPower |
| Local Inventory (Linux) | ✅ Complete | All 20+ categories |
| Local Inventory (Windows) | ❌ Not Started | Needs COM worker thread |
| Local Inventory (macOS) | ❌ Not Started | Needs System Profiler, IOKit |
| CLI (`glpi-agent`) | 🟡 Partial | Subcommands work |
| Daemon | 🟡 Partial | Scheduler + HTTP server core |

**Highest Priority**: Remaining vendor MIBs (network: Aruba, Netgear, CiscoUcsBoard; telephony: Avaya, Snom, Htek; LinuxAppliance, tp_link, Avocent) + Windows/macOS inventory.

---

## 🏗️ Architecture

### Workspace Structure (17 Crates)

```
crates/
├── FOUNDATION (Phase 1 ✅)
│   ├── glpi-core/           # Types, protocol, config, auth, logging
│   │   ├── types/          # Device, NetworkInterface, SnmpCredentials, InventoryCategory
│   │   ├── protocol/       # GLPI JSON (native), FusionInventory XML
│   │   ├── config/         # Layered configuration system
│   │   ├── auth/           # Basic, OAuth2, SSL
│   │   └── error.rs        # AgentError, Result type
│   │
│   └── glpi-transport/      # HTTP transport layer
│       ├── client.rs       # GlpiClient with reqwest
│       ├── auth.rs         # Authentication handlers
│       └── injector.rs     # glpi-injector functionality
│
├── NETWORK (Phase 2-3 🟡)
│   ├── glpi-discovery/      # Network discovery core
│   │   ├── ip_range.rs      # IP range expansion (CIDR, start-end)
│   │   ├── scanner.rs       # Parallel scanner with semaphores
│   │   ├── methods/         # Discovery methods
│   │   │   ├── ping.rs      # ICMP + TCP fallback
│   │   │   ├── arp.rs       # ARP cache parsing
│   │   │   ├── netbios.rs   # NetBIOS queries
│   │   │   └── snmp.rs      # SNMP detection
│   │   └── snmp/            # SNMP client & MIB framework
│   │       ├── client.rs    # Async SNMP client (snmp2 v0.5)
│   │       ├── mib/         # MIB implementations
│   │       │   ├── device.rs # Device, NetworkDevice, Printer, etc.
│   │       │   ├── mod.rs    # MIB registry
│   │       │   ├── standard/ # 8 standard MIBs
│   │       │   │   ├── system_mib.rs
│   │       │   │   ├── if_mib.rs
│   │       │   │   ├── entity_mib.rs
│   │       │   │   ├── printer_mib.rs
│   │       │   │   ├── bridge_mib.rs
│   │       │   │   ├── lldp_mib.rs
│   │       │   │   ├── cdp_mib.rs
│   │       │   │   └── ip_mib.rs
│   │       │   └── vendor/  # 40+ vendor MIBs
│   │       │       ├── cisco.rs
│   │       │       ├── juniper.rs
│   │       │       ├── fortinet.rs
│   │       │       ├── xerox.rs
│   │       │       └── ...
│   │       └── walk.rs      # WalkSession for fixture replay
│   │
│   └── glpi-inventory-remote/ # Remote inventory (Phase 7 ⏳)
│       └── glpi-vsphere/     # VMware ESX/vCenter (Phase 7 ⏳)
│
├── LOCAL INVENTORY (Phase 6 🟡)
│   └── glpi-inventory-local/
│       ├── task.rs          # Inventory task orchestration
│       └── categories/      # 20+ inventory categories
│           ├── os.rs         # OS name/version, kernel, arch
│           ├── hardware/    # BIOS, motherboard, chassis, UUID
│           ├── cpu.rs        # CPUs, cores, threads, cache
│           ├── memory.rs     # RAM modules (DMI type 17)
│           ├── storage.rs    # Disks, optical, SMART
│           ├── network.rs    # Interfaces, IP, MAC, WiFi
│           ├── software.rs   # Packages (dpkg/rpm)
│           ├── processes.rs  # Running processes
│           ├── users.rs      # Users, last logged user
│           ├── battery.rs    # Laptop batteries
│           ├── timezone.rs   # System timezone
│           ├── environment.rs# Environment variables
│           ├── controllers/  # PCI, USB, video, sound
│           ├── antivirus/    # AV detection (Linux/Windows/macOS)
│           ├── printer.rs    # Local printers (CUPS)
│           └── monitor.rs     # Monitors via EDID
│
└── DAEMON & CLI (Phase 5 🟡)
    ├── glpi-cli/          # CLI binary (published as glpi-agent)
    │   └── src/main.rs    # Subcommands: inventory, netdiscovery, netinventory, inject, daemon
    ├── glpi-scheduler/    # Daemon scheduling
    ├── glpi-http/         # Embedded HTTP server (ToolBox)
    ├── glpi-collect/      # Collect task v3.0
    ├── glpi-deploy/       # Deploy task v3.5
    ├── glpi-wakeonlan/    # WakeOnLan task
    └── glpi-plugins/      # Plugin system (HTTP, Proxy, SSL)
```

### Key Technology Stack

| Purpose | Technology | Version | Notes |
|---------|------------|---------|-------|
| **Language** | Rust | 1.96.0 | Pinned via rust-toolchain.toml |
| **Async Runtime** | Tokio | v1.x | See [ADR-005](docs/adr/ADR-005-tokio-async-runtime.md) |
| **HTTP Client** | reqwest | v0.12 | rustls backend, no native-tls |
| **HTTP Server** | axum | v0.7 | For embedded ToolBox server |
| **SNMP** | snmp2 | v0.5 | Async via tokio feature, see [ADR-003](docs/adr/ADR-003-snmp-stack-selection.md) |
| **Serialization** | serde + serde_json | v1.x | JSON serialization |
| **XML** | quick-xml | v0.36 | FusionInventory XML support |
| **CLI** | clap | v4.x | Argument parsing |
| **Config** | config | v0.14 | Base, + custom sources, see [ADR-004](docs/adr/ADR-004-configuration-layering.md) |
| **Logging** | tracing | v0.1 | Structured logging framework |
| **Testing** | rstest, insta, wiremock | latest | Parameterized, snapshot, mock tests |
| **Errors** | thiserror + anyhow | v1.x | Error types + dynamic errors |

---

## 🐳 Development Environment (Devcontainer)

The project uses a **Docker-based devcontainer** for consistent development.

### Quick Start

1. **VS Code with Remote-Containers extension** (recommended):
   - Open project in VS Code
   - Prompt will appear: "Reopen in Container"
   - Accept to build and start the devcontainer

2. **Manual Docker Compose**:
   ```bash
   cd .devcontainer
   docker compose up -d
   ```

### Services

| Service | Port | Role | URL |
|---------|------|------|-----|
| glpi | :8080 | GLPI server | http://localhost:8080 |
| mysql | :3306 | MySQL database | - |
| agent | - | Perl glpi-agent (reference) | - |

**Important**: The Perl agent is **NOT** started automatically with the devcontainer. Start it explicitly:
```bash
cd .devcontainer
docker compose up -d agent
```

### Devcontainer Commands

```bash
# Start all services (GLPI + MySQL)
docker compose up -d

# Start Perl agent for reference
docker compose up -d agent

# Tail Perl agent logs (inventory runs)
docker compose logs -f agent

# Run inventory as JSON without sending to server
docker compose exec agent glpi-inventory --json

# View GLPI UI
# Open: http://localhost:8080

# Tear everything down
docker compose down

# Tear down AND remove volumes (data loss!)
docker compose down -v
```

**Note**: From inside the devcontainer network, use `http://glpi` instead of `http://localhost` to reach the GLPI server.

### Devcontainer Files

| File | Purpose |
|------|---------|
| `devcontainer.json` | VS Code devcontainer configuration |
| `Dockerfile` | Rust development environment |
| `docker-compose.yml` | Service definitions (GLPI, MySQL, agent) |
| `.env` | Environment variables for services |
| `README.md` | Detailed devcontainer documentation |

---

## 🚀 Workflow (Universal for All Agents)

### Before Coding

1. **Read the ADRs** in [docs/adr/](docs/adr/) for architectural context
2. **Search for patterns** using your agent's search capabilities:
   - Look for similar implementations
   - Check existing tests
   - Review the crate structure

### Step-by-Step Process

1. **Analyze**:
   - Read relevant ADRs
   - Identify the target crate(s)
   - Find similar existing code

2. **Implement**:
   - Follow existing patterns
   - Match code style (indentation, naming, error handling)
   - Add SPDX license header: `// SPDX-License-Identifier: GPL-2.0-only`

3. **Test**:
   - Add tests for new functionality
   - Run crate-specific tests: `cargo test -p <crate>`
   - Verify formatting: `cargo fmt --check`
   - Run clippy: `cargo clippy --workspace --all-targets -- -D warnings`

4. **Document**:
   - Add doc comments (`///` for functions, `//!` for modules)
   - Update ADRs for major architectural decisions

### Critical Rules (Apply to All Agents)

1. **Read Before You Edit**: Never modify a file you haven't read in this session
2. **Minimal Changes**: Only change what's necessary
3. **Match Existing Style**: Follow established patterns and conventions
4. **Test Your Changes**: Relevant tests must pass
5. **Prove It Works**: Code must run and produce expected output

---

## 📚 Common Patterns & Code Examples

### Pattern 1: Adding a Vendor MIB

**Location**: `crates/glpi-discovery/src/snmp/mib/vendor/`

**Template**:
```rust
// SPDX-License-Identifier: GPL-2.0-only

//! [Vendor Name] printer/vendor MIB support.
//!
//! Applies to [Vendor] devices (`[OID]`).
//! Reads [specific information] from [MIB module].
//! Ported from upstream Perl `GLPI::Agent::SNMP::MibSupport::[Vendor]`.

use async_trait::async_trait;
use glpi_core::error::Result;

use crate::snmp::mib::{get_number, get_string, sysobjectid_matches, DeviceInfo, MibSupport, NetworkDevice};
use crate::snmp::query::SnmpQuery;

/// [Vendor] base OID from IANA enterprise numbers
const VENDOR_OID: &str = "1.3.6.1.4.1.XXXX";

// Specific OIDs for this vendor
const VENDOR_SERIAL: [u64; 8] = [1, 3, 6, 1, 4, 1, XXXX, 1, 2, 3, 4, 5];
const VENDOR_MODEL: [u64; 8] = [1, 3, 6, 1, 4, 1, XXXX, 1, 2, 3, 4, 6];

/// Vendor MIB module for [Vendor] devices
#[derive(Debug, Default, Clone, Copy)]
pub struct VendorMib;

#[async_trait]
impl MibSupport for VendorMib {
    fn name(&self) -> &'static str {
        "vendor-name"
    }

    fn applies_to(&self, info: &DeviceInfo) -> bool {
        sysobjectid_matches(info.sys_object_id.as_deref(), VENDOR_OID)
    }

    async fn run(&self, session: &mut dyn SnmpQuery, device: &mut NetworkDevice) -> Result<()> {
        // Read device information
        if let Some(serial) = get_string(session, &VENDOR_SERIAL).await? {
            device.info.serial = device.info.serial.or(Some(serial));
        }
        
        if let Some(model) = get_string(session, &VENDOR_MODEL).await? {
            device.info.model = device.info.model.or(Some(model));
        }
        
        // Add more vendor-specific data collection here
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snmp::mib::DeviceInfo;
    use crate::snmp::walk::WalkSession;

    #[test]
    fn applies_only_to_vendor() {
        // Should match vendor OID
        assert!(VendorMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.XXXX.1.2.3".to_owned()),
            ..DeviceInfo::default()
        }));
        
        // Should NOT match other vendors
        assert!(!VendorMib.applies_to(&DeviceInfo {
            sys_object_id: Some("1.3.6.1.4.1.9999.1.2.3".to_owned()),
            ..DeviceInfo::default()
        }));
    }

    #[tokio::test]
    async fn reads_device_info() {
        let mut session = WalkSession::parse(
            "\n
            .1.3.6.1.4.1.XXXX.1.2.3.4.5 = STRING: SER12345678\n
            .1.3.6.1.4.1.XXXX.1.2.3.4.6 = STRING: Model X1000\n
            ",
        ).unwrap();
        
        let mut device = NetworkDevice::default();
        VendorMib.run(&mut session, &mut device).await.unwrap();
        
        assert_eq!(device.info.serial, Some("SER12345678".to_owned()));
        assert_eq!(device.info.model, Some("Model X1000".to_owned()));
    }
}
```

**Registration** (in `crates/glpi-discovery/src/snmp/mib/mod.rs`):
```rust
pub use vendor::vendor_name::VendorMib;  // Add this line
```

And in the MIB registry:
```rust
MibRegistry::with_defaults()
    .with_vendor(VendorMib)  // Add this
```

### Pattern 2: Adding a Linux Inventory Category

**Location**: `crates/glpi-inventory-local/src/categories/`

**Template**:
```rust
// SPDX-License-Identifier: GPL-2.0-only

//! [Category] inventory collector for Linux systems.
//!
//! Collects [what this category does] from [source: /proc, /sys, command output].

use glpi_core::types::InventoryCategory;
use glpi_core::error::Result;

/// Collect [category] information from the local Linux system
#[cfg(target_os = "linux")]
pub fn collect() -> Result<InventoryCategory> {
    use std::fs;
    
    // Example: Read from /proc or /sys
    let content = fs::read_to_string("/proc/some/file")
        .map_err(|e| glpi_core::error::AgentError::IoError {
            context: "reading /proc/some/file".into(),
            source: e,
        })?;
    
    // Parse and build inventory category
    let mut category = InventoryCategory::default();
    // ... populate category ...
    
    Ok(category)
}

/// Return empty/default for non-Linux platforms
#[cfg(not(target_os = "linux"))]
pub fn collect() -> Result<InventoryCategory> {
    Ok(InventoryCategory::default())
}
```

**Registration** (in `crates/glpi-inventory-local/src/categories/mod.rs`):
```rust
#[cfg(target_os = "linux")]
pub use linux::category_name;
```

And in the task orchestration:
```rust
// In crates/glpi-inventory-local/src/task.rs
categories.push(Box::new(category_name::collect));
```

---

## 🧪 Testing Guidelines

### Test Structure

```
crates/
└── [crate_name]/
    └── tests/
        ├── fixtures/          # Test data files
        │   └── [test_name].json/.walk/.txt
        ├── golden.rs          # Golden-file tests
        ├── integration.rs     # Integration tests
        └── [module]_tests.rs  # Unit tests
```

### Golden-File Testing (Preferred)

See [ADR-007](docs/adr/ADR-007-golden-file-testing.md) for complete details.

**Example**:
```rust
use insta::assert_json_snapshot;
use serde_json::Value;

fn normalize_json(mut value: Value) -> Value {
    if let Value::Object(map) = &mut value {
        let mut sorted: std::collections::BTreeMap<_, _> = map.iter().collect();
        for (_, v) in sorted.iter_mut() {
            *v = normalize_json(v.take());
        }
        *map = sorted.into_iter().collect();
    }
    value
}

#[test]
fn test_inventory_output() {
    let inventory = create_test_inventory();
    let json = serde_json::to_value(&inventory).unwrap();
    let normalized = normalize_json(json);
    
    assert_json_snapshot!("inventory_output", normalized, "{}");
}
```

### SNMP Fixture Testing

```rust
use crate::snmp::walk::WalkSession;

#[tokio::test]
async fn test_mib_with_fixture() {
    // Load real snmpwalk capture from Perl agent
    let walk_data = std::fs::read_to_string(
        "tests/fixtures/snmp_walks/cisco_router.walk"
    ).unwrap();
    
    let mut session = WalkSession::parse(&walk_data).unwrap();
    let mut device = NetworkDevice::default();
    
    // Run the MIB interpretation
    CiscoMib.run(&mut session, &mut device).await.unwrap();
    
    // Assert expected values
    assert_eq!(device.info.hostname, Some("router1".into()));
    assert_eq!(device.info.os_version, Some("15.2(4)E1".into()));
}
```

### Test Commands

```bash
# Run all tests
cargo test --workspace

# Run tests for a specific crate
cargo test -p glpi-discovery

# Run with logging output
RUST_LOG=debug cargo test -p glpi-discovery

# Run only unit tests (no integration tests)
cargo test --workspace --lib

# Run only integration tests
cargo test --workspace --test '*'

# Run a specific test by name
cargo test -p glpi-discovery test_mib_parsing

# Update snapshot files (for golden-file tests)
cargo insta test --workspace --accept
```

---

## 📖 Common Commands

### Building

```bash
# Build all crates
cargo build --workspace

# Build with all features
cargo build --workspace --all-features

# Build for release (optimized)
cargo build --workspace --release

# Build a specific crate
cargo build -p glpi-discovery
```

### Testing

```bash
# Run all tests
cargo test --workspace

# Run tests for a specific crate
cargo test -p glpi-discovery

# Run with logging
RUST_LOG=debug cargo test

# Check formatting
cargo fmt --check

# Run clippy (all lints as errors)
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

### Running

```bash
# Run the CLI
cargo run -p glpi-cli -- --help

# Run inventory command
cargo run -p glpi-cli -- inventory --json

# Run netdiscovery
cargo run -p glpi-cli -- netdiscovery --range 192.168.1.0/24
```

### Code Quality

```bash
# Format all code
cargo fmt

# Check formatting without applying
cargo fmt --check

# Run clippy
cargo clippy --workspace --all-targets -- -D warnings

# Check for unused dependencies
cargo udeps --workspace
```

---

## 🎯 Current Priorities & How to Help

### High Priority (Phase 3: NetInventory + MIBs)

**Vendor MIBs needed**: HP, Brother, Lexmark, Dell Networking, Aruba, Palo Alto, Ricoh, Kyocera, Fujitsu, Lenovo

**How to help**:
1. Find the Perl MIB module: `GLPI::Agent::SNMP::MibSupport::[Vendor]`
2. Create file: `crates/glpi-discovery/src/snmp/mib/vendor/[vendor].rs`
3. Follow the pattern from `xerox.rs` or `cisco.rs`
4. Use `WalkSession` with fixture data for tests
5. Register in `mod.rs` and the MIB registry

**Fixtures**: Use `snmpwalk -v 2c -c public <device> -On > fixture.walk` from Perl agent

### High Priority (Phase 6: Local Inventory)

**Windows Inventory Categories** (15+ needed):
- WMI-based system information
- Registry-based configuration
- Windows-specific hardware detection

**macOS Inventory Categories** (10+ needed):
- System Profiler integration
- IOKit for hardware info
- Keychain for certificates

**Note**: Use `#[cfg(target_os = "windows")]` and `#[cfg(target_os = "macos")]` respectively.

**Important**: Windows code must run on a dedicated COM worker thread (not `Send`).

### Medium Priority (Phase 5: CLI + Daemon)

- Complete daemon mode
- ToolBox HTTP server (/status, /now, /)
- Configuration refinements
- Category filtering improvements

---

## 📄 Architecture Decision Records (ADRs)

All major architectural decisions are documented in [docs/adr/](docs/adr/):

| ADR | Title | Summary |
|-----|-------|---------|
| [ADR-001](docs/adr/ADR-001-use-rust-for-glpi-agent-rewrite.md) | Use Rust for Rewrite | Why Rust was chosen over Go/Python/C++ |
| [ADR-002](docs/adr/ADR-002-cargo-workspace-architecture.md) | Workspace Architecture | 17 crates, why not monolith |
| [ADR-003](docs/adr/ADR-003-snmp-stack-selection.md) | SNMP Stack | Why snmp2 crate was selected |
| [ADR-004](docs/adr/ADR-004-configuration-layering.md) | Configuration | Layered config with custom sources |
| [ADR-005](docs/adr/ADR-005-tokio-async-runtime.md) | Async Runtime | Why Tokio was chosen |
| [ADR-006](docs/adr/ADR-006-phased-migration-strategy.md) | Migration Strategy | Phased approach, 7 phases |
| [ADR-007](docs/adr/ADR-007-golden-file-testing.md) | Testing | Golden-file testing with fixtures |
| [ADR-008](docs/adr/ADR-008-protocol-priority.md) | Protocol | JSON first, XML as fallback |

**Always read the relevant ADRs before making architectural changes!**

---

## 🆘 Getting Help

### For Architectural Questions
1. Read the [ADRs](docs/adr/)
2. Check [CONTRIBUTING.md](CONTRIBUTING.md) for human developer guidelines
3. Review existing code patterns

### For Coding Patterns
1. Search existing code with `grep`
2. Look at similar implementations
3. Follow the established conventions

### For Agent-Specific Issues
1. Check your agent's configuration file in [.agents/](.agents/)
2. Review the universal guidelines in this file (AGENTS.md)

---

## 📝 Agent-Specific Configurations

Each AI agent has its own configuration file with **agent-specific** instructions:

| Agent | File | Focus |
|-------|------|-------|
| **Claude Code** | [.agents/claude/instructions.md](.agents/claude/instructions.md) | Devcontainer integration, critical instructions |
| **GitHub Copilot** | [.agents/COPILOT.md](.agents/COPILOT.md) | Prompt engineering, workflow |
| **Cursor** | [.agents/CURSOR.md](.agents/CURSOR.md) | VS Code integration, extensions |
| **Devstral** | [.agents/DEVSTRAL.md](.agents/DEVSTRAL.md) | Tool usage, operating discipline |
| **VS Code AI** | [.agents/VSCODE.md](.agents/VSCODE.md) | Chat/Inline Chat usage |

**Note**: Agent-specific files contain **ONLY** information specific to that agent. All universal content is in this file (AGENTS.md).

---

## 🎨 Code Style Guide

### Naming Conventions
- **Variables & Functions**: `snake_case`
- **Types, Traits, Enums**: `PascalCase`
- **Constants**: `SCREAMING_SNAKE_CASE`
- **Files**: `snake_case.rs`
- **Modules**: `snake_case/`

### Documentation
- **All public items** must have doc comments (`///` or `//!`)
- Use Markdown in doc comments
- Include examples where helpful
- Document errors with `#[errors]` where applicable

### Error Handling
- Use `glpi_core::error::Result` and `AgentError` for library errors
- Use `anyhow::Result` for binary entry points
- Provide context in error messages
- Avoid generic error messages

### Logging
- Use `tracing` macros: `info!`, `debug!`, `warn!`, `error!`, `trace!`
- Log at appropriate levels
- Include relevant context in log messages

```rust
use tracing::{info, debug, warn, error};

fn process_device(device_id: &str) {
    info!("Processing device: {}", device_id);
    debug!("Device details: id={}, status={}", device_id, status);
    
    if has_issues {
        warn!("Device {} has issues: {}", device_id, issues);
    }
}
```

---

## 🚨 Critical Reminders

1. **English Only**: All code and documentation must be in English
2. **Read First**: Never edit a file you haven't read
3. **Minimal Changes**: Don't touch what wasn't asked
4. **Test Everything**: All changes must be tested
5. **Document Decisions**: Major changes need ADRs
6. **SPDX Headers**: All Rust files must include: `// SPDX-License-Identifier: GPL-2.0-only`

---

*Last updated: June 2026*
*Maintainer: GLPI Agent Rust Team*
