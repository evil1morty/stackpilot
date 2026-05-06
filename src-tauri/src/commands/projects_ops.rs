//! Project / workspace CRUD + activation orchestration.
//!
//! Activation is the interesting bit: it stops every currently-tracked
//! service that isn't part of the new project, then starts the new project's
//! services with the project's env vars merged onto the parent env.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::commands::services;
use crate::known_services;
use crate::projects;
use crate::state::AppState;

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
    let cloned = project.clone();
    projects::save(&file)?;
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
    Ok(())
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ActivationReport {
    pub stopped: Vec<String>,
    pub started: Vec<String>,
    pub failed: Vec<ServiceFailure>,
    pub project: ProjectInfo,
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
    })
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
    }
    Ok(stopped)
}
