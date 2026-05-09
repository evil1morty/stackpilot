//! Apply curated presets: install missing apps sequentially, then start their
//! associated services. Reuses scoop_ops::drive for streaming output.

use std::sync::atomic::Ordering;

use serde::Serialize;
use tauri::ipc::Channel;
use tauri::State;

use crate::commands::scoop_ops::{acquire_or_reject, drive, scoop_powershell, ScoopEvent};
use crate::commands::services;
use crate::known_services;
use crate::presets::{self, Preset};
use crate::scoop::scoop_root;
use crate::state::AppState;

// ─────────────────────────────────────────── DTOs ─────────────────────────

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PresetApp {
    pub scoop_app: String,
    pub installed: bool,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PresetService {
    pub key: String,
    pub display: String,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PresetInfo {
    pub key: String,
    pub name: String,
    pub description: String,
    pub apps: Vec<PresetApp>,
    pub auto_start: Vec<PresetService>,
    pub apps_installed: usize,
    pub apps_total: usize,
}

// ─────────────────────────────────────────── helpers ──────────────────────

fn is_app_installed(scoop_app: &str) -> bool {
    let Some(root) = scoop_root() else {
        return false;
    };
    root.join("apps").join(scoop_app).join("current").exists()
}

fn build_info(p: &Preset) -> PresetInfo {
    let mut apps_installed = 0;
    let apps: Vec<PresetApp> = p
        .apps
        .iter()
        .map(|a| {
            let installed = is_app_installed(a);
            if installed {
                apps_installed += 1;
            }
            PresetApp {
                scoop_app: (*a).to_string(),
                installed,
            }
        })
        .collect();

    let auto_start: Vec<PresetService> = p
        .auto_start
        .iter()
        .filter_map(|k| {
            known_services::lookup(k).map(|svc| PresetService {
                key: svc.key.to_string(),
                display: svc.display.to_string(),
            })
        })
        .collect();

    PresetInfo {
        key: p.key.to_string(),
        name: p.name.to_string(),
        description: p.description.to_string(),
        apps_total: apps.len(),
        apps,
        auto_start,
        apps_installed,
    }
}

fn emit_system(on_event: &Channel<ScoopEvent>, msg: impl Into<String>) {
    let _ = on_event.send(ScoopEvent::Stderr {
        line: format!("◇ {}", msg.into()),
    });
}

// ─────────────────────────────────────────── commands ─────────────────────

#[tauri::command]
pub fn presets_list() -> Vec<PresetInfo> {
    presets::PRESETS.iter().map(build_info).collect()
}

#[tauri::command]
pub async fn presets_apply(
    key: String,
    on_event: Channel<ScoopEvent>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let p = presets::lookup(&key).ok_or_else(|| format!("unknown preset: {key}"))?;

    // Hold one slot for the entire preset apply. Each inner `drive` call
    // temporarily swaps the spawned PID into running_pid for cancel
    // targeting, then restores the sentinel — releasing the slot only when
    // _slot drops at function exit.
    let _slot = acquire_or_reject(&state, &on_event)?;

    let _ = on_event.send(ScoopEvent::Started {
        command: format!("preset {} → {}", p.key, p.name),
    });

    let initial_gen = state.cancellation_gen.load(Ordering::SeqCst);
    let was_cancelled = || state.cancellation_gen.load(Ordering::SeqCst) != initial_gen;

    // ── install missing apps sequentially ──
    let missing: Vec<&&str> = p.apps.iter().filter(|a| !is_app_installed(a)).collect();
    if missing.is_empty() {
        emit_system(&on_event, "All apps already installed — skipping install phase.");
    } else {
        emit_system(
            &on_event,
            format!("Installing {} missing app(s).", missing.len()),
        );
        for app in missing {
            if was_cancelled() {
                emit_system(&on_event, "Cancelled.");
                let _ = on_event.send(ScoopEvent::Finished { exit_code: 130 });
                return Ok(());
            }
            emit_system(&on_event, format!("→ scoop install {app}"));
            let cmd = scoop_powershell(&["install", app]).map_err(|e| {
                let _ = on_event.send(ScoopEvent::Error {
                    message: e.clone(),
                });
                e
            })?;
            let exit_code = drive(cmd, &on_event, &state, &format!("scoop install {app}")).await?;
            if exit_code != 0 {
                let msg = format!("install of {app} failed with exit code {exit_code}");
                let _ = on_event.send(ScoopEvent::Error {
                    message: msg.clone(),
                });
                let _ = on_event.send(ScoopEvent::Finished { exit_code });
                state.catalog.refresh();
                return Err(msg);
            }
        }
    }

    state.catalog.refresh();

    if was_cancelled() {
        emit_system(&on_event, "Cancelled before starting services.");
        let _ = on_event.send(ScoopEvent::Finished { exit_code: 130 });
        return Ok(());
    }

    // ── start auto_start services with best-effort error handling ──
    emit_system(&on_event, "Starting services.");
    let mut started = 0usize;
    let mut skipped = 0usize;
    for svc_key in p.auto_start {
        match services::services_start_inner(svc_key, &state).await {
            Ok(_info) => {
                started += 1;
                emit_system(&on_event, format!("✓ {svc_key} started"));
            }
            Err(e) => {
                skipped += 1;
                emit_system(&on_event, format!("• {svc_key} skipped: {e}"));
            }
        }
    }

    emit_system(
        &on_event,
        format!("Preset complete — {started} started, {skipped} skipped."),
    );
    let _ = on_event.send(ScoopEvent::Finished { exit_code: 0 });
    Ok(())
}
