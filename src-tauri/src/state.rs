use std::collections::HashMap;
use std::sync::atomic::AtomicU64;

use parking_lot::Mutex;

use crate::catalog::CatalogCache;
use crate::commands::services::TrackedChild;

#[derive(Default)]
pub struct AppState {
    pub catalog: CatalogCache,
    /// PID of the currently-running scoop subprocess, if any. Used by
    /// `scoop_cancel` to issue `taskkill /T /F /PID`. Phase 2 enforces
    /// one Scoop operation at a time.
    pub running_pid: Mutex<Option<u32>>,
    /// Tracked services Stackpilot has spawned. Keyed by `KnownService::key`.
    /// Children are leaked on drop (taskkill is responsible for cleanup).
    pub tracked: Mutex<HashMap<String, TrackedChild>>,
    /// Monotonic counter bumped by `scoop_cancel`. Multi-step orchestrators
    /// (e.g. `presets_apply`) snapshot it before starting and check before
    /// every step to detect that the user pressed Cancel.
    pub cancellation_gen: AtomicU64,
}
