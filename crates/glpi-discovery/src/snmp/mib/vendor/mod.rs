// SPDX-License-Identifier: GPL-2.0-only

//! Vendor-specific MIB modules, selected by `sysObjectID`.
//!
//! Each module implements [`MibSupport`](super::MibSupport) and overrides
//! `applies_to` to match its vendor's enterprise OID, so it runs only for the
//! relevant devices. [`register_all`] adds every implemented vendor module to a
//! registry (used by [`MibRegistry::with_defaults`](super::MibRegistry::with_defaults)).
//!
//! Implemented from published vendor OIDs and exercised with representative
//! `snmpwalk` fixtures; the set grows in batches.

use std::sync::Arc;

use super::MibRegistry;

pub mod cisco;

pub use cisco::CiscoMib;

/// Registers all implemented vendor MIB modules into `registry`.
pub fn register_all(registry: &mut MibRegistry) {
    registry.register(Arc::new(CiscoMib));
}
