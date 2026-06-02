# ADR-002: Cargo Workspace with Multiple Specialized Crates

## Status

🟢 Accepted

## Context and Problem Statement

The GLPI Agent must support multiple distinct functionalities:

- Core types, protocol handling, configuration
- Network discovery and SNMP inventory
- Local system inventory (Linux, Windows, macOS)
- Remote inventory (SSH, WinRM, ESX)
- HTTP transport and server communication
- Daemon/scheduler services
- CLI interface

A **monolithic crate** would lead to:
- **Tight coupling** between unrelated components
- **Large binary size** (including unused dependencies)
- **Difficult testing** (hard to test individual components in isolation)
- **Long compile times** (rebuilding everything for small changes)
- **Complex dependency management** (conflicting version requirements)

## Decision Options

1. **Single Monolithic Crate**
   - All functionality in one `glpi-agent` crate
   - Simple dependency management
   - Easy to reason about at a high level

2. **Multiple Independent Crates (Published Separately)**
   - Each major component as a separate crate on crates.io
   - Maximum modularity
   - Versioning complexity

3. **Cargo Workspace with Member Crates**
   - Single repository with multiple internal crates
   - Shared dependencies managed at workspace level
   - Can publish crates individually or as a group

4. **Microservices Architecture**
   - Separate processes for each major function
   - IPC for communication
   - Overkill for this use case

## Decision

We chose **Cargo Workspace with 17 member crates**, because:

- **Modularity**: Each crate has a single, well-defined responsibility
- **Dependency isolation**: Crates only depend on what they need
- **Incremental building**: Changing one crate doesn't require rebuilding the entire codebase
- **Shared configuration**: Workspace-level `Cargo.toml` defines common dependencies and lints
- **Flexible publishing**: Can publish individual crates or the entire agent as a binary
- **Clear boundaries**: Enforces separation of concerns through compile-time checks

## Workspace Structure

```
crates/
├── glpi-core/           # Foundation: types, protocol, config, logging
├── glpi-transport/      # HTTP client, injector
│
├── glpi-inventory-local/ # Local system inventory
├── glpi-discovery/      # Network discovery + SNMP inventory
├── glpi-inventory-remote/# SSH, WinRM, remote inventory
├── glpi-vsphere/        # VMware ESX/vCenter support
├── glpi-iec61850/       # IEC 61850/OT devices
│
├── glpi-cli/            # CLI binary (glpi-agent)
├── glpi-scheduler/      # Daemon scheduling
├── glpi-http/           # Embedded HTTP server (ToolBox)
├── glpi-collect/        # Collect task
├── glpi-deploy/         # Deploy task
├── glpi-wakeonlan/      # WakeOnLan task
└── glpi-plugins/        # Plugin system
```

## Consequences

### Positive

- **Clear separation of concerns**: Each crate has a focused purpose
- **Parallel development**: Teams can work on different crates independently
- **Dependency optimization**: Crates like `glpi-iec61850` can have optional C FFI dependencies without affecting others
- **Testing isolation**: Unit tests for `glpi-core` don't require network access or system dependencies
- **Documentation clarity**: Each crate's purpose is self-documenting

### Negative

- **Cross-crate refactoring**: Changes to `glpi-core` types may require updates across multiple crates
- **Circular dependencies**: Must be carefully avoided (enforced by Cargo)
- **Build complexity**: More crates = more potential for version conflicts
- **CI/CD overhead**: Testing all crates takes longer than a single crate

## Alternatives Considered

- **Single Monolithic Crate**: Would lead to a bloated binary and tight coupling. The agent's diverse responsibilities don't share enough common code to justify a monolith.
- **Multiple Independent Crates**: Versioning and publishing each crate separately would add unnecessary complexity. The crates are tightly coupled to the GLPI Agent's specific requirements.
- **Microservices**: Introduces unnecessary overhead. The agent's components communicate internally and don't need process isolation.
