// SPDX-License-Identifier: GPL-2.0-only

//! The `findFile` collector: walk a directory and return entries matching a
//! filter (name glob, file/dir kind, size bounds, content checksum).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::checksum::{file_sha256_hex, file_sha512_hex};

/// A `findFile` match filter. Every set field must match for an entry to be
/// returned; unset fields are ignored.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct FindFilter {
    /// Glob on the file name (`*` and `?`), case-sensitive.
    pub name: Option<String>,
    /// Require the entry to be a regular file.
    pub is_file: Option<bool>,
    /// Require the entry to be a directory.
    pub is_dir: Option<bool>,
    /// Require an exact size in bytes.
    pub size_equals: Option<u64>,
    /// Require a size strictly greater than this.
    pub size_greater: Option<u64>,
    /// Require a size strictly lower than this.
    pub size_lower: Option<u64>,
    /// Require this lower-case hex SHA-256 of the contents.
    #[serde(rename = "checkSumSHA256")]
    pub checksum_sha256: Option<String>,
    /// Require this lower-case hex SHA-512 of the contents.
    #[serde(rename = "checkSumSHA512")]
    pub checksum_sha512: Option<String>,
}

/// A matched filesystem entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FoundFile {
    /// Absolute (or root-relative) path of the entry.
    pub path: PathBuf,
    /// Size in bytes (0 for directories).
    pub size: u64,
}

/// Walks `root` (recursively when `recursive`) and returns up to `limit`
/// entries matching `filter`. `limit == 0` means unlimited.
#[must_use]
pub fn find_files(
    root: &Path,
    recursive: bool,
    limit: usize,
    filter: &FindFilter,
) -> Vec<FoundFile> {
    let mut matches = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(meta) = entry.metadata() else {
                continue;
            };
            if meta.is_dir() && recursive {
                stack.push(path.clone());
            }
            if matches_filter(&path, &meta, filter) {
                let size = if meta.is_dir() { 0 } else { meta.len() };
                matches.push(FoundFile { path, size });
                if limit != 0 && matches.len() >= limit {
                    return matches;
                }
            }
        }
    }
    matches
}

/// Returns `true` if `path` (with metadata `meta`) satisfies every set field of
/// `filter`.
fn matches_filter(path: &Path, meta: &std::fs::Metadata, filter: &FindFilter) -> bool {
    if filter.is_file == Some(true) && !meta.is_file() {
        return false;
    }
    if filter.is_dir == Some(true) && !meta.is_dir() {
        return false;
    }
    if let Some(pattern) = &filter.name {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();
        if !glob_match(pattern, name) {
            return false;
        }
    }
    if meta.is_file() {
        let size = meta.len();
        if filter.size_equals.is_some_and(|s| size != s)
            || filter.size_greater.is_some_and(|s| size <= s)
            || filter.size_lower.is_some_and(|s| size >= s)
        {
            return false;
        }
        // Checksums are the most expensive test, so they run last.
        if let Some(want) = &filter.checksum_sha256 {
            if file_sha256_hex(path).ok().as_deref() != Some(want.as_str()) {
                return false;
            }
        }
        if let Some(want) = &filter.checksum_sha512 {
            if file_sha512_hex(path).ok().as_deref() != Some(want.as_str()) {
                return false;
            }
        }
    } else if filter.size_equals.is_some()
        || filter.size_greater.is_some()
        || filter.size_lower.is_some()
        || filter.checksum_sha256.is_some()
        || filter.checksum_sha512.is_some()
    {
        // Size/checksum filters cannot match a directory.
        return false;
    }
    true
}

/// A minimal glob matcher supporting `*` (any run) and `?` (any one char).
#[must_use]
pub fn glob_match(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let t: Vec<char> = text.chars().collect();
    // Classic two-pointer wildcard match with backtracking on `*`.
    let (mut pi, mut ti) = (0usize, 0usize);
    let (mut star, mut mark) = (None, 0usize);
    while ti < t.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == t[ti]) {
            pi += 1;
            ti += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = Some(pi);
            mark = ti;
            pi += 1;
        } else if let Some(s) = star {
            pi = s + 1;
            mark += 1;
            ti = mark;
        } else {
            return false;
        }
    }
    while pi < p.len() && p[pi] == '*' {
        pi += 1;
    }
    pi == p.len()
}

#[cfg(test)]
mod tests {
    use super::{find_files, glob_match, FindFilter};
    use std::fs;

    #[test]
    fn glob_matches_wildcards() {
        assert!(glob_match("*.log", "system.log"));
        assert!(glob_match("file?.txt", "file1.txt"));
        assert!(glob_match("*", "anything"));
        assert!(!glob_match("*.log", "system.txt"));
        assert!(!glob_match("file?.txt", "file12.txt"));
        assert!(glob_match("a*b*c", "axxbyyc"));
    }

    #[test]
    fn finds_files_by_name_and_recursion() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.log"), b"hello").unwrap();
        fs::write(dir.path().join("b.txt"), b"x").unwrap();
        let sub = dir.path().join("sub");
        fs::create_dir(&sub).unwrap();
        fs::write(sub.join("c.log"), b"world").unwrap();

        let filter = FindFilter {
            name: Some("*.log".to_owned()),
            is_file: Some(true),
            ..FindFilter::default()
        };

        let shallow = find_files(dir.path(), false, 0, &filter);
        assert_eq!(shallow.len(), 1);

        let deep = find_files(dir.path(), true, 0, &filter);
        assert_eq!(deep.len(), 2);
    }

    #[test]
    fn filters_by_size_and_checksum() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("hello"), b"hello").unwrap();

        // SHA-256 of "hello".
        let want = "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824";
        let filter = FindFilter {
            checksum_sha256: Some(want.to_owned()),
            size_greater: Some(4),
            ..FindFilter::default()
        };
        let found = find_files(dir.path(), false, 0, &filter);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].size, 5);

        // A wrong checksum rejects the file.
        let filter = FindFilter {
            checksum_sha256: Some("deadbeef".to_owned()),
            ..FindFilter::default()
        };
        assert!(find_files(dir.path(), false, 0, &filter).is_empty());
    }

    #[test]
    fn limit_caps_results() {
        let dir = tempfile::tempdir().unwrap();
        for i in 0..5 {
            fs::write(dir.path().join(format!("f{i}.dat")), b"x").unwrap();
        }
        let found = find_files(dir.path(), false, 2, &FindFilter::default());
        assert_eq!(found.len(), 2);
    }
}
