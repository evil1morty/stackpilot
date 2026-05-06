use std::collections::HashMap;

use tauri::State;

use crate::catalog::{self, AppEntry, CatalogStats, ScoopStatus, SortBy};
use crate::scoopsearch;
use crate::state::AppState;

const RESULT_CAP: usize = 100;

/// Browse Scoop's catalog. Tries the online scoopsearch index first (real
/// BM25 relevance, last-commit timestamps, official-bucket filter). Falls
/// back to the local catalog if the network call fails or the user wants
/// installed-only — scoopsearch can't filter by what's installed locally.
#[tauri::command]
pub async fn catalog_list(
    query: Option<String>,
    bucket: Option<String>,
    installed_only: Option<bool>,
    sort: Option<SortBy>,
    state: State<'_, AppState>,
) -> Result<Vec<AppEntry>, String> {
    let installed_only = installed_only.unwrap_or(false);
    let sort = sort.unwrap_or_default();

    if installed_only {
        let catalog = state.catalog.ensure();
        return Ok(catalog::filter(
            &catalog,
            query.as_deref(),
            bucket.as_deref(),
            true,
            sort,
            RESULT_CAP,
        ));
    }

    // Try online first.
    let online = scoopsearch::search(scoopsearch::SearchOptions {
        query: query.as_deref(),
        sort,
        skip: 0,
        top: RESULT_CAP as u32,
        include_community: false,
    })
    .await;

    if let Ok(result) = online {
        let bucket_filter = bucket.as_deref();
        return Ok(result
            .items
            .into_iter()
            .filter(|app| match bucket_filter {
                Some(b) => app.bucket == b,
                None => true,
            })
            .map(merge_with_local_installed)
            .collect());
    }

    // Network failed → fall back to the local index.
    let catalog = state.catalog.ensure();
    Ok(catalog::filter(
        &catalog,
        query.as_deref(),
        bucket.as_deref(),
        false,
        sort,
        RESULT_CAP,
    ))
}

/// Convert an OnlineApp into an AppEntry, attaching local installed-state
/// without doing a full catalog walk.
fn merge_with_local_installed(app: scoopsearch::OnlineApp) -> AppEntry {
    let installed = catalog::lookup_installed(&app.name);
    AppEntry {
        name: app.name,
        bucket: app.bucket,
        version: app.version,
        description: app.description,
        homepage: app.homepage,
        license: app.license,
        depends: vec![],
        suggest: vec![],
        bins: vec![],
        supports_arch: vec!["64bit".to_string()],
        installed,
        committed: app.committed,
        repository: app.repository,
        repository_stars: app.repository_stars,
        highlights: app.highlights,
    }
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

// Suppress dead-code warning in the smoke test which builds catalog.rs alone.
#[allow(dead_code)]
fn _unused_hashmap_marker() -> HashMap<String, String> {
    HashMap::new()
}
