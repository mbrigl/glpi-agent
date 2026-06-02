# ADR-003: SNMP Stack Selection (snmp2 Crate)

## Status

🟢 Accepted

## Context and Problem Statement

The GLPI Agent requires **full SNMPv1/v2c/v3 support** with the following mandatory features:

- **Authentication**: MD5, SHA-1, SHA-224/256/384/512
- **Privacy**: DES, AES-128, AES-192, **AES-256**
- **Cisco compatibility**: AES key localization with `KeyExtension::Reeder` (Cisco's "AES-192-C / AES-256-C" variant)
- **Async support**: Non-blocking operations for high-concurrency network discovery
- **License compatibility**: Must be compatible with GPL-2.0-only

Initial assumption was that no Rust crate covered the full SNMPv3 algorithm matrix, requiring a hand-built USM (User-based Security Model) implementation.

## Decision Options

1. **`snmp2` Crate (v0.5)**
   - Pure Rust implementation
   - Async support via `tokio` feature
   - Full SNMPv1/v2c/v3 support
   - Dual-licensed: **MIT OR Apache-2.0**
   - Includes Cisco AES key extension (`KeyExtension::Reeder`)

2. **`rasn-snmp` + `rasn` + Hand-built USM**
   - Full control over implementation
   - Flexible to add custom features
   - Requires implementing RFC 3414 (USM) and RFC 7860 (SNMPv3)
   - High risk of crypto bugs

3. **`netsnmp` (C library) + FFI bindings**
   - Battle-tested implementation
   - Full feature support
   - Requires C dependencies
   - BSD license (compatible with GPL-2.0)

## Decision

We chose **`snmp2` v0.5 with the `tokio` feature**, because:

- **Eliminates highest risk**: No need to implement USM crypto ourselves
- **Full feature coverage**: Supports all required auth/priv algorithms including Cisco variants
- **License compatibility**: We **elect the MIT license arm** (Apache-2.0 is GPL-2.0-incompatible)
- **Async out-of-the-box**: Native Tokio integration
- **Pure Rust**: No C dependencies or FFI complexity
- **Maintenance**: Single crate dependency vs. multiple crates + custom code

## Consequences

### Positive

- **Rapid implementation**: SNMP support was implemented in Phase 2 without crypto bugs
- **Maintainability**: No custom USM code to maintain
- **Performance**: Pure Rust implementation with async support
- **Correctness**: Leverages existing, tested SNMP implementation

### Negative

- **Single maintainer risk**: `snmp2` is maintained primarily by one person. **Mitigation**: Fork and maintain our own version if needed.
- **`contextName` limitation**: `snmp2` v0.5 cannot set a non-default SNMPv3 `contextName`. **Mitigation**: Track upstream issue; implement workaround if needed.

## Alternatives Considered

- **`rasn-snmp` + Hand-built USM**: Complexity and risk of implementing USM crypto correctly (especially Cisco's key extension) outweighs the benefits.
- **`netsnmp`**: C FFI boundary adds complexity and potential for memory safety issues. Pure Rust solutions are preferred.

## Verification

The `snmp2` crate was verified to support:
- SNMPv3 with AES-256 and `KeyExtension::Reeder` (Cisco compatibility)
- All standard auth/priv combinations from RFC 3414
- Async operations via Tokio
- MIT license election for GPL-2.0 compatibility
