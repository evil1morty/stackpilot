//! Curated stack presets — one-click "install + start" bundles. Apps must
//! reference a Scoop manifest (any bucket) and `auto_start` keys must match
//! `known_services::KNOWN`.

pub struct Preset {
    pub key: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    /// Scoop apps to install (resolved without bucket prefix).
    pub apps: &'static [&'static str],
    /// Known-service keys to start once installs complete.
    pub auto_start: &'static [&'static str],
}

pub const PRESETS: &[Preset] = &[
    Preset {
        key: "lemp",
        name: "LEMP",
        description: "Nginx, MariaDB and Redis — the classic Linux/Mac dev stack, on Windows.",
        apps: &["nginx", "mariadb", "redis"],
        auto_start: &["nginx", "mariadb", "redis"],
    },
    Preset {
        key: "postgres-stack",
        name: "Postgres Stack",
        description: "Nginx + PostgreSQL + Redis. Solid choice for Django, Rails, or Phoenix.",
        apps: &["nginx", "postgresql", "redis"],
        auto_start: &["nginx", "postgresql", "redis"],
    },
    Preset {
        key: "mern-lite",
        name: "MERN Lite",
        description: "Nginx + MongoDB + Redis. NoSQL backend for Node and friends.",
        apps: &["nginx", "mongodb", "redis"],
        auto_start: &["nginx", "mongodb", "redis"],
    },
    Preset {
        key: "caddy-lab",
        name: "Caddy Lab",
        description: "Caddy with auto-HTTPS, PostgreSQL and Redis. Modern, low-config.",
        apps: &["caddy", "postgresql", "redis"],
        auto_start: &["caddy", "postgresql", "redis"],
    },
    Preset {
        key: "cache-only",
        name: "Cache Only",
        description: "Just Redis. For frontend devs who only need a quick local cache.",
        apps: &["redis"],
        auto_start: &["redis"],
    },
];

pub fn lookup(key: &str) -> Option<&'static Preset> {
    PRESETS.iter().find(|p| p.key == key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::known_services;

    #[test]
    fn preset_keys_unique() {
        let mut keys: Vec<&str> = PRESETS.iter().map(|p| p.key).collect();
        keys.sort();
        let len = keys.len();
        keys.dedup();
        assert_eq!(keys.len(), len, "duplicate preset key");
    }

    #[test]
    fn auto_start_targets_resolve_to_known_services() {
        for p in PRESETS {
            for svc_key in p.auto_start {
                assert!(
                    known_services::lookup(svc_key).is_some(),
                    "preset {} references unknown service {}",
                    p.key,
                    svc_key
                );
            }
        }
    }

    #[test]
    fn auto_start_apps_are_in_apps_list() {
        // Every auto-start service must have its scoop_app in the install list.
        for p in PRESETS {
            for svc_key in p.auto_start {
                let svc = known_services::lookup(svc_key).unwrap();
                assert!(
                    p.apps.contains(&svc.scoop_app),
                    "preset {}: auto_start {} requires app {} but it isn't in apps list",
                    p.key,
                    svc_key,
                    svc.scoop_app
                );
            }
        }
    }
}
