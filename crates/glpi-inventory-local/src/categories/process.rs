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

/// Collects the live process list via `ps aux` (macOS; same columns as Linux).
#[cfg(target_os = "macos")]
#[must_use]
pub fn collect() -> Vec<Process> {
    crate::sys::output("ps", &["aux"])
        .map(|text| parse_ps(&text))
        .unwrap_or_default()
}

/// Collects the live process list (Windows) from `Win32_Process`.
#[cfg(target_os = "windows")]
#[must_use]
pub fn collect() -> Vec<Process> {
    crate::sys::powershell(
        "Get-CimInstance Win32_Process | \
         Select-Object ProcessId,Name,CommandLine,VirtualSize | ConvertTo-Json -Compress",
    )
    .map(|json| parse_win_processes(&json))
    .unwrap_or_default()
}

/// Collects the live process list (unsupported-platform stub).
#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
#[must_use]
pub fn collect() -> Vec<Process> {
    Vec::new()
}

/// Parses a `Win32_Process` `ConvertTo-Json` result into the process list.
#[must_use]
pub fn parse_win_processes(json: &str) -> Vec<Process> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(json) else {
        return Vec::new();
    };
    crate::jsonutil::array(value)
        .iter()
        .filter_map(|item| {
            let pid = u32::try_from(crate::jsonutil::u64_field(item, "ProcessId")?).ok()?;
            Some(Process {
                user: None,
                pid,
                cpu_usage: None,
                mem: None,
                // VirtualSize is in bytes; the GLPI field is KB.
                virtual_memory: crate::jsonutil::u64_field(item, "VirtualSize").map(|b| b / 1024),
                tty: None,
                cmd: crate::jsonutil::str_field(item, "CommandLine")
                    .or_else(|| crate::jsonutil::str_field(item, "Name")),
            })
        })
        .collect()
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

    #[test]
    fn parses_windows_process_json() {
        use super::parse_win_processes;
        let json = r#"[{"ProcessId":4,"Name":"System","VirtualSize":"2203320320"},
            {"ProcessId":1234,"Name":"notepad.exe","CommandLine":"\"C:\\Windows\\notepad.exe\""}]"#;
        let procs = parse_win_processes(json);
        assert_eq!(procs.len(), 2);
        assert_eq!(procs[0].pid, 4);
        assert_eq!(procs[0].virtual_memory, Some(2_203_320_320 / 1024));
        assert_eq!(procs[0].cmd.as_deref(), Some("System")); // falls back to Name
        assert_eq!(procs[1].pid, 1234);
        assert!(procs[1].cmd.as_deref().unwrap().contains("notepad.exe"));
    }
}
