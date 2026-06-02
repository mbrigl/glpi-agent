// SPDX-License-Identifier: GPL-2.0-only

//! Build script for `glpi-iec61850`.
//!
//! It only does anything when the off-by-default `libiec61850` feature is
//! enabled: it then tells the linker to bind the system **libiec61850** client
//! library (v1.6.x). Without the feature the crate is pure Rust and this script
//! is a no-op, so the default build needs neither the C library nor a C
//! toolchain.
//!
//! Override the library name or search path with `IEC61850_LIB`
//! (default `iec61850`) and `IEC61850_LIB_DIR`.

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    if std::env::var_os("CARGO_FEATURE_LIBIEC61850").is_none() {
        return;
    }

    if let Some(dir) = std::env::var_os("IEC61850_LIB_DIR") {
        println!("cargo:rustc-link-search=native={}", dir.to_string_lossy());
    }
    let lib = std::env::var("IEC61850_LIB").unwrap_or_else(|_| "iec61850".to_owned());
    // libiec61850 is a C library; link it dynamically by default (override the
    // search path / name above for a static or relocated build).
    println!("cargo:rustc-link-lib={lib}");
    println!("cargo:rerun-if-env-changed=IEC61850_LIB");
    println!("cargo:rerun-if-env-changed=IEC61850_LIB_DIR");
}
