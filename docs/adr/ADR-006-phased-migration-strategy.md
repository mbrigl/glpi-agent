# ADR-006: Phased Migration Strategy

## Status

🟢 Accepted

## Context and Problem Statement

The GLPI Agent is a **large, complex system** with ~200 test files and ~4,300 sub-tests in the Perl implementation. A "big bang" rewrite would:
- Take **years** to complete
- Introduce **high risk** of regressions
- Delay **delivering value** to users
- Make **testing difficult**

## Decision Options

1. **Big Bang Rewrite** - Rewrite everything before releasing anything
2. **Feature-by-Feature Replacement** - Replace Perl modules in existing codebase
3. **Phased Approach** - Divide work into logical phases
4. **Minimal Viable Product (MVP) First** - Build minimal agent first

## Decision

We chose **Phased Approach**, because:
- **Risk mitigation**: Each phase is self-contained and testable
- **Incremental delivery**: Users can benefit from completed phases early
- **Clear prioritization**: Dependencies between phases are explicit
- **Test parity**: Can validate each phase against Perl agent behavior
- **Parallel development**: Teams can work on different phases simultaneously

## Phase Definition

### Phase 1: Foundation (✅ Complete)
- **`glpi-core`**: Types, protocol, configuration, auth, logging
- **`glpi-transport`**: HTTP client, glpi-injector
- **Golden-file harness**: Test infrastructure

### Phase 2: NetDiscovery Core (✅ Complete)
- IP range expansion
- Parallel scanner
- Discovery methods: Ping, ARP, NetBIOS, SNMP
- SNMP client with async support

### Phase 3: NetInventory + MIBs (🟡 In Progress)
- MIB framework
- Standard MIBs (8): system, if, entity, printer, bridge, lldp, cdp, ip
- Vendor MIBs (34+): Cisco, Juniper, Fortinet, Mikrotik, printers (HP, Brother, Ricoh, Konica/Sindoh, Lexmark, Xerox, Canon, Epson, OKI, Pantum), network (D-Link, Intelbras, Tiesse, Aerohive, Telco, FoxGate, Nokia/Alcatel, WatchGuard), etc.
- NetInventory task

### Phase 4: Platform-Specific Features (⏳ Pending)
- Windows: COM/WMI worker, Registry config, Certificate store
- macOS: Keychain, System Profiler
- IEC 61850: FFI binding

### Phase 5: CLI + Daemon (🟡 Partially Complete)
- `glpi-agent` binary with subcommands
- Configuration loading
- Category filtering
- `glpi-scheduler` and `glpi-http`

### Phase 6: Local Inventory (🟡 Linux Complete)
- Linux: All 20+ categories
- Windows: All categories (WMI-based)
- macOS: All categories
- Other platforms: Solaris, HP-UX, AIX, FreeBSD

### Phase 7: Remote Inventory (⏳ Pending)
- `glpi-inventory-remote`: SSH, WinRM
- `glpi-vsphere`: VMware ESX/vCenter
- State files and delta diff

## Test Migration

**Critical Rule**: Every test must be migrated alongside the module it covers.
- Reuse Perl agent's fixtures
- Golden-file comparison against Perl output
- Phase is "done" only when tests pass

## Consequences

### Positive
- Early feedback from each phase
- Risk reduction through isolation
- Parallel development possible
- Clear progress milestones

### Negative
- Delayed full feature set
- Integration challenges between phases
- Test maintenance overhead

## Alternatives Considered

- **Big Bang Rewrite**: Too risky with no intermediate deliverables.
- **Feature-by-Feature**: Complex Perl/Rust integration overhead.
- **MVP First**: Early architectural decisions might need revision.
