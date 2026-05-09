mod catalog;
mod commands;
mod health;
mod hosts_file;
mod http;
mod known_services;
mod persistence;
mod presets;
mod projects;
mod scoop;
mod scoopsearch;
mod service_logs;
mod ssl;
mod state;
mod tray;
mod vhosts;
mod winutil;

use commands::catalog::{catalog_list, catalog_refresh, catalog_stats, scoop_check};
use commands::ping::ping;
use commands::presets_ops::{presets_apply, presets_list};
use commands::projects_ops::{
    projects_activate, projects_create, projects_deactivate, projects_delete, projects_list,
    projects_open_terminal, projects_update,
};
use commands::scoop_ops::{
    scoop_bootstrap, scoop_cancel, scoop_install, scoop_uninstall, scoop_update,
};
use commands::services::{
    services_config_files, services_config_read, services_config_write, services_list,
    services_open_data, services_open_path, services_restart, services_start, services_stop,
    services_tail_log,
};
use commands::settings::{get_close_to_tray, quit_app, set_close_to_tray};
use state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Honors RUST_LOG; defaults to `info` so production builds still surface
    // warnings/errors without spamming. env_logger is a tiny dep that's well
    // understood and good enough for a desktop app of this size.
    let _ = env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("info"),
    )
    .try_init();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AppState::load_from_disk())
        .setup(|app| {
            tray::install(app)?;
            Ok(())
        })
        .on_window_event(tray::on_window_event)
        .invoke_handler(tauri::generate_handler![
            ping,
            catalog_list,
            catalog_stats,
            catalog_refresh,
            scoop_check,
            scoop_install,
            scoop_uninstall,
            scoop_update,
            scoop_bootstrap,
            scoop_cancel,
            services_list,
            services_start,
            services_stop,
            services_restart,
            services_open_data,
            services_open_path,
            services_tail_log,
            services_config_files,
            services_config_read,
            services_config_write,
            presets_list,
            presets_apply,
            projects_list,
            projects_create,
            projects_update,
            projects_delete,
            projects_activate,
            projects_deactivate,
            projects_open_terminal,
            set_close_to_tray,
            get_close_to_tray,
            quit_app,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
