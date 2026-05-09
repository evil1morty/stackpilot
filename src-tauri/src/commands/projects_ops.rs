//! Project / workspace CRUD + activation orchestration.
//!
//! Activation is the interesting bit: it stops every currently-tracked
//! service that isn't part of the new project, then starts the new project's
//! services with the project's env vars merged onto the parent env.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::commands::services;
use crate::hosts_file;
use crate::known_services;
use crate::projects;
use crate::scoop::scoop_root;
use crate::ssl;
use crate::state::AppState;
use crate::vhosts;
use crate::winutil::{hide_console_std, which};

/// Collect hostnames referenced by any project's vhosts. Used to GC stale
/// cert files after project mutations.
fn all_active_vhost_hosts(file: &projects::ProjectsFile) -> std::collections::HashSet<String> {
    let mut hosts = std::collections::HashSet::new();
    for p in &file.projects {
        for vh in &p.vhosts {
            let trimmed = vh.host.trim();
            if !trimmed.is_empty() {
                hosts.insert(trimmed.to_string());
            }
        }
    }
    hosts
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ProjectInfo {
    #[serde(flatten)]
    pub project: projects::Project,
    pub is_active: bool,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ProjectInput {
    pub name: String,
    #[serde(default)]
    pub root_dir: String,
    #[serde(default)]
    pub services: Vec<String>,
    #[serde(default)]
    pub env_vars: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub vhosts: Vec<projects::VHost>,
}

fn validate_input(input: &ProjectInput) -> Result<(), String> {
    if input.name.trim().is_empty() {
        return Err("project name cannot be empty".into());
    }
    for key in input.services.iter() {
        if known_services::lookup(key).is_none() {
            return Err(format!("unknown service: {key}"));
        }
    }
    for env_key in input.env_vars.keys() {
        if env_key.is_empty() || env_key.contains('=') {
            return Err(format!("invalid env var name: {env_key:?}"));
        }
    }
    Ok(())
}

fn to_info(p: projects::Project, active_key: &Option<String>) -> ProjectInfo {
    let is_active = active_key.as_deref() == Some(p.key.as_str());
    ProjectInfo {
        project: p,
        is_active,
    }
}

#[tauri::command]
pub fn projects_list() -> Vec<ProjectInfo> {
    let file = projects::load();
    file.projects
        .into_iter()
        .map(|p| to_info(p, &file.active_key))
        .collect()
}

#[tauri::command]
pub fn projects_create(input: ProjectInput) -> Result<ProjectInfo, String> {
    validate_input(&input)?;
    let mut file = projects::load();
    let taken: Vec<String> = file.projects.iter().map(|p| p.key.clone()).collect();
    let key = projects::generate_key(&input.name, &taken);
    let project = projects::Project {
        key,
        name: input.name,
        root_dir: input.root_dir,
        services: input.services,
        env_vars: input.env_vars,
        notes: input.notes,
        vhosts: input.vhosts,
        created_at: projects::now(),
        last_active_at: None,
    };
    file.projects.push(project.clone());
    projects::save(&file)?;
    Ok(to_info(project, &file.active_key))
}

#[tauri::command]
pub fn projects_update(key: String, input: ProjectInput) -> Result<ProjectInfo, String> {
    validate_input(&input)?;
    let mut file = projects::load();
    let project = file
        .projects
        .iter_mut()
        .find(|p| p.key == key)
        .ok_or_else(|| format!("project not found: {key}"))?;
    project.name = input.name;
    project.root_dir = input.root_dir;
    project.services = input.services;
    project.env_vars = input.env_vars;
    project.notes = input.notes;
    project.vhosts = input.vhosts;
    let cloned = project.clone();
    projects::save(&file)?;
    // Project edits can drop vhosts; sweep cert files that no project
    // references any more.
    ssl::reap_orphans(&all_active_vhost_hosts(&file));
    Ok(to_info(cloned, &file.active_key))
}

#[tauri::command]
pub fn projects_delete(key: String) -> Result<(), String> {
    let mut file = projects::load();
    let before = file.projects.len();
    file.projects.retain(|p| p.key != key);
    if file.projects.len() == before {
        return Err(format!("project not found: {key}"));
    }
    if file.active_key.as_deref() == Some(key.as_str()) {
        file.active_key = None;
    }
    projects::save(&file)?;
    // Drop cert files for hosts no longer referenced by any project.
    ssl::reap_orphans(&all_active_vhost_hosts(&file));
    Ok(())
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ActivationReport {
    pub stopped: Vec<String>,
    pub started: Vec<String>,
    pub failed: Vec<ServiceFailure>,
    pub project: ProjectInfo,
    /// Number of vhost config files written. None if nginx isn't installed.
    pub vhosts_written: Option<usize>,
    /// True if the hosts file was actually mutated (UAC prompt fired).
    /// False if no change was needed.
    pub hosts_file_updated: bool,
    /// Soft warnings collected during vhost emission (nginx not installed,
    /// nginx.conf missing, etc) — surfaced to the user but non-fatal.
    pub vhost_warnings: Vec<String>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ServiceFailure {
    pub key: String,
    pub error: String,
}

#[tauri::command]
pub async fn projects_activate(
    key: String,
    state: State<'_, AppState>,
) -> Result<ActivationReport, String> {
    let mut file = projects::load();
    let project_idx = file
        .projects
        .iter()
        .position(|p| p.key == key)
        .ok_or_else(|| format!("project not found: {key}"))?;

    let project = file.projects[project_idx].clone();
    let target_keys: std::collections::HashSet<String> =
        project.services.iter().cloned().collect();

    // Phase 1 — stop tracked services not in the new project.
    let to_stop: Vec<String> = {
        state
            .tracked
            .lock()
            .keys()
            .filter(|k| !target_keys.contains(k.as_str()))
            .cloned()
            .collect()
    };
    let mut stopped = Vec::new();
    for svc_key in &to_stop {
        if services::services_stop(svc_key.clone(), state.clone())
            .await
            .is_ok()
        {
            stopped.push(svc_key.clone());
        }
    }

    // Phase 2 — start project services that aren't already running.
    let env_map: HashMap<String, String> = projects::env_iter(&project.env_vars);
    let already_running: std::collections::HashSet<String> =
        state.tracked.lock().keys().cloned().collect();

    let mut started = Vec::new();
    let mut failed = Vec::new();
    for svc_key in &project.services {
        if already_running.contains(svc_key) {
            continue;
        }
        match services::services_start_with_env(svc_key, &state, &env_map).await {
            Ok(_) => started.push(svc_key.clone()),
            Err(e) => failed.push(ServiceFailure {
                key: svc_key.clone(),
                error: e,
            }),
        }
    }

    // Phase 3 — emit vhost configs + sync hosts file. Best-effort:
    // failures here become soft warnings, the project still activates.
    let mut vhost_warnings: Vec<String> = Vec::new();
    let mut vhosts_written: Option<usize> = None;
    let mut hosts_file_updated = false;

    if !project.vhosts.is_empty() {
        if let Some(root) = scoop_root() {
            match vhosts::emit(&project, &root) {
                Ok(outcome) => {
                    vhosts_written = Some(outcome.written.len());
                    vhost_warnings.extend(outcome.warnings);
                }
                Err(e) => vhost_warnings.push(format!("vhost emit: {e}")),
            }
            match vhosts::ensure_nginx_include(&root) {
                Ok(true) => vhost_warnings
                    .push("Patched nginx.conf to include vhosts (backup at nginx.conf.bak)".into()),
                Ok(false) => {}
                Err(e) => vhost_warnings.push(format!("nginx.conf patch: {e}")),
            }
        } else {
            vhost_warnings.push("Scoop not installed — skipped vhost emission".into());
        }

        // Sync hosts file. We do this even if vhost emission failed —
        // resolving the host is independent of nginx serving it.
        let aggregated = collect_active_hosts(&file, Some(&project.key));
        match hosts_file::replace_block(&aggregated) {
            Ok(true) => hosts_file_updated = true,
            Ok(false) => {}
            Err(e) => vhost_warnings.push(format!("hosts file: {e}")),
        }

        // Reload nginx if it's running. Failure is soft — user can manually
        // restart from the Services page if needed.
        if state.tracked.lock().contains_key("nginx") {
            let _ = services::services_restart("nginx".to_string(), state.clone()).await;
        }
    }

    // Mark project as active + bump last_active_at.
    file.projects[project_idx].last_active_at = Some(projects::now());
    file.active_key = Some(project.key.clone());
    projects::save(&file)?;

    let updated = file.projects[project_idx].clone();
    let active_key = file.active_key.clone();
    Ok(ActivationReport {
        stopped,
        started,
        failed,
        project: to_info(updated, &active_key),
        vhosts_written,
        hosts_file_updated,
        vhost_warnings,
    })
}

/// Hosts to keep in the hosts-file managed block. The currently-activating
/// project always contributes; everyone else contributes only if they were
/// last active recently — but for v0.3 we keep it simple: just the active
/// project.
fn collect_active_hosts(file: &projects::ProjectsFile, also: Option<&str>) -> Vec<String> {
    let mut out = Vec::new();
    let key_filter = match (file.active_key.as_deref(), also) {
        (Some(a), _) => Some(a.to_string()),
        (None, Some(b)) => Some(b.to_string()),
        _ => None,
    };
    let Some(key) = key_filter else { return out };
    if let Some(p) = file.projects.iter().find(|p| p.key == key) {
        for vh in &p.vhosts {
            if !vh.host.trim().is_empty() {
                out.push(vh.host.clone());
            }
        }
    }
    let _ = also;
    out
}

/// Open a new terminal window at the project's root_dir with the project's
/// env_vars applied. Tries Windows Terminal first, falls back to plain
/// PowerShell. Both inherit env vars from the spawned cmd-shim, so the user
/// gets DB_URL etc set automatically.
#[tauri::command]
pub fn projects_open_terminal(key: String) -> Result<(), String> {
    let file = projects::load();
    let project = file
        .projects
        .iter()
        .find(|p| p.key == key)
        .ok_or_else(|| format!("project not found: {key}"))?
        .clone();

    let cwd = if project.root_dir.trim().is_empty() {
        dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("."))
    } else {
        std::path::PathBuf::from(&project.root_dir)
    };
    if !cwd.exists() {
        return Err(format!(
            "root directory does not exist: {}",
            cwd.display()
        ));
    }

    let cwd_str = cwd.display().to_string();

    // Prefer Windows Terminal if it's on PATH; users who installed it via
    // Scoop or the Store will get a nicer experience.
    let use_wt = which("wt", &["exe", "cmd", "bat"]).is_some();

    let mut cmd = std::process::Command::new("cmd");
    if use_wt {
        // wt -d <dir> opens a new tab at the requested directory in the
        // user's default profile (PowerShell or pwsh).
        cmd.args(["/c", "start", "", "wt", "-d", &cwd_str]);
    } else {
        // Plain PowerShell with -NoExit so the window stays open after `cd`.
        // Single-quoting works because we only embed file paths and we
        // double up any embedded apostrophes for safety.
        let cd = format!(
            "Set-Location -LiteralPath '{}'",
            cwd_str.replace('\'', "''")
        );
        cmd.args([
            "/c",
            "start",
            "Stackpilot terminal",
            "powershell.exe",
            "-NoExit",
            "-Command",
            &cd,
        ]);
    }

    for (k, v) in &project.env_vars {
        cmd.env(k, v);
    }

    hide_console_std(&mut cmd);

    cmd.stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .stdin(std::process::Stdio::null())
        .spawn()
        .map_err(|e| format!("failed to launch terminal: {e}"))?;

    Ok(())
}

#[tauri::command]
pub async fn projects_deactivate(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    let mut file = projects::load();
    let active_key = file.active_key.clone();
    let project = active_key
        .as_ref()
        .and_then(|k| file.projects.iter().find(|p| &p.key == k).cloned());
    file.active_key = None;
    projects::save(&file)?;

    let mut stopped = Vec::new();
    if let Some(p) = project {
        for svc_key in &p.services {
            if services::services_stop(svc_key.clone(), state.clone())
                .await
                .is_ok()
            {
                stopped.push(svc_key.clone());
            }
        }

        // Drop our vhost configs + hosts entries for this project. Best-effort.
        if !p.vhosts.is_empty() {
            if let Some(root) = scoop_root() {
                let _ = vhosts::cleanup(&p, &root);
            }
            // Empty list strips our managed block entirely — no UAC prompt
            // unless the file actually had stackpilot entries.
            let _ = hosts_file::replace_block(&[]);
        }
    }
    Ok(stopped)
}
