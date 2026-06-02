# ADR-007: Golden-file Testing with Fixture Replay

## Status

🟢 Accepted

## Context and Problem Statement

The GLPI Agent must produce **output identical to the Perl agent** to ensure:
- GLPI server compatibility
- No regressions in functionality
- Proper handling of vendor quirks

Traditional unit tests are **insufficient** because they:
- Test individual functions, not end-to-end behavior
- May not catch serialization differences
- Don't verify protocol compliance

The Perl agent has **~200 test files** and **~4,300 sub-tests** backed by real-world fixtures.

## Decision Options

1. **Rewritten Tests in Rust** - Rewrite all Perl tests from scratch
2. **Golden-file Testing** - Compare against committed "golden" files
3. **Live GLPI Server Testing** - Submit to real GLPI server in CI
4. **Property-based Testing** - Generate random inputs

## Decision

We chose **Golden-file Testing with Fixture Replay**, because:
- **Leverages existing knowledge**: Reuses Perl agent's fixtures
- **Ensures parity**: Direct comparison with known-good output
- **Automated**: Runs in CI without requiring a live server
- **Fast**: No network dependencies
- **Comprehensive**: Tests end-to-end behavior

## Implementation

### Fixture Structure

```
crates/
├── glpi-core/
│   └── tests/
│       └── fixtures/
│           ├── protocol/
│           │   ├── glpi_inventory.json
│           │   └── fusion_inventory.xml
│           └── snmp/
│               └── cisco_router.walk
│
├── glpi-discovery/
│   └── tests/
│       └── fixtures/
│           └── snmp_walks/
│               ├── hp_switch.walk
│               └── mikrotik_router.walk
│
└── glpi-inventory-local/
    └── tests/
        └── fixtures/
            ├── linux/
            │   ├── cpuinfo
            │   └── inventory.json
            └── windows/
                └── wmi_dump.json
```

### Test Harness

```rust
use insta::assert_json_snapshot;

#[test]
fn test_inventory_serialization() {
    let inventory = create_test_inventory();
    let json = serde_json::to_value(&inventory).unwrap();
    let normalized = normalize_json(json);
    assert_json_snapshot!("inventory", normalized, "{}");
}
```

### SNMP Fixture Replay

```rust
#[tokio::test]
async fn test_mib_parsing() {
    let walk_data = load_fixture("snmp_walks/cisco_router.walk");
    let mut session = WalkSession::parse(&walk_data).unwrap();
    let device = interpret_mibs(&mut session).await.unwrap();
    let expected = load_fixture("snmp_devices/cisco_router.json");
    assert_json_snapshot!("cisco_router", device, "{}");
}
```

## Consequences

### Positive
- 100% parity with Perl agent
- Edge case coverage from 17 years of development
- Fast, automated tests
- Maintainable fixtures
- Regression protection

### Negative
- Fixture maintenance overhead
- Storage overhead (~10-50MB)
- Test brittleness (mitigated by normalization)

## Normalization Strategies

1. **JSON Field Order**: Sort object keys
2. **Timestamps**: Remove or replace with fixed values
3. **Hostnames/IPs**: Replace with placeholders
4. **Versions**: Normalize version strings
5. **Whitespace**: Remove insignificant whitespace

## Alternatives Considered

- **Rewritten Tests**: Risk of missing critical edge cases.
- **Live Server Testing**: Not practical for CI (availability, latency).
- **Property-based Testing**: Doesn't ensure protocol compliance.
