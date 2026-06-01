// SPDX-License-Identifier: GPL-2.0-only

//! Process inventory category (Linux `ps aux`).
//!
//! Parses `ps aux` columns into one [`Process`] per row: owner, pid, CPU/memory
//! percentages, virtual size, tty, start time and the full command. Percentages
//! are kept as their raw strings (so the result stays comparable/serializable).
//! The parser is pure and unit-tested; the live collector runs `ps`.

use serde::Serialize;

/// A running process.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Process {
    /// Owning user.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    /// Process id.
    pub pid: u32,
    /// CPU usage percentage (raw `%CPU`).
    #[serde(rename = "cpuusage", skip_serializing_if = "Option::is_none")]
    pub cpu_usage: Option<String>,
    /// Memory usage percentage (raw `%MEM`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mem: Option<String>,
    /// Virtual memory size in KB (`VSZ`).
    #[serde(rename = "virtualmemory", skip_serializing_if = "Option::is_none")]
    pub virtual_memory: Option<u64>,
    /// Controlling terminal.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tty: Option<String>,
    /// Full command line.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cmd: Option<String>,
}

/// `ps aux` columns: USER PID %CPU %MEM VSZ RSS TTY STAT START TIME COMMAND…
const COLUMNS: usize = 11;

/// Parses `ps aux` output into running processes (header row skipped).
#[must_use]
pub fn parse_ps(text: &str) -> Vec<Process> {
    text.lines()
        .filter_map(|line| {
            let tokens: Vec<&str> = line.split_whitespace().collect();
            if tokens.len() < COLUMNS || tokens[0] == "USER" {
                return None;
            }
            let pid = tokens[1].parse::<u32>().ok()?;
            Some(Process {
                user: non_empty(tokens[0]),
                pid,
                cpu_usage: non_empty(tokens[2]),
                mem: non_empty(tokens[3]),
                virtual_memory: tokens[4].parse().ok(),
                tty: non_empty(tokens[6]),
                // `started` is omitted: GLPI requires an absolute
                // `YYYY-MM-DD HH:MM:SS` timestamp, which the abbreviated `ps`
                // START column can't provide (a future refinement reads
                // /proc/<pid>/stat + btime).
                cmd: non_empty(&tokens[10..].join(" ")),
            })
        })
        .collect()
}

/// Collects the live process list via `ps aux` (Linux).
#[cfg(target_os = "linux")]
#[must_use]
pub fn collect() -> Vec<Process> {
    match std::process::Command::new("ps").arg("aux").output() {
        Ok(output) if output.status.success() => parse_ps(&String::from_utf8_lossy(&output.stdout)),
        _ => Vec::new(),
    }
}

/// Collects the live process list (non-Linux stub).
#[cfg(not(target_os = "linux"))]
#[must_use]
pub fn collect() -> Vec<Process> {
    Vec::new()
}

fn non_empty(value: &str) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

#[cfg(test)]
mod tests {
    use super::parse_ps;

    const PS: &str = "\
USER         PID %CPU %MEM    VSZ   RSS TTY      STAT START   TIME COMMAND
root           1  0.0  0.1 167744 11876 ?        Ss   10:00   0:01 /sbin/init
www-data     123  1.5  0.3 200000 30000 pts/0    S    10:05   0:10 nginx: worker process
";

    #[test]
    fn parses_processes_skipping_header() {
        let procs = parse_ps(PS);
        assert_eq!(procs.len(), 2);

        let init = &procs[0];
        assert_eq!(init.user.as_deref(), Some("root"));
        assert_eq!(init.pid, 1);
        assert_eq!(init.virtual_memory, Some(167744));
        assert_eq!(init.cmd.as_deref(), Some("/sbin/init"));

        let nginx = &procs[1];
        assert_eq!(nginx.pid, 123);
        assert_eq!(nginx.tty.as_deref(), Some("pts/0"));
        // Command with spaces is preserved whole.
        assert_eq!(nginx.cmd.as_deref(), Some("nginx: worker process"));
    }

    #[test]
    fn empty_input_yields_no_processes() {
        assert!(parse_ps("").is_empty());
        // Header only.
        assert!(parse_ps("USER PID %CPU %MEM VSZ RSS TTY STAT START TIME COMMAND").is_empty());
    }
}
