//! Service control: start / stop / restart / open data folder for the curated
//! `KNOWN` services. Spawned services are tracked in `AppState.tracked`
//! (PID + port + start time) and mirrored to disk so a relaunch after a
//! crash can re-attach. We deliberately do not retain the
//! `tokio::process::Child` handle — liveness is probed by checking that the
//! recorded PID still owns the recorded port.

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;

use serde::Serialize;
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;
use tokio::process::Command;

use crate::health::{self, ServiceHealth};
use crate::known_services::{self, KnownService};
use crate::scoop::scoop_root;
use crate::service_logs;
use crate::state::{AppState, TrackedService};

/// How long after start we treat "port not yet bound" as "Starting" rather
/// than failed/dead. Most services bind in <500ms; 5s is a generous cushion.
const STARTUP_GRACE_SECS: u64 = 5;

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
    pub health: ServiceHealth,
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

/// Drop tracked entries whose recorded PID no longer owns the recorded port,
/// with a brief startup grace window so a service mid-boot isn't culled.
/// Returns true if anything was removed (caller should persist).
fn sweep_dead(tracked: &mut HashMap<String, TrackedService>) -> bool {
    let now = unix_now();
    let mut dead: Vec<String> = Vec::new();
    for (k, t) in tracked.iter() {
        // Grace window: don't cull a freshly-started service that hasn't
        // bound its port yet.
        if now.saturating_sub(t.started_at) < STARTUP_GRACE_SECS {
            continue;
        }
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
pub async fn services_list(state: State<'_, AppState>) -> Result<Vec<ServiceInfo>, String> {
    let root = scoop_root();

    // Sweep + persist before snapshotting so the snapshot reflects truth.
    {
        let mut tracked = state.tracked.lock();
        let changed = sweep_dead(&mut tracked);
        drop(tracked);
        if changed {
            state.persist_tracked();
        }
    }
    let tracked_snapshot: HashMap<String, TrackedService> = state.tracked.lock().clone();
    let now = unix_now();

    // Materialize per-service inputs synchronously so each async probe owns
    // everything it needs (no shared borrow across .await points).
    struct Row {
        svc: &'static KnownService,
        bin: Option<PathBuf>,
        installed: bool,
        persist_dir: Option<String>,
        tracked_entry: Option<TrackedService>,
        external_pid: Option<u32>,
    }
    let rows: Vec<Row> = known_services::KNOWN
        .iter()
        .map(|svc| {
            let bin = root.as_ref().map(|r| known_services::bin_path(svc, r));
            let installed = bin.as_ref().is_some_and(|p| p.exists());
            let persist_dir = root
                .as_ref()
                .and_then(|r| known_services::persist_dir(svc, r))
                .map(|p| p.display().to_string());
            let tracked_entry = tracked_snapshot.get(svc.key).cloned();
            let external_pid = if tracked_entry.is_none() {
                svc.default_port.and_then(pid_on_port)
            } else {
                None
            };
            Row {
                svc,
                bin,
                installed,
                persist_dir,
                tracked_entry,
                external_pid,
            }
        })
        .collect();

    let probes = rows.into_iter().map(|row| async move {
        let status = match (&row.tracked_entry, row.external_pid) {
            (Some(t), _) => ServiceStatus::RunningTracked {
                pid: t.pid,
                started_at: t.started_at,
            },
            (None, Some(pid)) => ServiceStatus::RunningExternal { pid },
            _ => ServiceStatus::Stopped,
        };

        let health = match (&row.tracked_entry, row.external_pid) {
            (Some(t), _)
                if now.saturating_sub(t.started_at) < STARTUP_GRACE_SECS
                    && row.svc.default_port.and_then(pid_on_port) != Some(t.pid) =>
            {
                ServiceHealth::Starting
            }
            (Some(_), _) | (None, Some(_)) => health::check(row.svc).await,
            _ => ServiceHealth::Unknown,
        };

        ServiceInfo {
            key: row.svc.key.to_string(),
            scoop_app: row.svc.scoop_app.to_string(),
            display: row.svc.display.to_string(),
            category: row.svc.category.as_str().to_string(),
            installed: row.installed,
            status,
            health,
            default_port: row.svc.default_port,
            persist_dir: row.persist_dir,
            bin_path: row.bin.map(|p| p.display().to_string()),
        }
    });

    Ok(futures::future::join_all(probes).await)
}

/// Implementation of `services_start` that takes a borrowed `AppState`.
/// Used by `services_start` (Tauri command) and by orchestrators like
/// `presets_apply` and `projects_activate`. Optional `extra_env` is merged
/// onto the parent process env when spawning (project env vars).
pub(crate) async fn services_start_inner(
    key: &str,
    state: &AppState,
) -> Result<ServiceInfo, String> {
    services_start_with_env(key, state, &HashMap::new()).await
}

pub(crate) async fn services_start_with_env(
    key: &str,
    state: &AppState,
    extra_env: &HashMap<String, String>,
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

    // Capture stdout + stderr to <state_dir>/logs/<key>.log so users can
    // tail them from the Logs panel without needing an external terminal.
    let log = service_logs::prepare_for_spawn(svc.key)
        .map_err(|e| format!("failed to open log file for {}: {}", svc.display, e))?;

    // Run a one-time init if needed (Postgres initdb, MySQL --initialize, …).
    // Init output is captured into the same log file so the user can see it.
    if let Some((init_bin, init_args)) = known_services::init_step(svc, &root) {
        let init_log = service_logs::prepare_for_spawn(svc.key)
            .map_err(|e| format!("failed to open log for init step: {e}"))?;

        let mut init_cmd = Command::new(&init_bin);
        init_cmd
            .args(&init_args)
            .current_dir(&cwd)
            .stdout(Stdio::from(init_log.stdout))
            .stderr(Stdio::from(init_log.stderr))
            .stdin(Stdio::null());

        #[cfg(windows)]
        {
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            init_cmd.creation_flags(CREATE_NO_WINDOW);
        }

        let init_status = init_cmd.status().await.map_err(|e| {
            format!(
                "failed to run init step for {} ({}): {}",
                svc.display,
                init_bin.display(),
                e
            )
        })?;
        if !init_status.success() {
            return Err(format!(
                "{} initialization failed (exit {}). See its log for details.",
                svc.display,
                init_status.code().unwrap_or(-1)
            ));
        }
    }

    let mut cmd = Command::new(&bin);
    cmd.args(&args)
        .current_dir(&cwd)
        .envs(extra_env.iter())
        .stdout(Stdio::from(log.stdout))
        .stderr(Stdio::from(log.stderr))
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

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ServiceLog {
    pub key: String,
    pub path: String,
    pub size_bytes: u64,
    pub lines: Vec<String>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ConfigFileInfo {
    pub path: String,
    pub label: String,
    pub language: String,
    pub exists: bool,
    pub size_bytes: u64,
}

#[tauri::command]
pub fn services_config_files(key: String) -> Result<Vec<ConfigFileInfo>, String> {
    let svc = known_services::lookup(&key).ok_or_else(|| format!("unknown service: {key}"))?;
    let root = scoop_root().ok_or_else(|| "Scoop is not installed".to_string())?;
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for cf in known_services::config_files(svc, &root) {
        let path = cf.path.display().to_string();
        if !seen.insert(path.clone()) {
            continue;
        }
        let (exists, size_bytes) = std::fs::metadata(&cf.path)
            .map(|m| (true, m.len()))
            .unwrap_or((false, 0));
        out.push(ConfigFileInfo {
            path,
            label: cf.label.to_string(),
            language: cf.language.to_string(),
            exists,
            size_bytes,
        });
    }
    Ok(out)
}

#[tauri::command]
pub fn services_config_read(path: String) -> Result<String, String> {
    validate_config_path(&path)?;
    std::fs::read_to_string(&path).map_err(|e| format!("failed to read {path}: {e}"))
}

#[tauri::command]
pub fn services_config_write(path: String, content: String) -> Result<(), String> {
    validate_config_path(&path)?;

    // Backup-on-save: previous contents go to <path>.bak. Best-effort —
    // if the source doesn't exist (first write), skip without erroring.
    if let Ok(prev) = std::fs::read(&path) {
        let bak = format!("{path}.bak");
        let _ = std::fs::write(&bak, prev);
    }

    std::fs::write(&path, content).map_err(|e| format!("failed to write {path}: {e}"))
}

/// Defense-in-depth for the config-write path. The frontend only writes to
/// paths returned by services_config_files, but reject anything that isn't
/// inside a Scoop-managed directory just in case.
fn validate_config_path(path: &str) -> Result<(), String> {
    let Some(root) = scoop_root() else {
        return Err("Scoop is not installed".into());
    };
    let normalized = std::path::PathBuf::from(path);
    let canonical = normalized
        .canonicalize()
        .or_else(|_| Ok::<_, std::io::Error>(normalized.clone()))
        .map_err(|e: std::io::Error| e.to_string())?;
    let root_canonical = root
        .canonicalize()
        .unwrap_or(root.clone());
    if !canonical.starts_with(&root_canonical) {
        return Err(format!(
            "refusing to touch path outside Scoop root: {}",
            canonical.display()
        ));
    }
    Ok(())
}

#[tauri::command]
pub fn services_tail_log(key: String, max_lines: Option<usize>) -> Result<ServiceLog, String> {
    let limit = max_lines.unwrap_or(200).clamp(1, 2000);
    let lines = service_logs::tail(&key, limit).map_err(|e| e.to_string())?;
    let path = crate::persistence::log_file_for(&key)
        .display()
        .to_string();
    let size_bytes = service_logs::size(&key);
    Ok(ServiceLog {
        key,
        path,
        size_bytes,
        lines,
    })
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

/// Builds a ServiceInfo without doing any async health probe. The caller
/// (services_start / stop / restart) is returning a snapshot the UI will
/// soon overwrite via the next services_list poll, so it's fine to leave
/// `health` at Unknown here.
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
        health: ServiceHealth::Unknown,
        default_port: svc.default_port,
        persist_dir: root
            .as_ref()
            .and_then(|r| known_services::persist_dir(svc, r))
            .map(|p| p.display().to_string()),
        bin_path: bin.map(|p| p.display().to_string()),
    }
}
