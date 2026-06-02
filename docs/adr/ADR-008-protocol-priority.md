# ADR-008: GLPI Native JSON Protocol Priority

## Status

🟢 Accepted

## Context and Problem Statement

The GLPI Agent supports **two inventory protocols**:

1. **GLPI Native JSON Protocol** (v11+): Native JSON envelope, simpler, recommended
2. **FusionInventory XML Protocol** (Legacy): XML-based, for older GLPI versions

The Perl agent historically prioritized FusionInventory. However:
- GLPI **11+ strongly recommends** Native JSON
- JSON is **faster** to serialize/deserialize
- JSON is **easier** to debug and validate
- The **future direction** is clearly toward JSON

## Decision Options

1. **FusionInventory First** - Default to XML, JSON as opt-in
2. **JSON First** - Default to JSON, XML as fallback
3. **JSON Only** - Only support JSON
4. **Protocol Auto-Detection** - Try JSON first, fall back to XML

## Decision

We chose **JSON First (Modern Approach)**, because:
- **GLPI's direction**: GLPI 11+ explicitly recommends Native JSON
- **Performance**: JSON serialization is ~3-5x faster
- **Simplicity**: Cleaner schema, easier to maintain
- **Debugging**: Human-readable without special tooling
- **Future-proofing**: Reduces technical debt
- **Compatibility maintained**: XML still available via `--fusion` flag

## Implementation

### Protocol Selection

```rust
pub enum InventoryProtocol {
    Glpi,    // Native JSON (default)
    Fusion,  // XML (legacy)
}

impl Default for InventoryProtocol {
    fn default() -> Self {
        InventoryProtocol::Glpi  // JSON is the default
    }
}
```

### CLI Integration

```rust
#[derive(clap::Parser)]
struct InventoryCommand {
    #[clap(long)]
    fusion: bool,  // Use FusionInventory XML
}

fn get_protocol(fusion: bool) -> InventoryProtocol {
    if fusion {
        InventoryProtocol::Fusion
    } else {
        InventoryProtocol::Glpi  // Default to JSON
    }
}
```

## Consequences

### Positive
- Better performance
- Simpler code maintenance
- Aligns with modern standards
- Easier debugging

### Negative
- Backward compatibility requires explicit `--fusion` flag
- Dual maintenance of both protocols

## Mitigation Strategies

1. **Clear documentation** on protocol selection
2. **Configuration warning** when using FusionInventory
3. **Future auto-detection** (if needed)

## Verification

Both protocols verified to:
- Be accepted by GLPI servers
- Produce identical inventory results
- Handle all categories correctly
- Support all authentication methods
