// SPDX-License-Identifier: GPL-2.0-only

//! Configuration source parsers.
//!
//! Each source is read into a [`PartialOptions`] layer; the caller folds the
//! layers together in precedence order with [`super::Options::resolve`]. Two
//! textual sources share one key→field mapper ([`apply_pair`]):
//!
//! - the main `agent.cfg` and the `conf.d/*.cfg` drop-ins, in the upstream
//!   agent's `key = value` format (`#` / `;` comments, blank lines ignored),
//! - environment variables of the form `GLPI_AGENT_<OPTION>` (for example
//!   `GLPI_AGENT_NO_TASK=deploy,wakeonlan`).
//!
//! List-valued options (`server`, `tasks`, `no-task`, `no-category`,
//! `httpd-trust`) accept a comma-separated value; boolean options accept
//! `1/0`, `true/false`, `yes/no`, `on/off`.

use std::fs;
use std::path::Path;

use super::options::PartialOptions;
use crate::error::{AgentError, Result};

/// Environment-variable prefix recognized by [`from_env`].
const ENV_PREFIX: &str = "GLPI_AGENT_";

/// Parses the upstream `key = value` config format into a layer.
///
/// Lines that are blank or start with `#` / `;` (after trimming) are ignored.
/// Unknown keys are ignored too, so a real-world `agent.cfg` carrying options
/// this subset does not model yet still loads.
///
/// # Errors
///
/// Returns [`AgentError::Config`] for a non-comment line without a `=`, or
/// [`AgentError::Parse`] when a known key has an unparseable value.
pub fn parse_cfg(text: &str) -> Result<PartialOptions> {
    let mut partial = PartialOptions::default();
    for (lineno, raw) in text.lines().enumerate() {
        let line = strip_comment(raw).trim();
        if line.is_empty() {
            continue;
        }
        let (key, value) = line.split_once('=').ok_or_else(|| {
            AgentError::Config(format!("malformed config line {}: `{raw}`", lineno + 1))
        })?;
        apply_pair(&mut partial, key.trim(), unquote(value.trim()))?;
    }
    Ok(partial)
}

/// Reads and parses a single `agent.cfg`-style file.
///
/// # Errors
///
/// Returns [`AgentError::Io`] if the file cannot be read, or propagates a
/// parse error from [`parse_cfg`].
pub fn load_cfg_file(path: impl AsRef<Path>) -> Result<PartialOptions> {
    let text = fs::read_to_string(path)?;
    parse_cfg(&text)
}

/// Loads every `*.cfg` drop-in from a `conf.d` directory, in lexical order.
///
/// A missing directory yields an empty layer list (it is optional). Each file
/// becomes its own layer, so later files override earlier ones once resolved.
///
/// # Errors
///
/// Returns an error if the directory cannot be listed, a `*.cfg` file cannot be
/// read, or a file fails to parse.
pub fn load_conf_dir(dir: impl AsRef<Path>) -> Result<Vec<PartialOptions>> {
    let dir = dir.as_ref();
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut paths: Vec<_> = fs::read_dir(dir)?
        .collect::<std::io::Result<Vec<_>>>()?
        .into_iter()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "cfg"))
        .collect();
    paths.sort();

    let mut layers = Vec::with_capacity(paths.len());
    for path in paths {
        layers.push(load_cfg_file(&path)?);
    }
    Ok(layers)
}

/// Builds a layer from environment variables prefixed with `GLPI_AGENT_`.
///
/// The remainder of each name is lower-cased and `_` becomes `-`, so
/// `GLPI_AGENT_NO_TASK` maps to the `no-task` option. Pass
/// `std::env::vars()` in production; the explicit iterator keeps the parser
/// testable without touching the process environment.
///
/// # Errors
///
/// Propagates a parse error from [`apply_pair`] for a known key with an
/// unparseable value.
pub fn from_env<I, K, V>(vars: I) -> Result<PartialOptions>
where
    I: IntoIterator<Item = (K, V)>,
    K: AsRef<str>,
    V: AsRef<str>,
{
    let mut partial = PartialOptions::default();
    for (name, value) in vars {
        let Some(rest) = name.as_ref().strip_prefix(ENV_PREFIX) else {
            continue;
        };
        let key = rest.to_ascii_lowercase().replace('_', "-");
        apply_pair(&mut partial, &key, value.as_ref())?;
    }
    Ok(partial)
}

/// Sets the field matching `key` on `partial` from `value`.
///
/// Unknown keys are ignored. Shared by the cfg and env parsers so both honour
/// the same option names and value formats.
fn apply_pair(partial: &mut PartialOptions, key: &str, value: &str) -> Result<()> {
    match key {
        "server" => partial.server = Some(list(value)),
        "local" => partial.local = Some(value.into()),
        "proxy" => partial.proxy = Some(value.to_owned()),
        "tag" => partial.tag = Some(value.to_owned()),
        "tasks" => partial.tasks = Some(list(value)),
        "no-task" => partial.no_task = Some(list(value)),
        "no-category" => partial.no_category = Some(list(value)),
        "delaytime" => partial.delaytime = Some(number(key, value)?),
        "lazy" => partial.lazy = Some(boolean(key, value)?),
        "conf-reload-interval" => partial.conf_reload_interval = Some(number(key, value)?),
        "no-httpd" => partial.no_httpd = Some(boolean(key, value)?),
        "httpd-ip" => partial.httpd_ip = Some(value.to_owned()),
        "httpd-port" => partial.httpd_port = Some(number(key, value)?),
        "httpd-trust" => partial.httpd_trust = Some(list(value)),
        "debug" => partial.debug = Some(number(key, value)?),
        "no-ssl-check" => partial.no_ssl_check = Some(boolean(key, value)?),
        _ => {}
    }
    Ok(())
}

/// Splits a comma-separated list, trimming items and dropping empty ones.
fn list(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

/// Parses any integer option, attaching the key name to a parse error.
fn number<T: std::str::FromStr>(key: &str, value: &str) -> Result<T> {
    value
        .parse()
        .map_err(|_| AgentError::Parse(format!("option `{key}`: invalid number `{value}`")))
}

/// Parses a boolean from the accepted truthy/falsy spellings.
fn boolean(key: &str, value: &str) -> Result<bool> {
    match value.to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "0" | "false" | "no" | "off" => Ok(false),
        _ => Err(AgentError::Parse(format!(
            "option `{key}`: invalid boolean `{value}`"
        ))),
    }
}

/// Strips an inline `#`/`;` comment from a config line.
fn strip_comment(line: &str) -> &str {
    match line.find(['#', ';']) {
        Some(idx) => &line[..idx],
        None => line,
    }
}

/// Removes one matching pair of surrounding single or double quotes.
fn unquote(value: &str) -> &str {
    let bytes = value.as_bytes();
    if bytes.len() >= 2
        && (bytes[0] == b'"' || bytes[0] == b'\'')
        && bytes[bytes.len() - 1] == bytes[0]
    {
        &value[1..value.len() - 1]
    } else {
        value
    }
}

#[cfg(test)]
mod tests {
    use super::{from_env, load_conf_dir, parse_cfg};
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_dir() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir =
            std::env::temp_dir().join(format!("glpi-core-confd-{}-{nanos}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn parses_keys_comments_and_lists() {
        let cfg = "
            # main server list
            server = http://a.example, http://b.example
            tag = lab   ; inline comment
            no-task = deploy,wakeonlan
            debug = 2
            no-httpd = yes
        ";
        let partial = parse_cfg(cfg).unwrap();
        assert_eq!(
            partial.server.unwrap(),
            vec!["http://a.example".to_owned(), "http://b.example".to_owned()]
        );
        assert_eq!(partial.tag.as_deref(), Some("lab"));
        assert_eq!(partial.no_task.unwrap(), vec!["deploy", "wakeonlan"]);
        assert_eq!(partial.debug, Some(2));
        assert_eq!(partial.no_httpd, Some(true));
    }

    #[test]
    fn strips_surrounding_quotes() {
        let partial = parse_cfg("tag = \"quoted value\"").unwrap();
        assert_eq!(partial.tag.as_deref(), Some("quoted value"));
    }

    #[test]
    fn rejects_line_without_equals() {
        assert!(parse_cfg("this is not valid").is_err());
    }

    #[test]
    fn rejects_invalid_number() {
        assert!(parse_cfg("httpd-port = notaport").is_err());
    }

    #[test]
    fn ignores_unknown_keys() {
        let partial = parse_cfg("totally-unknown = whatever\ntag = kept").unwrap();
        assert_eq!(partial.tag.as_deref(), Some("kept"));
    }

    #[test]
    fn env_maps_underscores_to_dashes() {
        let partial = from_env([
            ("GLPI_AGENT_SERVER", "http://srv.example"),
            ("GLPI_AGENT_NO_TASK", "deploy"),
            ("PATH", "/ignored"),
        ])
        .unwrap();
        assert_eq!(partial.server.unwrap(), vec!["http://srv.example"]);
        assert_eq!(partial.no_task.unwrap(), vec!["deploy"]);
    }

    #[test]
    fn conf_dir_loads_cfg_files_in_lexical_order() {
        let dir = unique_dir();
        fs::write(dir.join("20-second.cfg"), "tag = second").unwrap();
        fs::write(dir.join("10-first.cfg"), "tag = first").unwrap();
        fs::write(dir.join("notes.txt"), "tag = ignored").unwrap();

        let layers = load_conf_dir(&dir).unwrap();
        fs::remove_dir_all(&dir).ok();

        assert_eq!(layers.len(), 2);
        assert_eq!(layers[0].tag.as_deref(), Some("first"));
        assert_eq!(layers[1].tag.as_deref(), Some("second"));
    }

    #[test]
    fn missing_conf_dir_is_empty() {
        let layers = load_conf_dir("/nonexistent/glpi/conf.d").unwrap();
        assert!(layers.is_empty());
    }
}
