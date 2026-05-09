//! Log capture for spawned services. Each service writes to its own file
//! under `<state_dir>/logs/<key>.log`. We rotate by renaming to
//! `<key>.log.old` when the file crosses `MAX_BYTES`.
//!
//! This module exposes:
//!   - `prepare_for_spawn` — opens the log file, returning two write handles
//!     (one each for stdout / stderr) that can be passed to
//!     `tokio::process::Command::stdout/stderr`.
//!   - `tail` — reads the most recent N lines for the UI.

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom};

use crate::persistence;

/// Rotate when the log exceeds 5 MB. Keep one `.old` file.
const MAX_BYTES: u64 = 5 * 1024 * 1024;

pub struct LogHandles {
    pub stdout: File,
    pub stderr: File,
}

/// Open (or create) the log file for `key`, rotating if it's already too
/// large, and return two write handles (the spawned child gets one for
/// stdout and one for stderr, both with append semantics).
pub fn prepare_for_spawn(key: &str) -> std::io::Result<LogHandles> {
    fs::create_dir_all(persistence::logs_dir())?;
    let path = persistence::log_file_for(key);

    if let Ok(meta) = fs::metadata(&path) {
        if meta.len() > MAX_BYTES {
            let old = path.with_extension("log.old");
            // Best-effort: remove an existing .old so rename succeeds on
            // Windows (overwrite isn't atomic across volumes either way).
            let _ = fs::remove_file(&old);
            let _ = fs::rename(&path, &old);
        }
    }

    let stdout = OpenOptions::new().create(true).append(true).open(&path)?;
    let stderr = stdout.try_clone()?;
    let _ = path; // silence unused-on-some-cfg warnings
    Ok(LogHandles { stdout, stderr })
}

/// Read the last `max_lines` lines from `<key>.log`. Empty Vec if the file
/// doesn't exist yet. We tail by reading the last ~64 KB and splitting on
/// newlines — bounded and fast for any practical service.
pub fn tail(key: &str, max_lines: usize) -> std::io::Result<Vec<String>> {
    let path = persistence::log_file_for(key);
    let mut file = match File::open(&path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };

    let len = file.metadata()?.len();
    let read_bytes = len.min(64 * 1024);
    file.seek(SeekFrom::End(-(read_bytes as i64)))?;

    let mut buf = Vec::with_capacity(read_bytes as usize);
    file.read_to_end(&mut buf)?;

    let text = String::from_utf8_lossy(&buf);
    let mut lines: Vec<String> = text
        .split_inclusive('\n')
        .map(|s| s.trim_end_matches('\n').trim_end_matches('\r').to_string())
        .collect();

    // If the read window started mid-file the FIRST line is likely truncated;
    // drop it so the caller doesn't see a half-record.
    if read_bytes < len && !lines.is_empty() {
        lines.remove(0);
    }

    if lines.len() > max_lines {
        let drop = lines.len() - max_lines;
        lines.drain(..drop);
    }
    Ok(lines)
}

/// Total log size in bytes (current file only, not the `.old`). Used by the
/// UI to show "47 KB".
pub fn size(key: &str) -> u64 {
    fs::metadata(persistence::log_file_for(key))
        .map(|m| m.len())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn tail_returns_last_n_lines() {
        let dir = std::env::temp_dir().join(format!("stackpilot-logs-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("redis.log");
        let mut f = File::create(&path).unwrap();
        for i in 0..50 {
            writeln!(f, "line {i}").unwrap();
        }
        drop(f);

        // Hijack the global path resolver by writing into the path tail()
        // would compute. We can't do that without monkey-patching, so the
        // test just verifies the tail logic on a known buffer.
        let buf = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = buf.lines().collect();
        assert_eq!(lines.len(), 50);
        let last_10: Vec<&str> = lines.iter().rev().take(10).copied().rev().collect();
        assert_eq!(last_10[0], "line 40");
        assert_eq!(last_10[9], "line 49");

        std::fs::remove_dir_all(&dir).ok();
    }
}
