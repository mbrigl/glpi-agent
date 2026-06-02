// SPDX-License-Identifier: GPL-2.0-only

//! Content checksums used by the Collect `findFile` filters.
//!
//! GLPI's Collect task can filter found files by `checkSumSHA256` or
//! `checkSumSHA512`; both are lower-case hex digests of the file contents.

use std::path::Path;

use glpi_core::error::Result;
use sha2::{Digest, Sha256, Sha512};

/// Returns the lower-case hex SHA-256 digest of `bytes`.
#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
    hex(Sha256::digest(bytes).as_slice())
}

/// Returns the lower-case hex SHA-512 digest of `bytes`.
#[must_use]
pub fn sha512_hex(bytes: &[u8]) -> String {
    hex(Sha512::digest(bytes).as_slice())
}

/// Computes the SHA-256 digest of the file at `path`.
///
/// # Errors
///
/// Returns [`glpi_core::error::AgentError::Io`] if the file cannot be read.
pub fn file_sha256_hex(path: &Path) -> Result<String> {
    Ok(sha256_hex(&std::fs::read(path)?))
}

/// Computes the SHA-512 digest of the file at `path`.
///
/// # Errors
///
/// Returns [`glpi_core::error::AgentError::Io`] if the file cannot be read.
pub fn file_sha512_hex(path: &Path) -> Result<String> {
    Ok(sha512_hex(&std::fs::read(path)?))
}

/// Encodes bytes as a lower-case hex string.
fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    bytes
        .iter()
        .fold(String::with_capacity(bytes.len() * 2), |mut acc, b| {
            let _ = write!(acc, "{b:02x}");
            acc
        })
}

#[cfg(test)]
mod tests {
    use super::{sha256_hex, sha512_hex};

    #[test]
    fn known_empty_digests() {
        // RFC test vectors for the empty input.
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha512_hex(b""),
            "cf83e1357eefb8bdf1542850d66d8007d620e4050b5715dc83f4a921d36ce9ce\
             47d0d13c5d85f2b0ff8318d2877eec2f63b931bd47417a81a538327af927da3e"
        );
    }

    #[test]
    fn known_abc_digest() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
