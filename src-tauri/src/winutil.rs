//! Small Windows-specific helpers shared across spawn sites.

use std::path::PathBuf;

#[cfg(windows)]
pub const CREATE_NO_WINDOW: u32 = 0x08000000;

/// Apply CREATE_NO_WINDOW to a tokio::process::Command so the spawned child
/// doesn't flash a console window. No-op on non-Windows.
#[allow(unused_variables)]
pub fn hide_console_tokio(cmd: &mut tokio::process::Command) {
    #[cfg(windows)]
    {
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
}

/// Same as `hide_console_tokio` but for std::process::Command.
#[allow(unused_variables)]
pub fn hide_console_std(cmd: &mut std::process::Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
}

/// Probe PATH for an executable named `name`, trying each provided extension
/// (and the bare name first). Returns the first match. Honors PATHEXT on
/// Windows when `exts` is empty.
pub fn which(name: &str, exts: &[&str]) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;

    let pathext_default = std::env::var("PATHEXT").unwrap_or_default();
    let pathext_lower: Vec<String> = pathext_default
        .split(';')
        .filter_map(|e| {
            let trimmed = e.trim().trim_start_matches('.');
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_ascii_lowercase())
            }
        })
        .collect();

    let candidates: Vec<&str> = if exts.is_empty() {
        pathext_lower.iter().map(String::as_str).collect()
    } else {
        exts.to_vec()
    };

    for dir in std::env::split_paths(&path) {
        // Bare name first (handles PATHEXT-less files like `openssl` on Unix).
        let bare = dir.join(name);
        if bare.is_file() {
            return Some(bare);
        }
        for ext in &candidates {
            let candidate = dir.join(format!("{name}.{ext}"));
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::which;

    #[test]
    fn which_finds_cmd_on_windows() {
        // Sanity check — `cmd` should always be on Windows PATH.
        if cfg!(windows) {
            assert!(which("cmd", &["exe"]).is_some());
        }
    }

    #[test]
    fn which_returns_none_for_garbage() {
        assert!(which("definitely-not-a-real-binary-xyzzy", &["exe"]).is_none());
    }
}
