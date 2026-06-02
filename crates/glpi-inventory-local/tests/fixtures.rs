// SPDX-License-Identifier: GPL-2.0-only

//! Fixture-replay tests: run the pure parsers against real captured command /
//! `/proc` output committed under `tests/fixtures/`.
//!
//! This seeds the fixture pipeline the migration plan calls for (§13) with a
//! genuine capture; more fixtures (and the upstream `resources/**` tree) are
//! added the same way. Loading a fixture verbatim and asserting the structured
//! result is the bulk of the migrated parser tests.

use glpi_inventory_local::parse_cpuinfo;

/// A real `/proc/cpuinfo` captured from the development host (24 logical
/// processors on a single socket).
const PROC_CPUINFO: &str = include_str!("fixtures/linux/proc_cpuinfo.txt");

#[test]
fn parses_real_proc_cpuinfo_capture() {
    let cpus = parse_cpuinfo(PROC_CPUINFO);

    // Single physical socket on this capture.
    assert_eq!(cpus.len(), 1);
    let cpu = &cpus[0];

    assert_eq!(cpu.manufacturer.as_deref(), Some("Intel"));
    assert!(
        cpu.name.as_deref().unwrap_or_default().contains("Intel"),
        "model name should be present"
    );
    // The 24 logical processors group into this socket.
    assert_eq!(cpu.threads, Some(24));
    // A nominal speed is derived (from the model `@ GHz` or cpu MHz).
    assert!(cpu.speed.unwrap_or(0) > 0, "a speed should be derived");
}
