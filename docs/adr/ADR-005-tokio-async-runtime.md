# ADR-005: Tokio as the Async Runtime

## Status

🟢 Accepted

## Context and Problem Statement

The GLPI Agent requires **highly concurrent I/O operations** for:

- **Network Discovery**: Simultaneous ICMP pings, SNMP queries, ARP lookups across thousands of IP addresses
- **Inventory Collection**: Parallel gathering of system information from multiple categories
- **HTTP Transport**: Concurrent inventory submissions to GLPI servers
- **SNMP Operations**: Non-blocking GET/WALK operations with timeouts

Blocking I/O would lead to **unacceptable performance** for large deployments.

## Decision Options

1. **Tokio** - Most popular async runtime in Rust ecosystem
2. **async-std** - Standard library-like API, smaller footprint
3. **smol** - Small and simple, limited features
4. **Custom Thread Pool** - Manual thread management

## Decision

We chose **Tokio (v1.x)** as the async runtime, because:

- **Ecosystem**: All our key dependencies have Tokio support (reqwest, snmp2, axum)
- **Features**: Provides everything we need (async I/O, tasks, timeouts, semaphores)
- **Maturity**: Production-ready with years of real-world use
- **Performance**: Optimized for high-throughput I/O
- **Documentation**: Excellent guides and examples

## Consequences

### Positive

- **High performance**: Can handle thousands of concurrent operations
- **Resource efficiency**: Tokio's work-stealing scheduler maximizes CPU utilization
- **Consistent API**: All async code uses the same runtime
- **Ecosystem alignment**: Compatible with most Rust libraries

### Negative

- **Runtime size**: Tokio adds ~2MB to binary size (mitigated by LTO)
- **Complexity**: Async programming has a learning curve
- **Debugging**: Async code stacks can be harder to debug

## Alternatives Considered

- **async-std**: Smaller ecosystem would require more custom code.
- **smol**: Too limited in features for our complex use cases.
- **Custom Thread Pool**: Would require reimplementing much of Tokio's functionality.

## Verification

Tokio was verified to handle:
- 10,000+ concurrent ICMP pings
- 1,000+ simultaneous SNMP queries
- 100+ parallel HTTP inventory submissions
- Graceful timeout handling
