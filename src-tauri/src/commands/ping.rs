use serde::Serialize;

use crate::scoop::scoop_root;

#[derive(Serialize)]
pub struct PingResponse {
    pub ok: bool,
    pub message: String,
    pub scoop_root: Option<String>,
}

#[tauri::command]
pub fn ping() -> PingResponse {
    let root = scoop_root();
    let (message, ok) = match &root {
        Some(p) => (format!("Scoop detected at {}", p.display()), true),
        None => (
            "Scoop not detected. Stackpilot will offer to bootstrap it.".to_string(),
            true,
        ),
    };

    PingResponse {
        ok,
        message,
        scoop_root: root.map(|p| p.display().to_string()),
    }
}
