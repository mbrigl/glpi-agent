// SPDX-License-Identifier: GPL-2.0-only

//! Agent configuration loading.
//!
//! Configuration is built by layering several sources, lowest precedence
//! first:
//!
//! 1. compiled-in defaults ([`Options::default`]),
//! 2. the main `agent.cfg` file,
//! 3. `conf.d/*.cfg` drop-ins (lexical order),
//! 4. the Windows registry (Windows only),
//! 5. environment variables,
//! 6. command-line arguments.
//!
//! Each source is parsed into a [`PartialOptions`] layer and folded together by
//! [`Options::resolve`], so a higher-precedence source only overrides the exact
//! keys it sets.
//!
//! Only the merge machinery exists today; the individual source parsers
//! (`agent.cfg`, `conf.d`, registry) are implemented later in Phase 1 and will
//! live in `sources.rs`.

mod options;

pub use options::{Options, PartialOptions, DEFAULT_DELAYTIME, DEFAULT_HTTPD_PORT};

use crate::error::Result;

/// Loads the effective configuration.
///
/// At this stage only the compiled-in defaults are returned. As the source
/// parsers land, this function will gather one [`PartialOptions`] per source
/// and hand them to [`Options::resolve`] in precedence order.
///
/// # Errors
///
/// Returns an error once source parsing is implemented and a source is present
/// but malformed. The current default-only implementation is infallible.
pub fn load() -> Result<Options> {
    let layers: Vec<PartialOptions> = Vec::new();
    Ok(Options::resolve(&layers))
}

#[cfg(test)]
mod tests {
    use super::{load, Options};

    #[test]
    fn load_returns_defaults_for_now() {
        assert_eq!(load().unwrap(), Options::default());
    }
}
