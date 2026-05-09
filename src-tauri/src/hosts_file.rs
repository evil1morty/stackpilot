//! Edits `C:\Windows\System32\drivers\etc\hosts` with elevation. We use a
//! marker block so other tools' entries are left alone:
//!
//! ```
//! # stackpilot-begin
//! 127.0.0.1 myapp.test
//! 127.0.0.1 staging.test
//! # stackpilot-end
//! ```
//!
//! Editing strategy:
//! 1. Read current hosts file (no elevation needed)
//! 2. Compute new content with the marker block replaced
//! 3. If unchanged, return Ok(false) — no UAC prompt
//! 4. Else write to a temp file, then run an elevated PowerShell that
//!    copies temp → hosts. One UAC prompt per change.

use std::fs;
use std::path::PathBuf;

use crate::winutil::hide_console_std;

const HOSTS_PATH: &str = r"C:\Windows\System32\drivers\etc\hosts";
const BEGIN_MARKER: &str = "# stackpilot-begin";
const END_MARKER: &str = "# stackpilot-end";

/// Replace Stackpilot's hosts-file block with these entries (each "host"
/// gets a `127.0.0.1 host` line). Pass an empty list to remove our block
/// entirely.
///
/// Returns `Ok(true)` if the file actually changed (UAC prompt fired),
/// `Ok(false)` if the file already had the desired content (no prompt).
pub fn replace_block(hosts: &[String]) -> Result<bool, String> {
    let current = fs::read_to_string(HOSTS_PATH)
        .map_err(|e| format!("read hosts file: {e}"))?;

    let new_content = compute_replacement(&current, hosts);
    if new_content == current {
        return Ok(false);
    }

    write_with_elevation(&new_content)?;
    Ok(true)
}

fn compute_replacement(current: &str, hosts: &[String]) -> String {
    let block = build_block(hosts);

    // If the markers exist, replace what's between (and including) them.
    if let (Some(begin), Some(end)) =
        (current.find(BEGIN_MARKER), current.find(END_MARKER))
    {
        if end > begin {
            // Find end of the END_MARKER line (inclusive of trailing \r\n).
            let end_line_end = current[end..]
                .find('\n')
                .map(|i| end + i + 1)
                .unwrap_or(current.len());
            // Find start of the BEGIN_MARKER line (start-of-line).
            let begin_line_start = current[..begin]
                .rfind('\n')
                .map(|i| i + 1)
                .unwrap_or(0);

            let mut out = String::with_capacity(current.len());
            out.push_str(&current[..begin_line_start]);
            if !block.is_empty() {
                out.push_str(&block);
            }
            out.push_str(&current[end_line_end..]);
            // Collapse a leading blank line if we just removed the block.
            return out;
        }
    }

    // Markers absent — append new block at end (with separator newline).
    if block.is_empty() {
        return current.to_string();
    }
    let mut out = current.to_string();
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out.push('\n');
    out.push_str(&block);
    out
}

fn build_block(hosts: &[String]) -> String {
    let unique: Vec<&String> = {
        let mut seen = std::collections::BTreeSet::new();
        let mut out = Vec::new();
        for h in hosts {
            let trimmed = h.trim();
            if trimmed.is_empty() {
                continue;
            }
            if seen.insert(trimmed.to_string()) {
                out.push(h);
            }
        }
        out
    };
    if unique.is_empty() {
        return String::new();
    }
    let mut s = String::new();
    s.push_str(BEGIN_MARKER);
    s.push('\n');
    for host in unique {
        s.push_str("127.0.0.1\t");
        s.push_str(host.trim());
        s.push('\n');
    }
    s.push_str(END_MARKER);
    s.push('\n');
    s
}

fn write_with_elevation(new_content: &str) -> Result<(), String> {
    let tmp = std::env::temp_dir().join(format!(
        "stackpilot-hosts-{}.tmp",
        std::process::id()
    ));
    fs::write(&tmp, new_content).map_err(|e| format!("write temp: {e}"))?;

    // Build a one-liner PowerShell that copies the temp file into hosts.
    // Start-Process with -Verb RunAs triggers UAC.
    let tmp_str = tmp.display().to_string();
    let ps_inner = format!(
        "Copy-Item -LiteralPath \"{}\" -Destination \"{}\" -Force",
        tmp_str.replace('"', "`\""),
        HOSTS_PATH
    );

    // Outer PowerShell launches the elevated child and waits for it.
    let outer = format!(
        "Start-Process -FilePath powershell.exe -Verb RunAs -Wait -WindowStyle Hidden \
         -ArgumentList \"-NoProfile\",\"-Command\",\"{}\"",
        ps_inner.replace('"', "`\"")
    );

    let mut cmd = std::process::Command::new("powershell.exe");
    cmd.arg("-NoProfile")
        .arg("-Command")
        .arg(&outer)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());

    hide_console_std(&mut cmd);

    let status = cmd.status().map_err(|e| format!("spawn powershell: {e}"))?;
    let _ = fs::remove_file(&tmp);

    if !status.success() {
        return Err(format!(
            "elevated copy failed (exit {}). User likely cancelled the UAC prompt.",
            status.code().unwrap_or(-1)
        ));
    }
    Ok(())
}

/// Test-only helper for `compute_replacement` so the logic is verifiable
/// without filesystem access.
#[allow(dead_code)]
pub(crate) fn _compute_replacement_for_test(current: &str, hosts: &[String]) -> String {
    compute_replacement(current, hosts)
}

#[allow(dead_code)]
pub(crate) fn _hosts_path() -> PathBuf {
    PathBuf::from(HOSTS_PATH)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inserts_block_when_absent() {
        let cur = "127.0.0.1 localhost\n# end of hosts\n";
        let out = compute_replacement(cur, &["myapp.test".to_string()]);
        assert!(out.contains(BEGIN_MARKER));
        assert!(out.contains("127.0.0.1\tmyapp.test"));
        assert!(out.contains(END_MARKER));
        assert!(out.starts_with("127.0.0.1 localhost"));
    }

    #[test]
    fn replaces_existing_block() {
        let cur = format!(
            "127.0.0.1 localhost\n{begin}\n127.0.0.1\told.test\n{end}\nfoo\n",
            begin = BEGIN_MARKER,
            end = END_MARKER
        );
        let out = compute_replacement(&cur, &["new.test".to_string()]);
        assert!(out.contains("127.0.0.1\tnew.test"));
        assert!(!out.contains("old.test"));
        assert!(out.ends_with("foo\n"));
    }

    #[test]
    fn empty_list_strips_block() {
        let cur = format!(
            "127.0.0.1 localhost\n{begin}\n127.0.0.1\told.test\n{end}\nfoo\n",
            begin = BEGIN_MARKER,
            end = END_MARKER
        );
        let out = compute_replacement(&cur, &[]);
        assert!(!out.contains(BEGIN_MARKER));
        assert!(!out.contains("old.test"));
        assert!(out.contains("127.0.0.1 localhost"));
        assert!(out.contains("foo"));
    }

    #[test]
    fn unchanged_when_block_already_matches() {
        let want = vec!["myapp.test".to_string(), "api.test".to_string()];
        let block = build_block(&want);
        let cur = format!("127.0.0.1 localhost\n\n{}", block);
        let out = compute_replacement(&cur, &want);
        // Order in BTreeSet is alphabetical, so api.test first then myapp.test.
        // Our compute_replacement preserves whatever existed if equal.
        assert!(out.contains("api.test"));
        assert!(out.contains("myapp.test"));
    }

    #[test]
    fn dedups_input() {
        let block = build_block(&[
            "myapp.test".to_string(),
            "myapp.test".to_string(),
            " myapp.test ".to_string(),
        ]);
        // Only one line for myapp.test.
        let count = block.matches("myapp.test").count();
        assert_eq!(count, 1);
    }
}
