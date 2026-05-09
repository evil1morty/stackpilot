use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, AtomicU64};

use parking_lot::Mutex;

use crate::catalog::CatalogCache;
use crate::persistence::{self, PersistedService, PersistedState};

/// What we track for an in-flight service. We deliberately do not keep the
/// `tokio::process::Child` handle so that 1) the same shape covers entries
/// re-attached from disk on next launch, and 2) stop is uniformly via
/// `taskkill /T /F /PID`. Liveness is probed via port-binding rather than
/// `Child::try_wait()`.
#[derive(Clone, Debug)]
pub struct TrackedService {
    pub pid: u32,
    pub port: Option<u16>,
    pub started_at: u64,
}

#[derive(Default)]
pub struct AppState {
    pub catalog: CatalogCache,
    /// PID of the currently-running scoop subprocess, if any. Used by
    /// `scoop_cancel` to issue `taskkill /T /F /PID`. Phase 2 enforces
    /// one Scoop operation at a time.
    pub running_pid: Mutex<Option<u32>>,
    /// Tracked services Stackpilot has spawned. Persisted to disk so a
    /// crash + relaunch re-attaches to still-running children.
    pub tracked: Mutex<HashMap<String, TrackedService>>,
    /// Service keys whose start sequence is mid-flight (init step, spawn,
    /// post-spawn verify). Held under the same critical section as
    /// `tracked` so a second start request can't race past the duplicate
    /// check before the first call has registered itself. Also blocks
    /// `services_stop` from racing with a half-started service.
    pub starting: Mutex<HashSet<&'static str>>,
    /// Monotonic counter bumped by `scoop_cancel`. Multi-step orchestrators
    /// (e.g. `presets_apply`) snapshot it before starting and check before
    /// every step to detect that the user pressed Cancel.
    pub cancellation_gen: AtomicU64,
    /// When true, closing the main window hides it to the tray instead of
    /// exiting. Synced from the frontend Settings menu via IPC.
    pub close_to_tray: AtomicBool,
}

impl AppState {
    /// Build initial state, loading any previously-tracked services from
    /// disk. Stale entries (process dead or port unowned) are dropped.
    pub fn load_from_disk() -> Self {
        let persisted = persistence::load();
        let mut tracked: HashMap<String, TrackedService> = HashMap::new();

        for svc in persisted.services {
            if !is_still_alive(&svc) {
                continue;
            }
            tracked.insert(
                svc.key,
                TrackedService {
                    pid: svc.pid,
                    port: svc.port,
                    started_at: svc.started_at,
                },
            );
        }

        // Persist the cleaned-up state immediately so the next read is
        // consistent with our in-memory map.
        persistence::save(&snapshot(&tracked));

        Self {
            catalog: CatalogCache::default(),
            running_pid: Mutex::new(None),
            tracked: Mutex::new(tracked),
            starting: Mutex::new(HashSet::new()),
            cancellation_gen: AtomicU64::new(0),
            close_to_tray: AtomicBool::new(true),
        }
    }

    /// Persist the current tracked map to disk. Best-effort — failures
    /// are logged but don't block the caller.
    pub fn persist_tracked(&self) {
        let snap = snapshot(&self.tracked.lock());
        persistence::save(&snap);
    }
}

/// A tracked entry is "still alive" iff the recorded PID is currently
/// listening on the recorded port. This is a strong signal that the
/// pre-crash service is still the one we're seeing — bare PID-equality
/// would risk re-attaching to an unrelated process that happened to recycle
/// the PID.
fn is_still_alive(svc: &PersistedService) -> bool {
    let Some(port) = svc.port else {
        // No port to verify against — treat as dead so we don't latch onto
        // a recycled PID.
        return false;
    };
    listeners::get_process_by_port(port, listeners::Protocol::TCP)
        .ok()
        .map(|p| p.pid == svc.pid)
        .unwrap_or(false)
}

fn snapshot(tracked: &HashMap<String, TrackedService>) -> PersistedState {
    PersistedState {
        version: 1,
        services: tracked
            .iter()
            .map(|(key, svc)| PersistedService {
                key: key.clone(),
                pid: svc.pid,
                port: svc.port,
                started_at: svc.started_at,
            })
            .collect(),
    }
}
