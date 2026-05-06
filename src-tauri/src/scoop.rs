use std::env;
use std::path::PathBuf;

/// Resolve the active Scoop install root.
///
/// Search order: $SCOOP env var → ~/scoop. Returns None if neither exists.
pub fn scoop_root() -> Option<PathBuf> {
    if let Ok(custom) = env::var("SCOOP") {
        let p = PathBuf::from(custom);
        if p.is_dir() {
            return Some(p);
        }
    }

    let home = env::var("USERPROFILE")
        .ok()
        .or_else(|| env::var("HOME").ok())?;
    let default = PathBuf::from(home).join("scoop");
    if default.is_dir() {
        Some(default)
    } else {
        None
    }
}
