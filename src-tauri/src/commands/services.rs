//! Service control: start / stop / restart / open data folder for the curated
//! `KNOWN` services. Spawned services are tracked in `AppState.tracked`
//! (PID + port + start time) and mirrored to disk so a relaunch after a
//! crash can re-attach. We deliberately do not retain the
//! `tokio::process::Child` handle — liveness is probed by checking that the
//! recorded PID still owns the recorded port.

use std::collections::HashMap;
use std::process::Stdio;

use serde::Serialize;
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;
use tokio::process::Command;

use crate::known_services::{self, KnownService};
use crate::scoop::scoop_root;
use crate::state::{AppState, TrackedService};

// ─────────────────────────────────────────────── public DTOs ──────────────

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum ServiceStatus {
    Stopped,
    RunningTracked {
        pid: u32,
        started_at: u64,
    },
    /// Listening on the default port but not spawned by Stackpilot — likely
    /// the user started it from a terminal or a previous app session.
    RunningExternal {
        pid: u32,
    },
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ServiceInfo {
    pub key: String,
    pub scoop_app: String,
    pub display: String,
    pub category: String,
    pub installed: bool,
    pub status: ServiceStatus,
    pub default_port: Option<u16>,
    pub persist_dir: Option<String>,
    pub bin_path: Option<String>,
}

// ─────────────────────────────────────────────── status helpers ───────────

/// Try to discover which PID is listening on `port`, if any. Wraps the
/// `listeners` crate which uses `GetExtendedTcpTable` on Windows. The crate
/// returns `Err` when nothing is listening, so we collapse that to `None`.
fn pid_on_port(port: u16) -> Option<u32> {
    listeners::get_process_by_port(port, listeners::Protocol::TCP)
        .ok()
        .map(|p| p.pid)
}

fn current_status(svc: &KnownService, tracked: &HashMap<String, TrackedService>) -> ServiceStatus {
    if let Some(t) = tracked.get(svc.key) {
        return ServiceStatus::RunningTracked {
            pid: t.pid,
            started_at: t.started_at,
        };
    }
    if let Some(port) = svc.default_port {
        if let Some(pid) = pid_on_port(port) {
            return ServiceStatus::RunningExternal { pid };
        }
    }
    ServiceStatus::Stopped
}

/// Drop tracked entries whose recorded PID no longer owns the recorded port.
/// Returns true if anything was removed (caller should persist).
fn sweep_dead(tracked: &mut HashMap<String, TrackedService>) -> bool {
    let mut dead: Vec<String> = Vec::new();
    for (k, t) in tracked.iter() {
        let alive = match t.port {
            Some(port) => pid_on_port(port) == Some(t.pid),
            None => false,
        };
        if !alive {
            dead.push(k.clone());
        }
    }
    let changed = !dead.is_empty();
    for k in dead {
        tracked.remove(&k);
    }
    changed
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

// ─────────────────────────────────────────────── commands ─────────────────

#[tauri::command]
pub fn services_list(state: State<'_, AppState>) -> Vec<ServiceInfo> {
    let root = scoop_root();
    let mut tracked = state.tracked.lock();
    let changed = sweep_dead(&mut tracked);
    drop(tracked);
    if changed {
        state.persist_tracked();
    }
    let tracked = state.tracked.lock();

    known_services::KNOWN
        .iter()
        .map(|svc| {
            let bin = root.as_ref().map(|r| known_services::bin_path(svc, r));
            let installed = bin.as_ref().is_some_and(|p| p.exists());

            ServiceInfo {
                key: svc.key.to_string(),
                scoop_app: svc.scoop_app.to_string(),
                display: svc.display.to_string(),
                category: svc.category.as_str().to_string(),
                installed,
                status: current_status(svc, &tracked),
                default_port: svc.default_port,
                persist_dir: root
                    .as_ref()
                    .and_then(|r| known_services::persist_dir(svc, r))
                    .map(|p| p.display().to_string()),
                bin_path: bin.map(|p| p.display().to_string()),
            }
        })
        .collect()
}

/// Implementation of `services_start` that takes a borrowed `AppState`.
/// Used by `services_start` (Tauri command) and by orchestrators like
/// `presets_apply`.
pub(crate) async fn services_start_inner(
    key: &str,
    state: &AppState,
) -> Result<ServiceInfo, String> {
    let svc = known_services::lookup(key).ok_or_else(|| format!("unknown service: {key}"))?;
    let root = scoop_root().ok_or_else(|| "Scoop is not installed".to_string())?;
    let bin = known_services::bin_path(svc, &root);
    if !bin.exists() {
        return Err(format!(
            "{} is not installed. Install it from the Catalog first.",
            svc.display
        ));
    }

    {
        let tracked = state.tracked.lock();
        if tracked.contains_key(svc.key) {
            return Err(format!("{} is already running", svc.display));
        }
    }

    if let Some(port) = svc.default_port {
        if let Some(pid) = pid_on_port(port) {
            return Err(format!(
                "Port {} is already bound by PID {}. Stop the other process first.",
                port, pid
            ));
        }
    }

    let cwd = known_services::working_dir(svc, &root);
    let args = known_services::launch_args(svc, &root);

    let mut cmd = Command::new(&bin);
    cmd.args(&args)
        .current_dir(&cwd)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .stdin(Stdio::null());

    #[cfg(windows)]
    {
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let child = cmd
        .spawn()
        .map_err(|e| format!("failed to spawn {}: {}", svc.display, e))?;
    let pid = child
        .id()
        .ok_or_else(|| "spawned child has no PID".to_string())?;

    // We don't keep the Child handle: it's not needed for stop (taskkill /T
    // /F by PID does the right thing) and would prevent us from re-attaching
    // to the same service after a Stackpilot relaunch. Drop forgets the
    // handle without killing the process.
    drop(child);

    state.tracked.lock().insert(
        svc.key.to_string(),
        TrackedService {
            pid,
            port: svc.default_port,
            started_at: unix_now(),
        },
    );
    state.persist_tracked();

    Ok(build_info_for(svc, state))
}

#[tauri::command]
pub async fn services_start(
    key: String,
    state: State<'_, AppState>,
) -> Result<ServiceInfo, String> {
    services_start_inner(&key, &state).await
}

#[tauri::command]
pub async fn services_stop(
    key: String,
    state: State<'_, AppState>,
) -> Result<ServiceInfo, String> {
    let svc = known_services::lookup(&key).ok_or_else(|| format!("unknown service: {key}"))?;

    let pid_to_kill: Option<u32> = {
        let mut tracked = state.tracked.lock();
        sweep_dead(&mut tracked);
        if let Some(t) = tracked.remove(svc.key) {
            Some(t.pid)
        } else if let Some(port) = svc.default_port {
            // Stop external process listening on our port — covers cases
            // where the user started it outside Stackpilot.
            pid_on_port(port)
        } else {
            None
        }
    };
    state.persist_tracked();

    let pid = match pid_to_kill {
        Some(p) => p,
        None => return Err(format!("{} is not running", svc.display)),
    };

    let mut kill = Command::new("taskkill");
    kill.args(["/T", "/F", "/PID", &pid.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    #[cfg(windows)]
    {
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        kill.creation_flags(CREATE_NO_WINDOW);
    }

    kill.status()
        .await
        .map_err(|e| format!("taskkill failed: {e}"))?;

    Ok(build_info(svc, &state))
}

#[tauri::command]
pub async fn services_restart(
    key: String,
    state: State<'_, AppState>,
) -> Result<ServiceInfo, String> {
    let _ = services_stop(key.clone(), state.clone()).await;
    // Brief pause so OS releases the port before we rebind.
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    services_start(key, state).await
}

#[tauri::command]
pub fn services_open_data(
    key: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let _ = state;
    let svc = known_services::lookup(&key).ok_or_else(|| format!("unknown service: {key}"))?;
    let root = scoop_root().ok_or_else(|| "Scoop is not installed".to_string())?;
    let dir = known_services::persist_dir(svc, &root)
        .ok_or_else(|| format!("{} has no persist directory", svc.display))?;

    if !dir.exists() {
        return Err(format!(
            "Persist directory {} does not exist yet. Start the service once to populate it.",
            dir.display()
        ));
    }

    app.opener()
        .open_path(dir.display().to_string(), None::<&str>)
        .map_err(|e| e.to_string())
}

fn build_info(svc: &KnownService, state: &State<'_, AppState>) -> ServiceInfo {
    build_info_for(svc, state)
}

fn build_info_for(svc: &KnownService, state: &AppState) -> ServiceInfo {
    let root = scoop_root();
    let bin = root.as_ref().map(|r| known_services::bin_path(svc, r));
    let installed = bin.as_ref().is_some_and(|p| p.exists());
    let tracked = state.tracked.lock();

    ServiceInfo {
        key: svc.key.to_string(),
        scoop_app: svc.scoop_app.to_string(),
        display: svc.display.to_string(),
        category: svc.category.as_str().to_string(),
        installed,
        status: current_status(svc, &tracked),
        default_port: svc.default_port,
        persist_dir: root
            .as_ref()
            .and_then(|r| known_services::persist_dir(svc, r))
            .map(|p| p.display().to_string()),
        bin_path: bin.map(|p| p.display().to_string()),
    }
}
