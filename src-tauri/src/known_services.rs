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
    // ── Caches ─────────────────────────────────────────────────────────
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
        key: "memcached",
        scoop_app: "memcached",
        display: "Memcached",
        category: Category::Cache,
        bin_relpath: "memcached.exe",
        default_port: Some(11211),
        persist_subdir: None,
    },
    // ── SQL databases ──────────────────────────────────────────────────
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
    // ── NoSQL ──────────────────────────────────────────────────────────
    KnownService {
        key: "mongodb",
        scoop_app: "mongodb",
        display: "MongoDB",
        category: Category::Database,
        bin_relpath: "bin/mongod.exe",
        default_port: Some(27017),
        persist_subdir: Some("data"),
    },
    // ── Web servers ────────────────────────────────────────────────────
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
        key: "apache",
        scoop_app: "apache",
        display: "Apache HTTPD",
        category: Category::WebServer,
        bin_relpath: "bin/httpd.exe",
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
    // ── Search ─────────────────────────────────────────────────────────
    KnownService {
        key: "meilisearch",
        scoop_app: "meilisearch",
        display: "Meilisearch",
        category: Category::Search,
        bin_relpath: "meilisearch.exe",
        default_port: Some(7700),
        persist_subdir: Some("data.ms"),
    },
    // ── Storage ────────────────────────────────────────────────────────
    KnownService {
        key: "minio",
        scoop_app: "minio",
        display: "MinIO",
        category: Category::Storage,
        bin_relpath: "minio.exe",
        default_port: Some(9000),
        persist_subdir: Some("data"),
    },
    // ── Messaging ──────────────────────────────────────────────────────
    KnownService {
        key: "mosquitto",
        scoop_app: "mosquitto",
        display: "Mosquitto MQTT",
        category: Category::MessageQueue,
        bin_relpath: "mosquitto.exe",
        default_port: Some(1883),
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
    let persist = scoop_root.join("persist").join(svc.scoop_app);
    match svc.key {
        "postgresql" => {
            vec!["-D".into(), persist.join("data").display().to_string()]
        }
        "mongodb" => {
            vec!["--dbpath".into(), persist.join("data").display().to_string()]
        }
        "mysql" | "mariadb" => vec!["--console".into()],
        "caddy" => vec!["run".into()],
        "minio" => vec![
            "server".into(),
            persist.join("data").display().to_string(),
            "--console-address".into(),
            ":9001".into(),
        ],
        "memcached" => vec!["-l".into(), "127.0.0.1".into()],
        "meilisearch" => vec![
            "--http-addr".into(),
            "127.0.0.1:7700".into(),
            "--db-path".into(),
            persist.join("data.ms").display().to_string(),
            "--no-analytics".into(),
        ],
        "mosquitto" => {
            let conf = persist.join("mosquitto.conf");
            if conf.exists() {
                vec!["-c".into(), conf.display().to_string()]
            } else {
                vec![]
            }
        }
        _ => vec![],
    }
}

/// A service that needs a one-time initialization step (initdb, mysql
/// --initialize, …) returns Some((bin, args)) when its data dir is empty.
/// Returning None means "ready to start as-is."
pub fn init_step(svc: &KnownService, scoop_root: &Path) -> Option<(PathBuf, Vec<String>)> {
    let install = scoop_root.join("apps").join(svc.scoop_app).join("current");
    let persist = scoop_root.join("persist").join(svc.scoop_app);
    match svc.key {
        "postgresql" => {
            let data = persist.join("data");
            // initdb writes PG_VERSION when complete; missing = uninitialized.
            if data.join("PG_VERSION").exists() {
                return None;
            }
            let bin = install.join("bin").join("initdb.exe");
            let args = vec![
                "-D".into(),
                data.display().to_string(),
                "-U".into(),
                "postgres".into(),
                "-E".into(),
                "UTF8".into(),
                "--locale=C".into(),
            ];
            Some((bin, args))
        }
        "mysql" | "mariadb" => {
            let data = persist.join("data");
            // mysql writes auto.cnf; mariadb writes mysql/host_*.frm. Use a
            // looser signal: any file inside data/ counts as initialized.
            if has_any_file(&data) {
                return None;
            }
            let bin_name = if svc.key == "mysql" { "mysqld.exe" } else { "mariadbd.exe" };
            let bin = install.join("bin").join(bin_name);
            let args = vec![
                "--initialize-insecure".into(),
                format!("--basedir={}", install.display()),
                format!("--datadir={}", data.display()),
            ];
            Some((bin, args))
        }
        "mongodb" => {
            // mongod creates its own data dir, but it must exist as a
            // writable path. Treat "no dbpath dir" as a trivial init step
            // (a single mkdir, surfaced through the same path so the user
            // sees it in logs).
            let data = persist.join("data");
            if data.exists() {
                return None;
            }
            let _ = std::fs::create_dir_all(&data);
            None
        }
        "minio" | "meilisearch" => {
            // Both services create their own data files but require the
            // parent dir to exist. Pre-create silently.
            let data = match svc.key {
                "meilisearch" => persist.join("data.ms"),
                _ => persist.join("data"),
            };
            if !data.exists() {
                let _ = std::fs::create_dir_all(&data);
            }
            None
        }
        _ => None,
    }
}

fn has_any_file(dir: &Path) -> bool {
    std::fs::read_dir(dir)
        .ok()
        .map(|mut rd| rd.next().is_some())
        .unwrap_or(false)
}

/// Editable config files Stackpilot knows about for a given service.
/// Resolved against `<scoop_root>/persist/<app>/` and the install dir.
/// Filter to those that actually exist before showing them in the UI.
pub fn config_files(svc: &KnownService, scoop_root: &Path) -> Vec<ConfigFile> {
    let install = scoop_root.join("apps").join(svc.scoop_app).join("current");
    let persist = scoop_root.join("persist").join(svc.scoop_app);
    match svc.key {
        "redis" => vec![
            ConfigFile::new(persist.join("redis.windows.conf"), "Redis", "ini"),
            ConfigFile::new(persist.join("redis.conf"), "Redis", "ini"),
            ConfigFile::new(install.join("redis.windows.conf"), "Redis (default)", "ini"),
        ],
        "postgresql" => vec![
            ConfigFile::new(persist.join("data").join("postgresql.conf"), "PostgreSQL", "ini"),
            ConfigFile::new(persist.join("data").join("pg_hba.conf"), "Auth (pg_hba)", "conf"),
            ConfigFile::new(persist.join("data").join("pg_ident.conf"), "Ident", "conf"),
        ],
        "mysql" | "mariadb" => vec![
            ConfigFile::new(install.join("my.ini"), "Server config", "ini"),
            ConfigFile::new(persist.join("my.cnf"), "User config", "ini"),
        ],
        "nginx" => vec![
            ConfigFile::new(persist.join("conf").join("nginx.conf"), "Nginx", "nginx"),
            ConfigFile::new(persist.join("conf").join("mime.types"), "MIME types", "conf"),
        ],
        "apache" => vec![
            ConfigFile::new(persist.join("conf").join("httpd.conf"), "httpd", "apache"),
            ConfigFile::new(persist.join("conf").join("extra").join("httpd-vhosts.conf"), "Vhosts", "apache"),
        ],
        "caddy" => vec![ConfigFile::new(persist.join("Caddyfile"), "Caddyfile", "caddy")],
        "mosquitto" => vec![ConfigFile::new(persist.join("mosquitto.conf"), "Broker config", "ini")],
        _ => vec![],
    }
}

/// Single editable config file.
#[derive(Clone, Debug)]
pub struct ConfigFile {
    pub path: PathBuf,
    pub label: &'static str,
    pub language: &'static str,
}

impl ConfigFile {
    fn new(path: PathBuf, label: &'static str, language: &'static str) -> Self {
        Self {
            path,
            label,
            language,
        }
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

    #[test]
    fn init_step_for_postgres_when_data_dir_missing() {
        let svc = lookup("postgresql").unwrap();
        let tmp = std::env::temp_dir()
            .join(format!("stackpilot-init-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let step = init_step(svc, &tmp);
        assert!(step.is_some(), "expected initdb step for empty data dir");
        let (bin, args) = step.unwrap();
        assert!(bin.to_string_lossy().ends_with("initdb.exe"));
        assert!(args.contains(&"-U".to_string()));
        assert!(args.contains(&"postgres".to_string()));
    }

    #[test]
    fn init_step_returns_none_for_initialized_postgres() {
        let svc = lookup("postgresql").unwrap();
        let tmp = std::env::temp_dir()
            .join(format!("stackpilot-init-test2-{}", std::process::id()));
        let data = tmp.join("persist").join("postgresql").join("data");
        std::fs::create_dir_all(&data).unwrap();
        std::fs::write(data.join("PG_VERSION"), "16").unwrap();

        let step = init_step(svc, &tmp);
        assert!(step.is_none(), "PG_VERSION present should skip init");

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn init_step_for_redis_is_none() {
        let svc = lookup("redis").unwrap();
        let step = init_step(svc, Path::new("C:\\fake\\scoop"));
        assert!(step.is_none(), "redis needs no init");
    }
}
