// SPDX-License-Identifier: GPL-2.0-only

//! Performance smoke tests (migration plan Phase 10).
//!
//! These guard the CPU-bound hot paths that a network scan depends on —
//! IP-range expansion and SNMP-walk parsing — against accidental quadratic
//! regressions. They are deliberately generous: the bounds are orders of
//! magnitude above the real cost on any modern machine, so they fail only on a
//! genuine algorithmic regression, not on a slow CI runner.
//!
//! The plan's headline targets (NetDiscovery ≥ 2× the Perl agent, idle daemon
//! RAM < 50 MB) need a live network and a side-by-side Perl run, so they are not
//! asserted here; this is the offline floor that protects the building blocks.

use std::time::{Duration, Instant};

use glpi_discovery::{Ipv4Range, WalkSession};

#[test]
fn expands_a_16_bit_network_quickly() {
    let range = Ipv4Range::parse("10.0.0.0/16").unwrap();
    let expected = range.len();
    assert!(expected >= 65_000, "a /16 should expand to ~65k addresses");

    let start = Instant::now();
    let count = range.iter().count() as u64;
    let elapsed = start.elapsed();

    assert_eq!(
        count, expected,
        "iterator must yield exactly len() addresses"
    );
    // ~65k address constructions: trivially sub-millisecond in practice.
    assert!(
        elapsed < Duration::from_secs(1),
        "expanding a /16 took {elapsed:?} (regression?)"
    );
}

#[test]
fn parses_a_large_walk_capture_quickly() {
    // Synthesize a sizable interface-table walk (1000 ifDescr rows).
    let mut walk = String::new();
    for i in 1..=1000 {
        walk.push_str(&format!(
            ".1.3.6.1.2.1.2.2.1.2.{i} = STRING: \"GigabitEthernet0/{i}\"\n"
        ));
    }

    let start = Instant::now();
    let session = WalkSession::parse(&walk).unwrap();
    let elapsed = start.elapsed();

    assert_eq!(session.len(), 1000);
    assert!(
        elapsed < Duration::from_secs(1),
        "parsing 1000 varbinds took {elapsed:?} (regression?)"
    );
}
