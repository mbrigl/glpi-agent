# Architecture Decision Records (ADRs)

This directory contains Architecture Decision Records (ADRs) for the **GLPI Agent Rust** project.

## Purpose

ADRs document **important architectural decisions** made during the development of the GLPI Agent Rust rewrite. Each ADR captures:

- The **context** and problem statement
- The **options** considered
- The **decision** made and its rationale
- The **consequences** (both positive and negative)

## Process

1. **Propose**: Create a new ADR as a Markdown file following the [template](#template)
2. **Discuss**: Review with the team (or via PR comments)
3. **Accept/Reject**: Mark the ADR status accordingly
4. **Supersede**: If a decision is reversed, create a new ADR and mark the old one as superseded

## Status Definitions

| Status | Description |
|--------|-------------|
| 🟢 **Accepted** | Decision is approved and implemented |
| 🟡 **Proposed** | Decision is under consideration |
| 🔴 **Rejected** | Decision was considered but not taken |
| ⚪ **Superseded** | Replaced by a newer ADR (reference the new ADR) |

## Index

| Number | Title | Status | Date |
|--------|-------|--------|------|
| [ADR-001](./ADR-001-use-rust-for-glpi-agent-rewrite.md) | Use Rust for the GLPI Agent Rewrite | 🟢 Accepted | 2024-XX-XX |
| [ADR-002](./ADR-002-cargo-workspace-architecture.md) | Cargo Workspace with Multiple Specialized Crates | 🟢 Accepted | 2024-XX-XX |
| [ADR-003](./ADR-003-snmp-stack-selection.md) | SNMP Stack Selection (snmp2 Crate) | 🟢 Accepted | 2024-XX-XX |
| [ADR-004](./ADR-004-configuration-layering.md) | Configuration Layering with Custom Sources | 🟢 Accepted | 2024-XX-XX |
| [ADR-005](./ADR-005-tokio-async-runtime.md) | Tokio as the Async Runtime | 🟢 Accepted | 2024-XX-XX |
| [ADR-006](./ADR-006-phased-migration-strategy.md) | Phased Migration Strategy | 🟢 Accepted | 2024-XX-XX |
| [ADR-007](./ADR-007-golden-file-testing.md) | Golden-file Testing with Fixture Replay | 🟢 Accepted | 2024-XX-XX |
| [ADR-008](./ADR-008-protocol-priority.md) | GLPI Native JSON Protocol Priority | 🟢 Accepted | 2024-XX-XX |

## Template

Use the following template for new ADRs:

```markdown
# ADR-XXXX: [Short Title in kebab-case]

## Status

🟢 Accepted | 🟡 Proposed | 🔴 Rejected | ⚪ Superseded by ADR-YYYY

## Context and Problem Statement

[Describe the context and problem that needs to be solved.]

## Decision Options

1. **Option A** - [Description]
2. **Option B** - [Description]
3. **Option C** - [Description]

## Decision

We chose **Option X**, because:
- [Reason 1]
- [Reason 2]

## Consequences

### Positive

- [Benefit 1]
- [Benefit 2]

### Negative

- [Trade-off 1]
- [Trade-off 2]

## Alternatives Considered

- **Option A**: [Why not chosen]
- **Option C**: [Why not chosen]
```

## Language Policy

All ADRs **must be written in English** without exception, as per the repository's [AGENTS.md](../AGENTS.md) guidelines.
