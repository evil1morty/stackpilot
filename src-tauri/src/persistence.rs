//! Disk-backed app state. Stackpilot writes its tracked-service map to a
//! single JSON file under the platform data dir so a relaunch after a crash
//! can re-attach to services it spawned. We're deliberately small here —
//! services are the only thing we persist for now.

use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

const STATE_FILENAME: &str = "state.json";
const STATE_VERSION: u32 = 1;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PersistedService {
    pub key: String,
    pub pid: u32,
    pub port: Option<u16>,
    pub started_at: u64,
}

#[derive(Serialize, Deserialize, Default, Debug)]
pub struct PersistedState {
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default)]
    pub services: Vec<PersistedService>,
}

fn default_version() -> u32 {
    STATE_VERSION
}

/// `%APPDATA%\Stackpilot\` on Windows. Created lazily by save().
pub fn state_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Stackpilot")
}

pub fn state_file() -> PathBuf {
    state_dir().join(STATE_FILENAME)
}

/// Logs live alongside state. Created on first write. Used by Phase B.
#[allow(dead_code)]
pub fn logs_dir() -> PathBuf {
    state_dir().join("logs")
}

/// Read state from disk. Missing or malformed file yields a fresh empty state
/// — we never abort startup for a corrupt JSON, just lose previous tracking.
pub fn load() -> PersistedState {
    let path = state_file();
    let Ok(text) = fs::read_to_string(&path) else {
        return PersistedState {
            version: STATE_VERSION,
            services: Vec::new(),
        };
    };
    serde_json::from_str(&text).unwrap_or_else(|_| PersistedState {
        version: STATE_VERSION,
        services: Vec::new(),
    })
}

/// Atomically write state to disk by writing to a temp file and renaming.
/// Failures are logged-and-eaten — persistence is best-effort.
pub fn save(state: &PersistedState) {
    let dir = state_dir();
    if let Err(e) = fs::create_dir_all(&dir) {
        eprintln!("stackpilot: cannot create state dir {}: {e}", dir.display());
        return;
    }
    let path = state_file();
    let tmp = path.with_extension("json.tmp");

    let text = match serde_json::to_string_pretty(state) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("stackpilot: cannot serialize state: {e}");
            return;
        }
    };

    if let Err(e) = fs::write(&tmp, text) {
        eprintln!("stackpilot: cannot write state tmp file: {e}");
        return;
    }
    if let Err(e) = fs::rename(&tmp, &path) {
        eprintln!("stackpilot: cannot rename state file: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrips_through_json() {
        let state = PersistedState {
            version: STATE_VERSION,
            services: vec![PersistedService {
                key: "redis".into(),
                pid: 12345,
                port: Some(6379),
                started_at: 1_720_000_000,
            }],
        };
        let text = serde_json::to_string(&state).unwrap();
        let back: PersistedState = serde_json::from_str(&text).unwrap();
        assert_eq!(back.services.len(), 1);
        assert_eq!(back.services[0].pid, 12345);
        assert_eq!(back.services[0].port, Some(6379));
    }

    #[test]
    fn missing_fields_default_safely() {
        let raw = r#"{"services":[]}"#; // no version
        let parsed: PersistedState = serde_json::from_str(raw).unwrap();
        assert_eq!(parsed.version, STATE_VERSION);
        assert!(parsed.services.is_empty());
    }

    #[test]
    fn malformed_json_treated_as_empty() {
        let raw = "{not valid json";
        let parsed: PersistedState = serde_json::from_str(raw).unwrap_or_default();
        assert!(parsed.services.is_empty());
    }
}
