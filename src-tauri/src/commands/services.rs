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
use crate::winutil::hide_console_tokio;

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
///
/// Prefer `PortMap::get` when probing more than one port back-to-back —
/// `listeners::get_all` enumerates the kernel TCP table once and is much
/// cheaper than this call repeated per port.
fn pid_on_port(port: u16) -> Option<u32> {
    listeners::get_process_by_port(port, listeners::Protocol::TCP)
        .ok()
        .map(|p| p.pid)
}

/// Snapshot of TCP-port → owning PID. Built once per `services_list` call
/// and shared across every row's status + health probe so we don't pay for
/// `GetExtendedTcpTable` (or its non-Windows equivalent) per service.
struct PortMap(HashMap<u16, u32>);

impl PortMap {
    fn snapshot() -> Self {
        let mut map = HashMap::new();
        if let Ok(listeners) = listeners::get_all() {
            for l in listeners {
                if matches!(l.protocol, listeners::Protocol::TCP) {
                    // Multiple sockets can bind the same port (IPv4 + IPv6);
                    // first writer wins — they should all reference the same
                    // owning process.
                    map.entry(l.socket.port()).or_insert(l.process.pid);
                }
            }
        }
        Self(map)
    }

    fn get(&self, port: u16) -> Option<u32> {
        self.0.get(&port).copied()
    }
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
fn sweep_dead_with(tracked: &mut HashMap<String, TrackedService>, ports: &PortMap) -> bool {
    let now = unix_now();
    let mut dead: Vec<String> = Vec::new();
    for (k, t) in tracked.iter() {
        // Grace window: don't cull a freshly-started service that hasn't
        // bound its port yet.
        if now.saturating_sub(t.started_at) < STARTUP_GRACE_SECS {
            continue;
        }
        let alive = match t.port {
            Some(port) => ports.get(port) == Some(t.pid),
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

/// Wrapper that builds a fresh PortMap for callers that don't have one in
/// hand (e.g. services_stop). For services_list, prefer the snapshot path.
fn sweep_dead(tracked: &mut HashMap<String, TrackedService>) -> bool {
    sweep_dead_with(tracked, &PortMap::snapshot())
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

    // ONE TCP-table enumeration shared across sweep + every row + every
    // health-probe staleness check. Calling `pid_on_port` per row used to
    // hit the kernel ~3× per service.
    let ports = PortMap::snapshot();

    // Sweep + persist before snapshotting so the snapshot reflects truth.
    {
        let mut tracked = state.tracked.lock();
        let changed = sweep_dead_with(&mut tracked, &ports);
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
        port_owner: Option<u32>,
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
            let port_owner = svc.default_port.and_then(|p| ports.get(p));
            let external_pid = if tracked_entry.is_none() { port_owner } else { None };
            Row {
                svc,
                bin,
                installed,
                persist_dir,
                tracked_entry,
                external_pid,
                port_owner,
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
                    && row.port_owner != Some(t.pid) =>
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

        hide_console_tokio(&mut init_cmd);

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

    hide_console_tokio(&mut cmd);

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

    // Post-spawn verification: poll for up to ~3 s checking that our PID
    // is the one bound to the service's port. If the process exits early
    // (postgres "pre-existing shared memory block", redis bind error, etc.)
    // we surface the last few log lines instead of falsely reporting OK.
    if let Some(port) = svc.default_port {
        let verified = verify_started(pid, port).await;
        if !verified {
            let tail = service_logs::tail(svc.key, 12)
                .ok()
                .map(|lines| {
                    lines
                        .into_iter()
                        .filter(|l| !l.trim().is_empty())
                        .collect::<Vec<_>>()
                        .join("\n")
                })
                .unwrap_or_default();

            // Best-effort taskkill in case the process is still alive but
            // not bound (e.g. infinite-recovery loop).
            let mut kill = Command::new("taskkill");
            kill.args(["/T", "/F", "/PID", &pid.to_string()])
                .stdout(Stdio::null())
                .stderr(Stdio::null());
            hide_console_tokio(&mut kill);
            let _ = kill.status().await;

            let hint = startup_failure_hint(svc, &tail);
            return Err(format_startup_failure(svc, &tail, hint.as_deref()));
        }
    }

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

/// Wait up to ~3 seconds for the spawned PID to bind the expected port.
/// Returns true on success, false if the process never bound (likely died
/// or is in a recovery loop).
async fn verify_started(pid: u32, port: u16) -> bool {
    use std::time::Duration;
    for _ in 0..15 {
        tokio::time::sleep(Duration::from_millis(200)).await;
        if pid_on_port(port) == Some(pid) {
            return true;
        }
    }
    false
}

/// Detect known failure signatures in the log tail and return a friendly
/// hint pointing at the fix.
fn startup_failure_hint(svc: &KnownService, tail: &str) -> Option<String> {
    let lower = tail.to_lowercase();
    if lower.contains("pre-existing shared memory block is still in use") {
        return Some(format!(
            "An older {} instance is still running. Open Task Manager → kill any \
             postgres.exe processes, then click Start again.",
            svc.display
        ));
    }
    if lower.contains("address already in use") || lower.contains("bind: address already in use") {
        return Some(format!(
            "Something else is bound to port {}. Stop the other process and retry.",
            svc.default_port.unwrap_or(0)
        ));
    }
    if lower.contains("permission denied") {
        return Some("Permission denied — check the service's data directory permissions.".into());
    }
    if lower.contains("could not lock file") {
        return Some(format!(
            "{}'s data directory is locked by another process. Make sure no other \
             instance is running.",
            svc.display
        ));
    }
    None
}

fn format_startup_failure(svc: &KnownService, tail: &str, hint: Option<&str>) -> String {
    let mut msg = format!("{} failed to start.", svc.display);
    if let Some(h) = hint {
        msg.push_str("\n\n");
        msg.push_str(h);
    }
    if !tail.trim().is_empty() {
        msg.push_str("\n\nLast log lines:\n");
        msg.push_str(tail);
    }
    msg
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

    let (pid_to_kill, mutated) = {
        let mut tracked = state.tracked.lock();
        let swept = sweep_dead(&mut tracked);
        let removed = tracked.remove(svc.key);
        let mutated = swept || removed.is_some();
        let pid = if let Some(t) = removed {
            Some(t.pid)
        } else if let Some(port) = svc.default_port {
            // Stop external process listening on our port — covers cases
            // where the user started it outside Stackpilot.
            pid_on_port(port)
        } else {
            None
        };
        (pid, mutated)
    };
    if mutated {
        state.persist_tracked();
    }

    let pid = match pid_to_kill {
        Some(p) => p,
        None => return Err(format!("{} is not running", svc.display)),
    };

    let mut kill = Command::new("taskkill");
    kill.args(["/T", "/F", "/PID", &pid.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    hide_console_tokio(&mut kill);

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
    /// File is in the install dir and will be clobbered by `scoop update`.
    pub volatile: bool,
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
            volatile: cf.volatile,
        });
    }
    Ok(out)
}

/// Open a path with the system's default app (notepad for .conf/.log on
/// Windows by default, or whatever the user has registered). Used by the
/// "Open in editor" / "Open log" buttons.
#[tauri::command]
pub fn services_open_path(
    path: String,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let p = std::path::Path::new(&path);
    if !p.exists() {
        return Err(format!("file does not exist: {path}"));
    }
    app.opener()
        .open_path(path, None::<&str>)
        .map_err(|e| e.to_string())
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
/// paths returned by services_config_files, but we reject anything outside a
/// Scoop-managed directory just in case.
///
/// Strategy: canonicalize root and the longest existing ancestor of `path`,
/// then compare prefixes. `dunce::canonicalize` strips Windows' `\\?\`
/// extended-length prefix so the comparison stays sane. We canonicalize the
/// existing ancestor (rather than `path` itself) so the validation works
/// even when the file doesn't exist yet — useful for first-time saves.
fn validate_config_path(path: &str) -> Result<(), String> {
    let Some(root) = scoop_root() else {
        return Err("Scoop is not installed".into());
    };

    let candidate = std::path::Path::new(path);
    let canonical_root = dunce::canonicalize(&root)
        .map_err(|e| format!("canonicalize scoop root: {e}"))?;

    // Walk up to the deepest existing ancestor — symlinks/junctions resolve
    // here, so a config-file path that has been swapped to point outside the
    // Scoop tree gets caught.
    let mut probe: Option<&std::path::Path> = Some(candidate);
    let canonical_target = loop {
        let Some(p) = probe else {
            return Err(format!("could not resolve any ancestor of {path}"));
        };
        if p.exists() {
            break dunce::canonicalize(p)
                .map_err(|e| format!("canonicalize {}: {e}", p.display()))?;
        }
        probe = p.parent();
    };

    if !canonical_target.starts_with(&canonical_root) {
        return Err(format!("refusing to touch path outside Scoop root: {path}"));
    }

    // Belt-and-braces: even after canonicalization, reject `..` segments in
    // the *original* input — keeps surprises out of error messages and
    // avoids relying on canonicalize semantics for traversal-style attacks.
    let path_norm = path.replace('\\', "/");
    if path_norm.contains("/../") || path_norm.starts_with("../") || path_norm.ends_with("/..") {
        return Err(format!("path contains parent traversal: {path}"));
    }
    Ok(())
}

#[tauri::command]
pub fn services_tail_log(
    key: String,
    max_lines: Option<usize>,
    since_size: Option<u64>,
) -> Result<ServiceLog, String> {
    let limit = max_lines.unwrap_or(200).clamp(1, 2000);
    let path = crate::persistence::log_file_for(&key)
        .display()
        .to_string();
    let size_bytes = service_logs::size(&key);

    // Fast path: caller has a snapshot and the file hasn't grown — return an
    // empty `lines` list so the frontend can skip the re-render. Saves a
    // 64 KB read + ~500 line decode every poll for an idle service.
    if let Some(since) = since_size {
        if since == size_bytes {
            return Ok(ServiceLog {
                key,
                path,
                size_bytes,
                lines: Vec::new(),
            });
        }
    }

    let lines = service_logs::tail(&key, limit).map_err(|e| e.to_string())?;
    Ok(ServiceLog {
        key,
        path,
        size_bytes,
        lines,
    })
}

/// Open the most useful folder for a given service: the persist dir if it
/// exists and has anything in it, otherwise the install dir (where Scoop
/// services without `persist` keep their data — Redis writes RDB files
/// next to the binary, Caddy ships its Caddyfile here, etc.).
#[tauri::command]
pub fn services_open_data(key: String, app: AppHandle) -> Result<(), String> {
    let svc = known_services::lookup(&key).ok_or_else(|| format!("unknown service: {key}"))?;
    let root = scoop_root().ok_or_else(|| "Scoop is not installed".to_string())?;

    let persist = known_services::persist_dir(svc, &root);
    let install = root.join("apps").join(svc.scoop_app).join("current");

    let target = persist
        .filter(|p| {
            // Use persist iff it exists AND has at least one entry.
            p.is_dir()
                && std::fs::read_dir(p)
                    .map(|mut rd| rd.next().is_some())
                    .unwrap_or(false)
        })
        .unwrap_or_else(|| install.clone());

    if !target.exists() {
        return Err(format!(
            "{} install directory not found at {}.",
            svc.display,
            install.display()
        ));
    }

    app.opener()
        .open_path(target.display().to_string(), None::<&str>)
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
