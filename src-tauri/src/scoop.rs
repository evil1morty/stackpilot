use std::env;
use std::path::PathBuf;

use parking_lot::RwLock;

/// Cached result of `resolve_scoop_root`. Cleared via `invalidate_cache`
/// when scoop is bootstrapped or a refresh is forced. Cache holds an
/// `Option<Option<PathBuf>>` because both states are meaningful:
/// - outer `None` → never resolved, do the work
/// - outer `Some(None)` → resolved and there's no scoop install
/// - outer `Some(Some(p))` → cached path
static ROOT_CACHE: RwLock<Option<Option<PathBuf>>> = RwLock::new(None);

/// Resolve the active Scoop install root.
///
/// Search order: $SCOOP env var → ~/scoop. Returns None if neither exists.
/// Cached for the process lifetime; call `invalidate_cache` after a
/// bootstrap or when the user might have moved their install.
pub fn scoop_root() -> Option<PathBuf> {
    if let Some(cached) = &*ROOT_CACHE.read() {
        return cached.clone();
    }
    let resolved = resolve();
    *ROOT_CACHE.write() = Some(resolved.clone());
    resolved
}

fn resolve() -> Option<PathBuf> {
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

/// Drop the cached root so the next `scoop_root()` call re-resolves. Call
/// this from any path that may have created the install (`scoop_bootstrap`)
/// or from an explicit user-triggered refresh.
pub fn invalidate_cache() {
    *ROOT_CACHE.write() = None;
}
