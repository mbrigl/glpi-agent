// SPDX-License-Identifier: GPL-2.0-only

//! Agent configuration loading.
//!
//! Configuration is built by layering several sources, lowest precedence
//! first:
//!
//! 1. compiled-in defaults ([`Options::default`]),
//! 2. the main `agent.cfg` file,
//! 3. `conf.d/*.cfg` drop-ins (lexical order),
//! 4. the Windows registry (Windows only — not implemented yet),
//! 5. environment variables (`GLPI_AGENT_*`),
//! 6. command-line arguments (assembled by the CLI crate).
//!
//! Each source is parsed into a [`PartialOptions`] layer (see [`sources`]) and
//! folded together by [`Options::resolve`], so a higher-precedence source only
//! overrides the exact keys it sets.
//!
//! [`Loader`] assembles the file/conf.d/environment layers in order;
//! command-line arguments are appended by the caller as a final layer, and the
//! Windows registry source is still to come.

mod options;
pub mod sources;

use std::path::{Path, PathBuf};

pub use options::{Options, PartialOptions, DEFAULT_DELAYTIME, DEFAULT_HTTPD_PORT};

use crate::error::Result;

/// Assembles configuration layers from the on-disk and environment sources, in
/// precedence order.
///
/// A caller typically builds a `Loader`, resolves it to the base [`Options`],
/// and then applies any command-line layer on top via [`Options::resolve`] (or
/// [`PartialOptions::apply`]).
#[derive(Debug, Clone, Default)]
pub struct Loader {
    cfg_file: Option<PathBuf>,
    conf_dir: Option<PathBuf>,
    include_env: bool,
}

impl Loader {
    /// Creates an empty loader (no sources). Add sources with the builder
    /// methods; resolving an empty loader yields [`Options::default`].
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the main `agent.cfg` file to read.
    #[must_use]
    pub fn with_cfg_file(mut self, path: impl Into<PathBuf>) -> Self {
        self.cfg_file = Some(path.into());
        self
    }

    /// Sets the `conf.d` directory to scan for `*.cfg` drop-ins.
    #[must_use]
    pub fn with_conf_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.conf_dir = Some(path.into());
        self
    }

    /// Includes the process environment (`GLPI_AGENT_*`) as the top layer.
    #[must_use]
    pub fn with_env(mut self) -> Self {
        self.include_env = true;
        self
    }

    /// Gathers the configured sources into ordered [`PartialOptions`] layers.
    ///
    /// Order is: `agent.cfg`, then `conf.d/*.cfg` (lexical), then the
    /// environment — lowest precedence first.
    ///
    /// # Errors
    ///
    /// Propagates any read or parse error from the underlying sources.
    pub fn layers(&self) -> Result<Vec<PartialOptions>> {
        let mut layers = Vec::new();
        if let Some(path) = &self.cfg_file {
            layers.push(sources::load_cfg_file(path)?);
        }
        if let Some(dir) = &self.conf_dir {
            layers.extend(sources::load_conf_dir(dir)?);
        }
        if self.include_env {
            layers.push(sources::from_env(std::env::vars())?);
        }
        Ok(layers)
    }

    /// Resolves the configured sources to a final [`Options`], on top of the
    /// compiled-in defaults.
    ///
    /// # Errors
    ///
    /// Propagates any read or parse error from [`Loader::layers`].
    pub fn resolve(&self) -> Result<Options> {
        Ok(Options::resolve(&self.layers()?))
    }
}

/// Loads the effective configuration from the standard file, `conf.d`, and the
/// environment.
///
/// This is a convenience wrapper over [`Loader`]. Command-line arguments are
/// layered on by the CLI crate, which has them.
///
/// # Errors
///
/// Propagates any read or parse error from the underlying sources.
pub fn load(cfg_file: impl AsRef<Path>, conf_dir: impl AsRef<Path>) -> Result<Options> {
    Loader::new()
        .with_cfg_file(cfg_file.as_ref())
        .with_conf_dir(conf_dir.as_ref())
        .with_env()
        .resolve()
}

#[cfg(test)]
mod tests {
    use super::{Loader, Options};
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_dir() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir =
            std::env::temp_dir().join(format!("glpi-core-loader-{}-{nanos}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn empty_loader_resolves_to_defaults() {
        assert_eq!(Loader::new().resolve().unwrap(), Options::default());
    }

    #[test]
    fn conf_d_overrides_agent_cfg() {
        let dir = unique_dir();
        let cfg = dir.join("agent.cfg");
        let confd = dir.join("conf.d");
        fs::create_dir_all(&confd).unwrap();
        fs::write(&cfg, "tag = base\nserver = http://base.example").unwrap();
        fs::write(confd.join("10-override.cfg"), "tag = overridden").unwrap();

        let options = Loader::new()
            .with_cfg_file(&cfg)
            .with_conf_dir(&confd)
            .resolve()
            .unwrap();
        fs::remove_dir_all(&dir).ok();

        // conf.d wins for `tag`, but the agent.cfg-only `server` survives.
        assert_eq!(options.tag.as_deref(), Some("overridden"));
        assert_eq!(options.server, vec!["http://base.example".to_owned()]);
    }

    #[test]
    fn missing_cfg_file_is_an_error() {
        let result = Loader::new()
            .with_cfg_file("/nonexistent/agent.cfg")
            .resolve();
        assert!(result.is_err());
    }
}
