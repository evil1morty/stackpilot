//! User-defined project workspaces. Each project is a name + optional root
//! directory + the set of services to run + per-project env vars. Activating
//! a project stops whatever is currently running and starts the project's
//! services with its env applied.
//!
//! Stored as a single JSON file at `%APPDATA%\Stackpilot\projects.json`.

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::persistence::state_dir;

const PROJECTS_FILENAME: &str = "projects.json";
const PROJECTS_VERSION: u32 = 1;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub key: String,
    pub name: String,
    /// Project root on disk (where the user's code lives). Optional —
    /// projects can be services-only, e.g. "Local databases".
    #[serde(default)]
    pub root_dir: String,
    /// `KnownService::key`s to start when activated.
    #[serde(default)]
    pub services: Vec<String>,
    /// Env vars merged onto the parent process env when spawning services.
    #[serde(default)]
    pub env_vars: BTreeMap<String, String>,
    #[serde(default)]
    pub notes: String,
    pub created_at: u64,
    #[serde(default)]
    pub last_active_at: Option<u64>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct ProjectsFile {
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default)]
    pub projects: Vec<Project>,
    #[serde(default)]
    pub active_key: Option<String>,
}

fn default_version() -> u32 {
    PROJECTS_VERSION
}

fn projects_file_path() -> PathBuf {
    state_dir().join(PROJECTS_FILENAME)
}

pub fn load() -> ProjectsFile {
    let path = projects_file_path();
    let Ok(text) = fs::read_to_string(&path) else {
        return ProjectsFile {
            version: PROJECTS_VERSION,
            projects: Vec::new(),
            active_key: None,
        };
    };
    serde_json::from_str(&text).unwrap_or_default()
}

pub fn save(file: &ProjectsFile) -> Result<(), String> {
    let dir = state_dir();
    fs::create_dir_all(&dir).map_err(|e| format!("create state dir: {e}"))?;
    let path = projects_file_path();
    let tmp = path.with_extension("json.tmp");
    let text = serde_json::to_string_pretty(file).map_err(|e| e.to_string())?;
    fs::write(&tmp, text).map_err(|e| format!("write tmp: {e}"))?;
    fs::rename(&tmp, &path).map_err(|e| format!("rename: {e}"))?;
    Ok(())
}

/// Generate a unique key from a free-form name. Lowercase, alphanumerics +
/// dashes, suffixed with a counter on collision.
pub fn generate_key(name: &str, taken: &[String]) -> String {
    let base: String = name
        .chars()
        .flat_map(|c| {
            if c.is_ascii_alphanumeric() {
                vec![c.to_ascii_lowercase()]
            } else if c.is_whitespace() || matches!(c, '-' | '_' | '.' | '/') {
                vec!['-']
            } else {
                vec![]
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .replace("--", "-");
    let base = if base.is_empty() {
        "project".to_string()
    } else {
        base
    };

    if !taken.contains(&base) {
        return base;
    }
    for n in 2..1000 {
        let candidate = format!("{base}-{n}");
        if !taken.contains(&candidate) {
            return candidate;
        }
    }
    format!("{base}-{}", uuid())
}

fn uuid() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_micros())
        .unwrap_or(0)
        .to_string()
}

pub fn now() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Convert env_vars into something tokio::process::Command::envs accepts.
pub fn env_iter(env_vars: &BTreeMap<String, String>) -> HashMap<String, String> {
    env_vars.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_generation_basics() {
        assert_eq!(generate_key("My App", &[]), "my-app");
        assert_eq!(generate_key("foo bar baz", &[]), "foo-bar-baz");
        assert_eq!(generate_key("CamelCase", &[]), "camelcase");
    }

    #[test]
    fn key_generation_collisions() {
        let taken = vec!["app".to_string(), "app-2".to_string()];
        assert_eq!(generate_key("app", &taken), "app-3");
    }

    #[test]
    fn key_generation_empty() {
        assert_eq!(generate_key("", &[]), "project");
        assert_eq!(generate_key("!!!", &[]), "project");
    }

    #[test]
    fn projects_file_roundtrips() {
        let mut env = BTreeMap::new();
        env.insert("DB_URL".into(), "postgres://localhost/myapp".into());

        let file = ProjectsFile {
            version: 1,
            projects: vec![Project {
                key: "myapp".into(),
                name: "MyApp".into(),
                root_dir: "C:/code/myapp".into(),
                services: vec!["redis".into(), "postgresql".into()],
                env_vars: env,
                notes: "main project".into(),
                created_at: 1_700_000_000,
                last_active_at: None,
            }],
            active_key: Some("myapp".into()),
        };

        let text = serde_json::to_string(&file).unwrap();
        let back: ProjectsFile = serde_json::from_str(&text).unwrap();
        assert_eq!(back.projects.len(), 1);
        assert_eq!(back.projects[0].services, vec!["redis", "postgresql"]);
        assert_eq!(back.active_key.as_deref(), Some("myapp"));
    }
}
