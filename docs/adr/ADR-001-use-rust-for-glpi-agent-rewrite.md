# ADR-001: Use Rust for the GLPI Agent Rewrite

## Status

🟢 Accepted

## Context and Problem Statement

The upstream [GLPI Agent](https://github.com/glpi-project/glpi-agent) is written in **Perl** and has accumulated technical debt over its 17-year history. A rewrite is necessary to:

- Improve **performance** (especially for large-scale deployments)
- Reduce **maintenance burden** (Perl's declining popularity, dependency management issues)
- Enable **cross-compilation** for easier distribution
- Support **modern security standards** (TLS 1.3, SNMPv3 with AES-256)
- Provide a **maintainable codebase** for the next decade

The rewrite must maintain **full backward compatibility** with the GLPI inventory protocol and support all existing features.

## Decision Options

1. **Rust**
   - Modern systems language with memory safety guarantees
   - Strong type system and compile-time checks
   - Excellent async support (Tokio ecosystem)
   - Cross-compilation support for all target platforms (Linux, Windows, macOS)
   - Growing ecosystem for systems programming

2. **Go**
   - Simple concurrency model (goroutines)
   - Built-in cross-compilation
   - Strong standard library for networking
   - Less strict type system than Rust

3. **Python**
   - Extensive library ecosystem
   - Easy to prototype
   - Performance limitations for high-concurrency scenarios
   - Dependency management challenges

4. **C++**
   - High performance
   - Full control over system resources
   - Complex build system
   - Memory safety issues

5. **Continue with Perl**
   - No rewrite effort
   - Maintain existing codebase
   - Continued technical debt accumulation

## Decision

We chose **Rust**, because:

- **Memory safety**: Eliminates entire classes of bugs (use-after-free, buffer overflows) critical for a security-sensitive agent
- **Performance**: Comparable to C/C++ with zero-cost abstractions
- **Cross-compilation**: First-class support for Linux, Windows, and macOS from any platform
- **Dependency management**: Cargo provides deterministic builds and easy dependency management
- **Modern tooling**: Built-in testing, documentation, and formatting (cargo test, cargo doc, cargo fmt)
- **Ecosystem**: Strong support for async I/O (Tokio), HTTP (reqwest), serialization (serde), and SNMP (snmp2)
- **Community**: Growing adoption in systems programming, ensuring long-term viability
- **License compatibility**: Rust's MIT/Apache 2.0 licenses are compatible with GLPI's GPL-2.0-only

## Consequences

### Positive

- **Improved reliability**: Memory safety guarantees reduce crash rates
- **Better performance**: Efficient handling of concurrent network operations
- **Easier distribution**: Single binary deployment per platform
- **Modern development experience**: Strong compiler support, IDE integration
- **Type safety**: Compile-time validation of inventory data structures

### Negative

- **Learning curve**: Team members must learn Rust
- **Compile times**: Slower than interpreted languages (mitigated by incremental compilation)
- **Binary size**: Larger binaries than Perl scripts (mitigated by LTO and stripping)
- **Less mature ecosystem**: Some niche Perl modules (e.g., SNMP) have no direct Rust equivalents

## Alternatives Considered

- **Go**: Strong contender, but lacks Rust's type system strength for complex domain modeling (inventory categories, SNMP MIBs). Go's error handling (multiple return values) is less ergonomic for our use case.
- **Python**: Performance would be insufficient for large-scale network discovery with thousands of devices. Dependency management (pip) is less robust than Cargo.
- **C++**: Memory safety concerns and complex build systems outweigh performance benefits. Rust provides similar performance with safer defaults.
- **Perl**: Would perpetuate technical debt without addressing fundamental issues with dependency management and concurrency.
