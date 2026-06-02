// SPDX-License-Identifier: GPL-2.0-only

//! IEC 61850 discovery method.
//!
//! Detects Intelligent Electronic Devices (IEDs) by opening a TCP connection to
//! the MMS port (102 by default). A completed handshake marks the host as an
//! IEC 61850 device; the deep nameplate inventory is left to NetInventory (which
//! drives the MMS exchange over [`glpi_iec61850`]). This mirrors how the SNMP
//! method contributes liveness while the heavy lifting happens in the inventory
//! task.

use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

use async_trait::async_trait;
use glpi_core::error::Result;
use tokio::net::TcpStream;

use crate::traits::{DiscoveryMethod, Probe};

/// The registered TCP port for MMS / IEC 61850 (`iso-tsap`).
pub const MMS_PORT: u16 = 102;

/// Discovery method that detects an IED by connecting to its MMS port.
#[derive(Debug, Clone)]
pub struct Iec61850Method {
    port: u16,
    timeout: Duration,
}

impl Iec61850Method {
    /// Creates a method that probes the MMS port, bounding the connect by
    /// `timeout`. Defaults to TCP port 102.
    #[must_use]
    pub fn new(timeout: Duration) -> Self {
        Self {
            port: MMS_PORT,
            timeout,
        }
    }

    /// Overrides the TCP port (default [`MMS_PORT`]).
    #[must_use]
    pub fn with_port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }
}

#[async_trait]
impl DiscoveryMethod for Iec61850Method {
    fn name(&self) -> &'static str {
        "iec61850"
    }

    async fn probe(&self, target: IpAddr) -> Result<Option<Probe>> {
        let addr = SocketAddr::new(target, self.port);
        match tokio::time::timeout(self.timeout, TcpStream::connect(addr)).await {
            // A completed handshake: the host listens on the MMS port.
            Ok(Ok(_stream)) => Ok(Some(Probe::alive())),
            // Refused / unreachable / timed out: not an IED (or not reachable).
            Ok(Err(_)) | Err(_) => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Iec61850Method, MMS_PORT};
    use crate::traits::{DiscoveryMethod, Probe};
    use std::net::{IpAddr, Ipv4Addr};
    use std::time::Duration;
    use tokio::net::TcpListener;

    #[test]
    fn default_port_is_mms_102() {
        assert_eq!(
            Iec61850Method::new(Duration::from_millis(100)).port,
            MMS_PORT
        );
        assert_eq!(MMS_PORT, 102);
    }

    #[tokio::test]
    async fn detects_a_listening_mms_port() {
        // Bind a throwaway listener and point the method at its port.
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let method = Iec61850Method::new(Duration::from_secs(1)).with_port(port);

        let probe = method.probe(IpAddr::V4(Ipv4Addr::LOCALHOST)).await.unwrap();
        assert_eq!(probe, Some(Probe::alive()));
        assert_eq!(method.name(), "iec61850");
    }

    #[tokio::test]
    async fn no_listener_means_not_found() {
        // A port with nothing bound: connection refused -> None.
        let method = Iec61850Method::new(Duration::from_millis(200)).with_port(1);
        let probe = method.probe(IpAddr::V4(Ipv4Addr::LOCALHOST)).await.unwrap();
        assert_eq!(probe, None);
    }
}
