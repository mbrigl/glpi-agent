// SPDX-License-Identifier: GPL-2.0-only

//! Logged-in user inventory category (Linux `who`).
//!
//! Parses `who` output into the distinct logins currently on the system for
//! the GLPI `users` section. The parser is pure and unit-tested; the live
//! collector runs `who`.

use serde::Serialize;

/// A logged-in user.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct User {
    /// Login name.
    pub login: String,
}

/// Parses `who` output into the distinct logins, in first-seen order.
#[must_use]
pub fn parse_who(text: &str) -> Vec<User> {
    let mut seen = std::collections::HashSet::new();
    text.lines()
        .filter_map(|line| line.split_whitespace().next())
        .filter(|login| seen.insert(login.to_owned()))
        .map(|login| User {
            login: login.to_owned(),
        })
        .collect()
}

/// Collects the live logged-in users via `who` (Linux).
#[cfg(target_os = "linux")]
#[must_use]
pub fn collect() -> Vec<User> {
    match std::process::Command::new("who").output() {
        Ok(output) if output.status.success() => {
            parse_who(&String::from_utf8_lossy(&output.stdout))
        }
        _ => Vec::new(),
    }
}

/// Collects the live logged-in users (non-Linux stub).
#[cfg(not(target_os = "linux"))]
#[must_use]
pub fn collect() -> Vec<User> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::parse_who;

    #[test]
    fn parses_distinct_logins() {
        let who = "\
root     tty1         2024-01-15 10:00
user     pts/0        2024-01-15 10:05 (192.168.1.5)
user     pts/1        2024-01-15 10:06 (192.168.1.5)
";
        let users = parse_who(who);
        // "user" appears twice but is reported once, in order.
        assert_eq!(users.len(), 2);
        assert_eq!(users[0].login, "root");
        assert_eq!(users[1].login, "user");
    }

    #[test]
    fn empty_input_yields_no_users() {
        assert!(parse_who("").is_empty());
    }
}
