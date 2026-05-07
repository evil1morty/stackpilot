//! Per-runtime settings the frontend can flip. For now just one toggle —
//! whether the close button hides to tray or exits. Persisted in
//! localStorage on the frontend; mirrored here on every change so the
//! native close handler can read it synchronously.

use std::sync::atomic::Ordering;

use tauri::State;

use crate::state::AppState;

#[tauri::command]
pub fn set_close_to_tray(enabled: bool, state: State<'_, AppState>) {
    state.close_to_tray.store(enabled, Ordering::SeqCst);
}

#[tauri::command]
pub fn get_close_to_tray(state: State<'_, AppState>) -> bool {
    state.close_to_tray.load(Ordering::SeqCst)
}

#[tauri::command]
pub fn quit_app(app: tauri::AppHandle) {
    app.exit(0);
}
