// SPDX-License-Identifier: GPL-2.0-only

//! Agent events.
//!
//! Ported from the upstream `GLPI::Agent::Event`: the daemon and HTTP control
//! server represent work to do as typed [`Event`]s. Six kinds exist
//! ([`EventKind`]): `init` and `maintenance` and `job` are internal (not
//! HTTP-triggerable), while `runnow`, `taskrun` and `partial` can be raised by
//! `/now` HTTP requests. [`Event::from_params`] builds an event from such a
//! request's parameters, and the serde representation carries an event across
//! the task-fork IPC channel.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// The kind of an [`Event`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EventKind {
    /// Service start-up (internal; not HTTP-triggerable).
    Init,
    /// Run one or more tasks now (HTTP `/now`).
    RunNow,
    /// Run a single planned task (HTTP `/now`, or following a `RunNow`).
    TaskRun,
    /// Partial inventory of selected categories (HTTP `/now`, or `--partial`).
    Partial,
    /// Internal maintenance trigger (e.g. deploy storage cleanup).
    Maintenance,
    /// A toolbox-managed inventory job (internal).
    Job,
}

/// A unit of work for the daemon / scheduler.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Event {
    /// The event kind.
    pub kind: EventKind,
    /// Human-readable name (mandatory; an event without one is invalid).
    pub name: String,
    /// Target task (`""`, a task name, or `all`).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub task: String,
    /// Delay in seconds before the event runs.
    #[serde(default, skip_serializing_if = "is_zero_u64")]
    pub delay: u64,
    /// Scheduled run date (epoch seconds; `init` / `job`).
    #[serde(default, skip_serializing_if = "is_zero_i64")]
    pub rundate: i64,
    /// Categories for a `partial` inventory.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub category: String,
    /// Optional target identifier the event is bound to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    /// Whether the event may be triggered over HTTP.
    #[serde(default, skip_serializing_if = "is_false")]
    pub httpd_support: bool,
    /// Extra parameters (e.g. `full`, `reschedule`, or request-specific keys).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub params: BTreeMap<String, String>,
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_zero_u64(v: &u64) -> bool {
    *v == 0
}
#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_zero_i64(v: &i64) -> bool {
    *v == 0
}
#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_false(v: &bool) -> bool {
    !*v
}

impl Event {
    /// An `init` event (service start), optionally for a single `task`.
    #[must_use]
    pub fn init(task: impl Into<String>, rundate: i64) -> Self {
        Self {
            kind: EventKind::Init,
            name: "init".to_owned(),
            task: task.into(),
            rundate,
            httpd_support: false,
            ..Self::empty(EventKind::Init)
        }
    }

    /// A `runnow` event for `task` (a task name, `all`, or empty → `all`).
    #[must_use]
    pub fn run_now(task: impl Into<String>, delay: u64, params: BTreeMap<String, String>) -> Self {
        let task = task.into();
        Self {
            kind: EventKind::RunNow,
            name: "run now".to_owned(),
            task: if task.is_empty() {
                "all".to_owned()
            } else {
                task
            },
            delay,
            httpd_support: true,
            params,
            ..Self::empty(EventKind::RunNow)
        }
    }

    /// A `taskrun` event for the planned `task`. For the inventory task, `full`
    /// selects a full (`Some(true)`) or partial (`Some(false)`) run; `None`
    /// defaults to full. `reschedule` resets the target's next run date.
    #[must_use]
    pub fn task_run(
        task: impl Into<String>,
        delay: u64,
        reschedule: bool,
        full: Option<bool>,
    ) -> Self {
        let task = task.into();
        let mut params = BTreeMap::new();
        params.insert("reschedule".to_owned(), bool_flag(reschedule));
        if task == "inventory" {
            params.insert("full".to_owned(), bool_flag(full.unwrap_or(true)));
        }
        Self {
            kind: EventKind::TaskRun,
            name: "run".to_owned(),
            task,
            delay,
            httpd_support: true,
            params,
            ..Self::empty(EventKind::TaskRun)
        }
    }

    /// A `partial` inventory event for the given `category` list.
    #[must_use]
    pub fn partial(category: impl Into<String>, params: BTreeMap<String, String>) -> Self {
        Self {
            kind: EventKind::Partial,
            name: "partial inventory".to_owned(),
            task: "inventory".to_owned(),
            category: category.into(),
            httpd_support: true,
            params,
            ..Self::empty(EventKind::Partial)
        }
    }

    /// A `maintenance` event for `task`.
    #[must_use]
    pub fn maintenance(task: impl Into<String>, name: impl Into<String>, delay: u64) -> Self {
        Self {
            kind: EventKind::Maintenance,
            name: name.into(),
            task: task.into(),
            delay,
            httpd_support: false,
            ..Self::empty(EventKind::Maintenance)
        }
    }

    /// A `job` event (toolbox-managed inventory).
    #[must_use]
    pub fn job(name: impl Into<String>, task: impl Into<String>, rundate: i64) -> Self {
        Self {
            kind: EventKind::Job,
            name: name.into(),
            task: task.into(),
            rundate,
            httpd_support: false,
            ..Self::empty(EventKind::Job)
        }
    }

    /// Builds an event from request parameters (mirrors `Event->new(%params)`),
    /// dispatching on the first set kind flag; `None` for an unrecognised
    /// request. `init` / `maintenance` are accepted here for internal use even
    /// though HTTP callers never set them.
    #[must_use]
    pub fn from_params(params: &BTreeMap<String, String>) -> Option<Self> {
        let flag = |key: &str| params.get(key).is_some_and(|v| is_yes(v));
        let get = |key: &str| params.get(key).cloned().unwrap_or_default();
        let rundate = || get("rundate").parse().unwrap_or(0);
        let delay = || get("delay").parse().unwrap_or(0);

        if flag("init") {
            Some(Self::init(get("task"), rundate()))
        } else if flag("runnow") {
            let task = match (params.get("task"), params.get("tasks")) {
                (Some(t), _) if !t.is_empty() => t.clone(),
                (_, Some(t)) if !t.is_empty() => t.clone(),
                _ => String::new(),
            };
            let extra = extra_params(params, &["runnow", "task", "tasks", "delay"]);
            Some(Self::run_now(task, delay(), extra))
        } else if flag("taskrun") {
            let task = get("task");
            let full = if task == "inventory" {
                Some(inventory_full(params))
            } else {
                None
            };
            Some(Self::task_run(task, delay(), flag("reschedule"), full))
        } else if flag("partial") {
            let extra = extra_params(params, &["partial"]);
            Some(Self::partial(get("category"), extra))
        } else if flag("maintenance") {
            let name = params
                .get("name")
                .filter(|n| !n.is_empty())
                .cloned()
                .unwrap_or_else(|| "maintenance".to_owned());
            Some(Self::maintenance(get("task"), name, delay()))
        } else if flag("job") && params.get("name").is_some_and(|n| !n.is_empty()) {
            Some(Self::job(get("name"), get("task"), rundate()))
        } else {
            None
        }
    }

    /// Reads an extra parameter by key.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&str> {
        self.params.get(key).map(String::as_str)
    }

    /// `&`-joined `key=value` form of the scalar fields (the upstream
    /// `dump_as_string`), used in text logs / URLs.
    #[must_use]
    pub fn to_query_string(&self) -> String {
        let mut pairs = vec![format!("name={}", self.name), format!("task={}", self.task)];
        if self.delay != 0 {
            pairs.push(format!("delay={}", self.delay));
        }
        if self.rundate != 0 {
            pairs.push(format!("rundate={}", self.rundate));
        }
        if !self.category.is_empty() {
            pairs.push(format!("category={}", self.category));
        }
        pairs.join("&")
    }

    /// An otherwise-empty event of `kind` (used as a struct-update base).
    fn empty(kind: EventKind) -> Self {
        Self {
            kind,
            name: String::new(),
            task: String::new(),
            delay: 0,
            rundate: 0,
            category: String::new(),
            target: None,
            httpd_support: false,
            params: BTreeMap::new(),
        }
    }
}

/// `"1"`/`"0"` for a flag, matching the upstream string-valued params.
fn bool_flag(value: bool) -> String {
    if value { "1" } else { "0" }.to_owned()
}

/// Whether a parameter value means "yes"/"1".
fn is_yes(value: &str) -> bool {
    value == "1" || value.eq_ignore_ascii_case("yes")
}

/// The inventory `full` decision: explicit `full` wins, else the inverse of
/// `partial`, else full by default.
fn inventory_full(params: &BTreeMap<String, String>) -> bool {
    if let Some(full) = params.get("full") {
        is_yes(full)
    } else if let Some(partial) = params.get("partial") {
        !is_yes(partial)
    } else {
        true
    }
}

/// Collects parameters except the given keys.
fn extra_params(params: &BTreeMap<String, String>, exclude: &[&str]) -> BTreeMap<String, String> {
    params
        .iter()
        .filter(|(key, _)| !exclude.contains(&key.as_str()))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{Event, EventKind};
    use std::collections::BTreeMap;

    fn params(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect()
    }

    #[test]
    fn run_now_defaults_task_to_all_and_keeps_extra_params() {
        let event = Event::run_now("", 0, params(&[("full", "1")]));
        assert_eq!(event.kind, EventKind::RunNow);
        assert_eq!(event.task, "all");
        assert!(event.httpd_support);
        assert_eq!(event.get("full"), Some("1"));
    }

    #[test]
    fn task_run_inventory_full_precedence() {
        // full defaults to true for inventory.
        assert_eq!(
            Event::task_run("inventory", 0, false, None).get("full"),
            Some("1")
        );
        assert_eq!(
            Event::task_run("inventory", 0, false, Some(false)).get("full"),
            Some("0")
        );
        // non-inventory task carries no `full`.
        assert_eq!(
            Event::task_run("netinventory", 0, true, None).get("full"),
            None
        );
    }

    #[test]
    fn from_params_dispatches_by_kind_flag() {
        assert_eq!(
            Event::from_params(&params(&[("runnow", "1"), ("task", "inventory")]))
                .unwrap()
                .kind,
            EventKind::RunNow
        );
        let taskrun = Event::from_params(&params(&[
            ("taskrun", "yes"),
            ("task", "inventory"),
            ("partial", "1"),
        ]))
        .unwrap();
        assert_eq!(taskrun.kind, EventKind::TaskRun);
        // partial=1 (and no full) -> full=0.
        assert_eq!(taskrun.get("full"), Some("0"));

        let partial =
            Event::from_params(&params(&[("partial", "1"), ("category", "software")])).unwrap();
        assert_eq!(partial.kind, EventKind::Partial);
        assert_eq!(partial.task, "inventory");
        assert_eq!(partial.category, "software");

        // job requires a name.
        assert!(Event::from_params(&params(&[("job", "1")])).is_none());
        assert_eq!(
            Event::from_params(&params(&[("job", "1"), ("name", "ToolBox")]))
                .unwrap()
                .kind,
            EventKind::Job
        );
        // Unrecognised request.
        assert!(Event::from_params(&params(&[("foo", "bar")])).is_none());
    }

    #[test]
    fn serde_round_trip_for_ipc() {
        let event = Event::partial("cpu,memory", params(&[("uuid", "abc")]));
        let json = serde_json::to_string(&event).unwrap();
        let back: Event = serde_json::from_str(&json).unwrap();
        assert_eq!(event, back);
        // Lower-cased kind tag on the wire.
        assert!(json.contains("\"kind\":\"partial\""));
    }

    #[test]
    fn query_string_lists_scalar_fields() {
        let event = Event::init("inventory", 1_700_000_000);
        let qs = event.to_query_string();
        assert!(qs.contains("name=init"));
        assert!(qs.contains("task=inventory"));
        assert!(qs.contains("rundate=1700000000"));
    }
}
