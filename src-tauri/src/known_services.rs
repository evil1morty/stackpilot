//! Curated table of services Stackpilot knows how to start, stop, and inspect.
//! Each entry maps to a Scoop manifest plus the metadata we need to launch the
//! right binary with the right working directory + arguments.

use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug)]
#[allow(dead_code)] // reserved for future service additions
pub enum Category {
    Database,
    Cache,
    WebServer,
    MessageQueue,
    Search,
    Storage,
}

impl Category {
    pub fn as_str(&self) -> &'static str {
        match self {
            Category::Database => "database",
            Category::Cache => "cache",
            Category::WebServer => "webserver",
            Category::MessageQueue => "queue",
            Category::Search => "search",
            Category::Storage => "storage",
        }
    }
}

pub struct KnownService {
    /// Stable internal key (used in IPC).
    pub key: &'static str,
    /// App name as it appears in the Scoop catalog.
    pub scoop_app: &'static str,
    pub display: &'static str,
    pub category: Category,
    /// Path of the executable, relative to <scoop_root>/apps/<app>/current.
    pub bin_relpath: &'static str,
    /// Default port the service listens on (for conflict-detection + display).
    pub default_port: Option<u16>,
    /// Subdirectory under <scoop_root>/persist/<app>/ that holds user data
    /// (configs, DB files). Used by "Open data folder".
    pub persist_subdir: Option<&'static str>,
}

pub const KNOWN: &[KnownService] = &[
    KnownService {
        key: "redis",
        scoop_app: "redis",
        display: "Redis",
        category: Category::Cache,
        bin_relpath: "redis-server.exe",
        default_port: Some(6379),
        persist_subdir: Some("data"),
    },
    KnownService {
        key: "postgresql",
        scoop_app: "postgresql",
        display: "PostgreSQL",
        category: Category::Database,
        bin_relpath: "bin/postgres.exe",
        default_port: Some(5432),
        persist_subdir: Some("data"),
    },
    KnownService {
        key: "mysql",
        scoop_app: "mysql",
        display: "MySQL",
        category: Category::Database,
        bin_relpath: "bin/mysqld.exe",
        default_port: Some(3306),
        persist_subdir: Some("data"),
    },
    KnownService {
        key: "mariadb",
        scoop_app: "mariadb",
        display: "MariaDB",
        category: Category::Database,
        bin_relpath: "bin/mariadbd.exe",
        default_port: Some(3306),
        persist_subdir: Some("data"),
    },
    KnownService {
        key: "mongodb",
        scoop_app: "mongodb",
        display: "MongoDB",
        category: Category::Database,
        bin_relpath: "bin/mongod.exe",
        default_port: Some(27017),
        persist_subdir: Some("data"),
    },
    KnownService {
        key: "nginx",
        scoop_app: "nginx",
        display: "Nginx",
        category: Category::WebServer,
        bin_relpath: "nginx.exe",
        default_port: Some(80),
        persist_subdir: Some("conf"),
    },
    KnownService {
        key: "caddy",
        scoop_app: "caddy",
        display: "Caddy",
        category: Category::WebServer,
        bin_relpath: "caddy.exe",
        default_port: Some(2019),
        persist_subdir: None,
    },
];

pub fn lookup(key: &str) -> Option<&'static KnownService> {
    KNOWN.iter().find(|s| s.key == key)
}

/// Where the service binary lives on disk.
pub fn bin_path(svc: &KnownService, scoop_root: &Path) -> PathBuf {
    scoop_root
        .join("apps")
        .join(svc.scoop_app)
        .join("current")
        .join(svc.bin_relpath)
}

/// Where the service should `cd` to before launching. Most services resolve
/// configs relative to cwd, so we default to the install dir.
pub fn working_dir(svc: &KnownService, scoop_root: &Path) -> PathBuf {
    scoop_root
        .join("apps")
        .join(svc.scoop_app)
        .join("current")
}

/// Persist dir where DBs and user-edited configs live. None if the service
/// doesn't use one.
pub fn persist_dir(svc: &KnownService, scoop_root: &Path) -> Option<PathBuf> {
    svc.persist_subdir
        .map(|_| scoop_root.join("persist").join(svc.scoop_app))
}

/// Service-specific launch args. Kept as a free function so adding a new
/// service is just one match arm.
pub fn launch_args(svc: &KnownService, scoop_root: &Path) -> Vec<String> {
    match svc.key {
        "postgresql" => {
            let data = scoop_root
                .join("persist")
                .join("postgresql")
                .join("data");
            vec!["-D".into(), data.display().to_string()]
        }
        "mongodb" => {
            let data = scoop_root.join("persist").join("mongodb").join("data");
            vec!["--dbpath".into(), data.display().to_string()]
        }
        "mysql" | "mariadb" => vec!["--console".into()],
        "caddy" => vec!["run".into()],
        _ => vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_unique() {
        let mut keys: Vec<&str> = KNOWN.iter().map(|s| s.key).collect();
        keys.sort();
        let original_len = keys.len();
        keys.dedup();
        assert_eq!(keys.len(), original_len, "duplicate service key");
    }

    #[test]
    fn lookup_works() {
        assert!(lookup("redis").is_some());
        assert!(lookup("postgresql").is_some());
        assert!(lookup("does-not-exist").is_none());
    }

    #[test]
    fn launch_args_for_postgres_reference_data_dir() {
        let svc = lookup("postgresql").unwrap();
        let args = launch_args(svc, Path::new("C:\\fake\\scoop"));
        assert_eq!(args[0], "-D");
        assert!(args[1].ends_with("data"));
    }
}
