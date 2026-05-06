use tauri::State;

use crate::catalog::{self, AppEntry, CatalogStats, ScoopStatus};
use crate::state::AppState;

const RESULT_CAP: usize = 300;

#[tauri::command]
pub fn catalog_list(
    query: Option<String>,
    bucket: Option<String>,
    installed_only: Option<bool>,
    state: State<'_, AppState>,
) -> Vec<AppEntry> {
    let catalog = state.catalog.ensure();
    catalog::filter(
        &catalog,
        query.as_deref(),
        bucket.as_deref(),
        installed_only.unwrap_or(false),
        RESULT_CAP,
    )
}

#[tauri::command]
pub fn catalog_stats(state: State<'_, AppState>) -> CatalogStats {
    let catalog = state.catalog.ensure();
    catalog::stats(&catalog)
}

#[tauri::command]
pub fn catalog_refresh(state: State<'_, AppState>) -> CatalogStats {
    let catalog = state.catalog.refresh();
    catalog::stats(&catalog)
}

#[tauri::command]
pub fn scoop_check() -> ScoopStatus {
    catalog::current_status()
}
