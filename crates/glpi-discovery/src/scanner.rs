// SPDX-License-Identifier: GPL-2.0-only

//! The parallel network scanner.
//!
//! [`Scanner`] drives a set of [`DiscoveryMethod`]s across a stream of target
//! addresses. Probes run concurrently up to a configurable limit (a
//! [`Semaphore`]) and each is bounded by a per-probe timeout. Every address
//! that responds to at least one method becomes a [`DiscoveredHost`] in the
//! returned vector, sorted by address for deterministic output.
//!
//! The scanner is transport-agnostic: it consumes any iterator of [`IpAddr`],
//! so callers expand [`Ipv4Range`](crate::ip_range::Ipv4Range)s (or supply
//! individual IPv6 hosts) and hand the addresses here.

use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Semaphore;
use tokio::task::JoinSet;

use crate::traits::{probe_address, DiscoveredHost, DiscoveryMethod};

/// Progress snapshot reported after each address finishes probing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScanProgress {
    /// Number of addresses probed so far.
    pub completed: u64,
    /// Total number of addresses in the scan.
    pub total: u64,
}

/// Callback invoked once per completed address with the running [`ScanProgress`].
pub type ProgressCallback = Arc<dyn Fn(ScanProgress) + Send + Sync>;

/// A bounded-concurrency network scanner.
///
/// Build one with [`Scanner::new`], optionally attach a progress callback with
/// [`Scanner::with_progress`], then run [`Scanner::scan`].
#[derive(Clone)]
pub struct Scanner {
    concurrency: usize,
    timeout: Duration,
    on_progress: Option<ProgressCallback>,
}

impl Scanner {
    /// Creates a scanner that probes at most `concurrency` addresses at once,
    /// applying `timeout` to each individual method probe.
    ///
    /// `concurrency` is clamped to at least 1.
    #[must_use]
    pub fn new(concurrency: usize, timeout: Duration) -> Self {
        Self {
            concurrency: concurrency.max(1),
            timeout,
            on_progress: None,
        }
    }

    /// Attaches a progress callback, invoked once per completed address.
    ///
    /// The callback may run on any worker thread and must be cheap; it is meant
    /// for updating a counter or progress bar, not for heavy work.
    #[must_use]
    pub fn with_progress(mut self, callback: ProgressCallback) -> Self {
        self.on_progress = Some(callback);
        self
    }

    /// Scans every address in `targets` with each method in `methods`.
    ///
    /// Returns the responding hosts sorted by address. Methods that error or
    /// time out for a given address are skipped (logged at debug level), never
    /// aborting the overall scan.
    pub async fn scan<I>(
        &self,
        targets: I,
        methods: &[Arc<dyn DiscoveryMethod>],
    ) -> Vec<DiscoveredHost>
    where
        I: IntoIterator<Item = IpAddr>,
    {
        let targets: Vec<IpAddr> = targets.into_iter().collect();
        let total = targets.len() as u64;
        let semaphore = Arc::new(Semaphore::new(self.concurrency));
        let methods: Arc<[Arc<dyn DiscoveryMethod>]> = Arc::from(methods.to_vec());

        let mut tasks: JoinSet<Option<DiscoveredHost>> = JoinSet::new();
        for target in targets {
            let semaphore = Arc::clone(&semaphore);
            let methods = Arc::clone(&methods);
            let timeout = self.timeout;
            tasks.spawn(async move {
                // The semaphore has a 'static lifetime via the Arc, so the
                // permit can be held across the await without lifetime issues.
                let _permit = semaphore
                    .acquire_owned()
                    .await
                    .expect("scanner semaphore is never closed");
                probe_address(target, &methods, timeout).await
            });
        }

        let mut hosts = Vec::new();
        let mut completed = 0u64;
        while let Some(joined) = tasks.join_next().await {
            completed += 1;
            if let Some(callback) = &self.on_progress {
                callback(ScanProgress { completed, total });
            }
            match joined {
                Ok(Some(host)) => hosts.push(host),
                Ok(None) => {}
                Err(err) => tracing::warn!(error = %err, "scan task panicked"),
            }
        }

        hosts.sort_by_key(|host| match host.address {
            IpAddr::V4(v4) => (0u8, u128::from(u32::from(v4))),
            IpAddr::V6(v6) => (1u8, u128::from(v6)),
        });
        hosts
    }
}
