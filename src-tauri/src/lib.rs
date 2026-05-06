mod catalog;
mod commands;
mod known_services;
mod scoop;
mod state;

use commands::catalog::{catalog_list, catalog_refresh, catalog_stats, scoop_check};
use commands::ping::ping;
use commands::scoop_ops::{scoop_bootstrap, scoop_cancel, scoop_install, scoop_uninstall};
use commands::services::{
    services_list, services_open_data, services_restart, services_start, services_stop,
};
use state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            ping,
            catalog_list,
            catalog_stats,
            catalog_refresh,
            scoop_check,
            scoop_install,
            scoop_uninstall,
            scoop_bootstrap,
            scoop_cancel,
            services_list,
            services_start,
            services_stop,
            services_restart,
            services_open_data,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
