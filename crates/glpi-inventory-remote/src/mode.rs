// SPDX-License-Identifier: GPL-2.0-only

//! Remote SSH modes (`ssh`, `libssh2`, `perl`).
//!
//! The `remote` URL may carry a `?mode=` option whose value is an
//! underscore-separated list of modes (e.g. `ssh_perl`), matching the upstream
//! agent. `ssh` and `libssh2` select the transport; `perl` is an opt-in
//! enhancement that lets the orchestrator use remote `perl` one-liners where
//! they are richer than plain coreutils. Unknown tokens are ignored.

use crate::target::RemoteTarget;

/// The set of enabled remote modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RemoteModes {
    /// Use the command-line `ssh` transport.
    pub ssh: bool,
    /// Use the libssh2 (russh) transport.
    pub libssh2: bool,
    /// Allow remote `perl` one-liners (opt-in).
    pub perl: bool,
}

impl RemoteModes {
    /// Parses an underscore-separated mode list (`ssh_perl`, `perl`, …);
    /// unrecognised tokens are ignored, mirroring the upstream agent.
    #[must_use]
    pub fn parse(value: &str) -> Self {
        let mut modes = Self::default();
        for token in value.split('_') {
            match token.trim().to_ascii_lowercase().as_str() {
                "ssh" => modes.ssh = true,
                "libssh2" => modes.libssh2 = true,
                "perl" => modes.perl = true,
                _ => {}
            }
        }
        modes
    }

    /// Reads the modes from a target's `?mode=` option (none → all-default).
    #[must_use]
    pub fn from_target(target: &RemoteTarget) -> Self {
        target.option("mode").map(Self::parse).unwrap_or_default()
    }

    /// `true` if the `perl` enhancement mode is enabled.
    #[must_use]
    pub fn perl(&self) -> bool {
        self.perl
    }
}

#[cfg(test)]
mod tests {
    use super::RemoteModes;
    use crate::target::RemoteTarget;

    #[test]
    fn parses_underscore_list() {
        let m = RemoteModes::parse("ssh_perl");
        assert!(m.ssh && m.perl && !m.libssh2);
        assert_eq!(
            RemoteModes::parse("perl"),
            RemoteModes {
                ssh: false,
                libssh2: false,
                perl: true
            }
        );
        // Unknown tokens ignored.
        assert_eq!(RemoteModes::parse("bogus"), RemoteModes::default());
    }

    #[test]
    fn reads_from_target_option() {
        let t = RemoteTarget::parse("ssh://host/?mode=libssh2_perl").unwrap();
        let m = RemoteModes::from_target(&t);
        assert!(m.libssh2 && m.perl && !m.ssh);
        // No ?mode= → empty (auto).
        let t = RemoteTarget::parse("ssh://host").unwrap();
        assert_eq!(RemoteModes::from_target(&t), RemoteModes::default());
    }
}
