// SPDX-License-Identifier: GPL-2.0-only

//! SHA-512 checksums for the Deploy task.
//!
//! Deploy verifies downloaded file parts and runs `FileSHA512` /
//! `FileSHA512Mismatch` preconditions, all keyed on the lower-case hex SHA-512
//! of file contents. Server-supplied digests may use either case, so
//! [`sha512_matches`] compares case-insensitively.

use std::path::Path;

use glpi_core::error::Result;
use sha2::{Digest, Sha512};

/// Returns the lower-case hex SHA-512 digest of `bytes`.
#[must_use]
pub fn sha512_hex(bytes: &[u8]) -> String {
    let digest = Sha512::digest(bytes);
    use std::fmt::Write;
    digest
        .iter()
        .fold(String::with_capacity(128), |mut acc, b| {
            let _ = write!(acc, "{b:02x}");
            acc
        })
}

/// Computes the SHA-512 of the file at `path`.
///
/// # Errors
///
/// Returns [`glpi_core::error::AgentError::Io`] if the file cannot be read.
pub fn file_sha512_hex(path: &Path) -> Result<String> {
    Ok(sha512_hex(&std::fs::read(path)?))
}

/// Compares two SHA-512 hex digests case-insensitively.
#[must_use]
pub fn sha512_matches(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

#[cfg(test)]
mod tests {
    use super::{sha512_hex, sha512_matches};

    #[test]
    fn empty_input_digest() {
        assert_eq!(
            sha512_hex(b""),
            "cf83e1357eefb8bdf1542850d66d8007d620e4050b5715dc83f4a921d36ce9ce\
             47d0d13c5d85f2b0ff8318d2877eec2f63b931bd47417a81a538327af927da3e"
        );
    }

    #[test]
    fn comparison_is_case_insensitive() {
        let lower = sha512_hex(b"glpi");
        let upper = lower.to_ascii_uppercase();
        assert!(sha512_matches(&lower, &upper));
        assert!(!sha512_matches(&lower, "deadbeef"));
    }
}
