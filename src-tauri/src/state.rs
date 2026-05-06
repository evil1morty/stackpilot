use parking_lot::Mutex;

use crate::catalog::CatalogCache;

#[derive(Default)]
pub struct AppState {
    pub catalog: CatalogCache,
    /// PID of the currently-running scoop subprocess, if any. Used by
    /// `scoop_cancel` to send `taskkill /T /F /PID`. Phase 2 enforces
    /// one operation at a time.
    pub running_pid: Mutex<Option<u32>>,
}
