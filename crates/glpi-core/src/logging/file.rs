// SPDX-License-Identifier: GPL-2.0-only

//! The file logging backend.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

use super::{Backend, LogLevel};

/// A [`Backend`] that appends each message to a file as `[<level>] <message>`.
///
/// The file is opened (creating it if needed) on every write. Relying on the
/// operating system's append semantics keeps the backend `Send + Sync` without
/// a lock; any I/O error is reported once to standard error and otherwise
/// swallowed, so logging never takes the caller down.
#[derive(Debug, Clone)]
pub struct FileBackend {
    path: PathBuf,
}

impl FileBackend {
    /// Creates a backend that appends to `path`.
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
        }
    }
}

impl Backend for FileBackend {
    fn emit(&self, level: LogLevel, message: &str) {
        match OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
        {
            Ok(mut file) => {
                if let Err(e) = writeln!(file, "[{level}] {message}") {
                    eprintln!(
                        "[error] failed to write log file {}: {e}",
                        self.path.display()
                    );
                }
            }
            Err(e) => {
                eprintln!(
                    "[error] failed to open log file {}: {e}",
                    self.path.display()
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::FileBackend;
    use crate::logging::{LogLevel, Logger};
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_path() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "glpi-core-filelog-{}-{nanos}.log",
            std::process::id()
        ))
    }

    #[test]
    fn appends_filtered_lines() {
        let path = unique_temp_path();
        let logger = Logger::new()
            .with_max_level(LogLevel::Info)
            .with_backend(FileBackend::new(&path));

        logger.error("first");
        logger.info("second");
        logger.debug("dropped");

        let contents = fs::read_to_string(&path).unwrap();
        let _ = fs::remove_file(&path);

        assert_eq!(contents, "[error] first\n[info] second\n");
    }
}
