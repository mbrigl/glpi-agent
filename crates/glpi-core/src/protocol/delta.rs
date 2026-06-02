// SPDX-License-Identifier: GPL-2.0-only

//! Delta (partial) inventory state.
//!
//! To avoid re-sending an unchanged inventory in full, the agent keeps a small
//! per-device *state file* holding a checksum of each content section. On the
//! next run it compares the freshly collected content against that state and,
//! when nothing forces a full inventory, submits only the changed sections
//! flagged as `partial`. `full-inventory-postpone` bounds how many consecutive
//! partials may be sent before a full inventory is forced again.
//!
//! The logic is generic over any `Serialize` content (it works on the GLPI
//! native JSON object), so it serves both local and remote inventory without
//! touching the typed payloads. Section checksums use the deterministic
//! [`DefaultHasher`] (fixed keys), so they are stable across agent runs.

use std::collections::hash_map::DefaultHasher;
use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{AgentError, Result};

/// The content key that identifies the agent; always kept in a partial.
const VERSION_CLIENT: &str = "versionclient";
/// The boolean flag marking a content object as a partial inventory.
const PARTIAL_FLAG: &str = "partial";

/// Persisted per-device inventory state.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct InventoryState {
    /// Section key → checksum of its last-submitted value.
    pub checksums: BTreeMap<String, u64>,
    /// Number of consecutive partial inventories sent since the last full one.
    #[serde(default)]
    pub since_full: u32,
}

/// Whether a planned submission is a full or a partial inventory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InventoryMode {
    /// A complete inventory.
    Full,
    /// A partial inventory carrying only the changed sections.
    Partial,
}

/// The outcome of [`plan`]: the content to submit, the sections it covers, and
/// the new state to persist afterwards.
#[derive(Debug, Clone)]
pub struct DeltaPlan {
    /// Full or partial.
    pub mode: InventoryMode,
    /// The content object to submit (a partial carries `"partial": true`).
    pub content: Value,
    /// Section keys included (empty on an unchanged partial).
    pub changed_sections: Vec<String>,
    /// State to write back with [`save_state`].
    pub state: InventoryState,
}

/// Computes a deterministic checksum per content section (skipping
/// `versionclient`), keyed by the GLPI content key.
///
/// # Errors
///
/// Returns [`AgentError::Json`] if `content` does not serialize to a JSON object.
pub fn section_checksums<T: Serialize>(content: &T) -> Result<BTreeMap<String, u64>> {
    let value = serde_json::to_value(content)?;
    let object = value
        .as_object()
        .ok_or_else(|| AgentError::Protocol("inventory content is not a JSON object".to_owned()))?;
    let mut checksums = BTreeMap::new();
    for (key, section) in object {
        if key == VERSION_CLIENT || key == PARTIAL_FLAG {
            continue;
        }
        checksums.insert(key.clone(), checksum(section));
    }
    Ok(checksums)
}

/// Hashes a section's canonical JSON with the deterministic [`DefaultHasher`].
fn checksum(section: &Value) -> u64 {
    // `serde_json` preserves struct field and array order, so the serialized
    // form is stable for equal values.
    let serialized = section.to_string();
    let mut hasher = DefaultHasher::new();
    serialized.hash(&mut hasher);
    hasher.finish()
}

/// Plans the next submission for `content` given the `previous` state and the
/// `full-inventory-postpone` bound (`0` disables partials → always full).
///
/// A full inventory is forced when there is no previous state, when
/// `max_postpone` is `0`, or when `max_postpone` consecutive partials have
/// already been sent. Otherwise only the changed sections are included.
///
/// # Errors
///
/// Propagates [`section_checksums`] serialization failures.
pub fn plan<T: Serialize>(
    content: &T,
    previous: Option<&InventoryState>,
    max_postpone: u32,
) -> Result<DeltaPlan> {
    let value = serde_json::to_value(content)?;
    let current = section_checksums(&value)?;

    let force_full =
        max_postpone == 0 || previous.is_none_or(|state| state.since_full >= max_postpone);

    if force_full {
        return Ok(DeltaPlan {
            mode: InventoryMode::Full,
            changed_sections: current.keys().cloned().collect(),
            content: value,
            state: InventoryState {
                checksums: current,
                since_full: 0,
            },
        });
    }

    // Safe: `force_full` is true when `previous` is `None`.
    let previous = previous.expect("previous state present for partial");
    let changed: Vec<String> = current
        .iter()
        .filter(|(key, sum)| previous.checksums.get(*key) != Some(*sum))
        .map(|(key, _)| key.clone())
        .collect();

    let content = partial_content(&value, &changed);
    Ok(DeltaPlan {
        mode: InventoryMode::Partial,
        changed_sections: changed,
        content,
        state: InventoryState {
            checksums: current,
            since_full: previous.since_full + 1,
        },
    })
}

/// Builds a partial content object: `versionclient`, the `changed` sections and
/// `"partial": true`.
fn partial_content(full: &Value, changed: &[String]) -> Value {
    let mut object = serde_json::Map::new();
    if let Some(version) = full.get(VERSION_CLIENT) {
        object.insert(VERSION_CLIENT.to_owned(), version.clone());
    }
    for key in changed {
        if let Some(section) = full.get(key) {
            object.insert(key.clone(), section.clone());
        }
    }
    object.insert(PARTIAL_FLAG.to_owned(), Value::Bool(true));
    Value::Object(object)
}

/// Returns the state-file path for `deviceid` under `dir`.
#[must_use]
pub fn state_path(dir: &Path, deviceid: &str) -> PathBuf {
    dir.join(format!("{}.json", sanitize(deviceid)))
}

/// Loads the saved state for `deviceid`, or `None` when no state file exists.
///
/// # Errors
///
/// Returns an error on an unreadable or malformed state file.
pub fn load_state(dir: &Path, deviceid: &str) -> Result<Option<InventoryState>> {
    let path = state_path(dir, deviceid);
    match std::fs::read(&path) {
        Ok(bytes) => Ok(Some(serde_json::from_slice(&bytes)?)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(AgentError::Io(e)),
    }
}

/// Writes `state` for `deviceid` under `dir`, creating `dir` if needed.
///
/// # Errors
///
/// Returns an error if the directory cannot be created or the file written.
pub fn save_state(dir: &Path, deviceid: &str, state: &InventoryState) -> Result<()> {
    std::fs::create_dir_all(dir).map_err(AgentError::Io)?;
    let json = serde_json::to_vec_pretty(state)?;
    std::fs::write(state_path(dir, deviceid), json).map_err(AgentError::Io)
}

/// Maps a device id to a safe single-path-component file stem.
fn sanitize(deviceid: &str) -> String {
    deviceid
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') {
                c
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{
        load_state, plan, save_state, section_checksums, state_path, InventoryMode, InventoryState,
    };
    use serde_json::json;

    fn content(cpu_speed: u64, software: &str) -> serde_json::Value {
        json!({
            "versionclient": "GLPI-Agent_v2.0.0",
            "cpus": [{ "name": "Xeon", "speed": cpu_speed }],
            "softwares": [{ "name": software }],
        })
    }

    #[test]
    fn checksums_are_stable_and_section_scoped() {
        let a = section_checksums(&content(2100, "bash")).unwrap();
        let b = section_checksums(&content(2100, "bash")).unwrap();
        assert_eq!(a, b);
        assert!(a.contains_key("cpus") && a.contains_key("softwares"));
        // versionclient is never tracked as a section.
        assert!(!a.contains_key("versionclient"));

        // Changing one section changes only its checksum.
        let c = section_checksums(&content(2100, "zsh")).unwrap();
        assert_eq!(a["cpus"], c["cpus"]);
        assert_ne!(a["softwares"], c["softwares"]);
    }

    #[test]
    fn first_run_is_full_and_resets_counter() {
        let plan = plan(&content(2100, "bash"), None, 5).unwrap();
        assert_eq!(plan.mode, InventoryMode::Full);
        assert_eq!(plan.state.since_full, 0);
        assert!(plan.content.get("partial").is_none());
    }

    #[test]
    fn postpone_zero_is_always_full() {
        let prev = InventoryState::default();
        let plan = plan(&content(2100, "bash"), Some(&prev), 0).unwrap();
        assert_eq!(plan.mode, InventoryMode::Full);
    }

    #[test]
    fn partial_includes_only_changed_sections() {
        // Establish a baseline state.
        let base = plan(&content(2100, "bash"), None, 5).unwrap();
        // Next run: only the software changed.
        let next = plan(&content(2100, "zsh"), Some(&base.state), 5).unwrap();
        assert_eq!(next.mode, InventoryMode::Partial);
        assert_eq!(next.changed_sections, vec!["softwares".to_owned()]);
        assert_eq!(next.content["partial"], json!(true));
        assert!(next.content.get("softwares").is_some());
        assert!(next.content.get("cpus").is_none());
        // versionclient is always carried.
        assert_eq!(next.content["versionclient"], json!("GLPI-Agent_v2.0.0"));
        assert_eq!(next.state.since_full, 1);
    }

    #[test]
    fn full_is_forced_after_max_postpone_partials() {
        let prev = InventoryState {
            since_full: 3,
            ..InventoryState::default()
        };
        let plan = plan(&content(2100, "bash"), Some(&prev), 3).unwrap();
        assert_eq!(plan.mode, InventoryMode::Full);
        assert_eq!(plan.state.since_full, 0);
    }

    #[test]
    fn state_round_trips_on_disk() {
        let dir = std::env::temp_dir().join(format!("glpi-delta-{}", std::process::id()));
        let deviceid = "host/with:weird*chars";
        let state = plan(&content(2100, "bash"), None, 5).unwrap().state;
        save_state(&dir, deviceid, &state).unwrap();
        // The path is a single sanitised component.
        assert!(state_path(&dir, deviceid)
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("host_with_weird_chars"));
        let loaded = load_state(&dir, deviceid).unwrap().unwrap();
        assert_eq!(loaded, state);
        // Unknown device → None.
        assert!(load_state(&dir, "never-seen").unwrap().is_none());
        std::fs::remove_dir_all(&dir).ok();
    }
}
