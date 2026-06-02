// SPDX-License-Identifier: GPL-2.0-only

//! Associated-file download and assembly.
//!
//! A Deploy order ships each file as one or more parts, identified by their
//! SHA-512. The agent fetches every part (from a mirror or the GLPI server),
//! verifies its SHA-512, and concatenates them into the target file. Fetching
//! sits behind the [`PartFetcher`] seam: [`HttpPartFetcher`] downloads over
//! HTTP(S), while [`MockPartFetcher`] serves bytes from memory in tests.

use std::collections::BTreeMap;
use std::path::Path;

use async_trait::async_trait;
use glpi_core::error::{AgentError, Result};
use serde::Deserialize;

use crate::checksum::{sha512_hex, sha512_matches};

/// An associated file: its name and the ordered SHA-512 of its parts.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct AssociatedFile {
    /// File name to write.
    pub name: String,
    /// Part SHA-512 digests, in assembly order.
    #[serde(default)]
    pub multiparts: Vec<String>,
    /// Mirror base URLs to try when fetching parts.
    #[serde(default)]
    pub mirrors: Vec<String>,
}

/// Fetches a file part by its SHA-512.
#[async_trait]
pub trait PartFetcher: Send + Sync {
    /// Fetches the bytes of the part identified by `sha512`, trying `mirrors`.
    ///
    /// # Errors
    ///
    /// Returns an error if the part cannot be fetched from any source.
    async fn fetch(&self, sha512: &str, mirrors: &[String]) -> Result<Vec<u8>>;
}

/// Downloads `file`'s parts through `fetcher`, verifies each part's SHA-512 and
/// writes the assembled file to `target`. When `expected_sha512` is given, the
/// fully-assembled file's digest is verified too.
///
/// # Errors
///
/// Returns [`AgentError::Task`] on a part or whole-file checksum mismatch, or
/// [`AgentError::Io`] on a write failure.
pub async fn assemble(
    file: &AssociatedFile,
    fetcher: &dyn PartFetcher,
    target: &Path,
    expected_sha512: Option<&str>,
) -> Result<()> {
    let mut assembled = Vec::new();
    for (index, part_sha) in file.multiparts.iter().enumerate() {
        let bytes = fetcher.fetch(part_sha, &file.mirrors).await?;
        let actual = sha512_hex(&bytes);
        if !sha512_matches(&actual, part_sha) {
            return Err(AgentError::Task(format!(
                "part {index} of {} failed SHA-512 verification",
                file.name
            )));
        }
        assembled.extend_from_slice(&bytes);
    }

    if let Some(expected) = expected_sha512 {
        let whole = sha512_hex(&assembled);
        if !sha512_matches(&whole, expected) {
            return Err(AgentError::Task(format!(
                "assembled file {} failed SHA-512 verification",
                file.name
            )));
        }
    }

    std::fs::write(target, &assembled)?;
    tracing::info!(file = %file.name, bytes = assembled.len(), parts = file.multiparts.len(), "assembled deploy file");
    Ok(())
}

/// A [`PartFetcher`] that downloads parts over HTTP(S).
///
/// Each mirror is treated as a base URL; the part is requested at
/// `<mirror>/<sha512>` (the GLPI deploy mirror layout). Mirrors are tried in
/// order until one succeeds.
#[derive(Debug, Clone)]
pub struct HttpPartFetcher {
    client: reqwest::Client,
}

impl HttpPartFetcher {
    /// Builds a fetcher with a default HTTP client.
    ///
    /// # Errors
    ///
    /// Returns [`AgentError::Transport`] if the client cannot be built.
    pub fn new() -> Result<Self> {
        let client = reqwest::Client::builder()
            .build()
            .map_err(|e| AgentError::Transport(format!("building deploy HTTP client: {e}")))?;
        Ok(Self { client })
    }
}

#[async_trait]
impl PartFetcher for HttpPartFetcher {
    async fn fetch(&self, sha512: &str, mirrors: &[String]) -> Result<Vec<u8>> {
        let mut last_error = None;
        for mirror in mirrors {
            let url = format!("{}/{sha512}", mirror.trim_end_matches('/'));
            match self.client.get(&url).send().await {
                Ok(response) if response.status().is_success() => match response.bytes().await {
                    Ok(bytes) => return Ok(bytes.to_vec()),
                    Err(e) => last_error = Some(format!("reading {url}: {e}")),
                },
                Ok(response) => {
                    last_error = Some(format!("{url} returned HTTP {}", response.status()))
                }
                Err(e) => last_error = Some(format!("requesting {url}: {e}")),
            }
        }
        Err(AgentError::Transport(format!(
            "could not fetch part {sha512}: {}",
            last_error.unwrap_or_else(|| "no mirrors configured".to_owned())
        )))
    }
}

/// An in-memory [`PartFetcher`] for tests: SHA-512 → bytes.
#[derive(Debug, Default, Clone)]
pub struct MockPartFetcher {
    parts: BTreeMap<String, Vec<u8>>,
}

impl MockPartFetcher {
    /// Builds an empty mock fetcher.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers `bytes` under their own SHA-512 (as the server would key them).
    #[must_use]
    pub fn with_part(mut self, bytes: &[u8]) -> Self {
        self.parts.insert(sha512_hex(bytes), bytes.to_vec());
        self
    }
}

#[async_trait]
impl PartFetcher for MockPartFetcher {
    async fn fetch(&self, sha512: &str, _mirrors: &[String]) -> Result<Vec<u8>> {
        self.parts
            .get(sha512)
            .cloned()
            .ok_or_else(|| AgentError::Transport(format!("mock: no part {sha512}")))
    }
}

#[cfg(test)]
mod tests {
    use super::{assemble, AssociatedFile, MockPartFetcher};
    use crate::checksum::sha512_hex;

    fn file_from_parts(parts: &[&[u8]]) -> (AssociatedFile, MockPartFetcher) {
        let mut fetcher = MockPartFetcher::new();
        let mut multiparts = Vec::new();
        for part in parts {
            fetcher = fetcher.with_part(part);
            multiparts.push(sha512_hex(part));
        }
        let file = AssociatedFile {
            name: "payload.bin".to_owned(),
            multiparts,
            mirrors: Vec::new(),
        };
        (file, fetcher)
    }

    #[tokio::test]
    async fn assembles_and_verifies_multipart_file() {
        let (file, fetcher) = file_from_parts(&[b"hello ", b"deploy ", b"world"]);
        let whole = sha512_hex(b"hello deploy world");
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join(&file.name);

        assemble(&file, &fetcher, &target, Some(&whole))
            .await
            .unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"hello deploy world");
    }

    #[tokio::test]
    async fn rejects_a_tampered_whole_file_digest() {
        let (file, fetcher) = file_from_parts(&[b"abc"]);
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join(&file.name);
        let err = assemble(&file, &fetcher, &target, Some("deadbeef"))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("SHA-512"));
        assert!(!target.exists());
    }

    #[tokio::test]
    async fn missing_part_is_an_error() {
        let file = AssociatedFile {
            name: "x".to_owned(),
            multiparts: vec!["0".repeat(128)],
            mirrors: Vec::new(),
        };
        let dir = tempfile::tempdir().unwrap();
        let err = assemble(&file, &MockPartFetcher::new(), &dir.path().join("x"), None)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("no part"));
    }
}
