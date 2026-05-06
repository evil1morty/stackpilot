//! Curated stack presets — one-click "install + start" bundles modeled on
//! real-world dev stacks (Laravel Sail, Django/Rails backends, Mastodon,
//! Supabase, MERN, classic LAMP). Apps must reference a Scoop manifest
//! (any bucket) and `auto_start` keys must match `known_services::KNOWN`.

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
        key: "lamp-classic",
        name: "LAMP Classic",
        description: "Apache + MySQL + Redis. The PHP/WordPress/Drupal default since 2003.",
        apps: &["apache", "mysql", "redis"],
        auto_start: &["apache", "mysql", "redis"],
    },
    Preset {
        key: "lemp",
        name: "LEMP",
        description: "Nginx + MariaDB + Redis. Modern PHP stack with the faster web server.",
        apps: &["nginx", "mariadb", "redis"],
        auto_start: &["nginx", "mariadb", "redis"],
    },
    Preset {
        key: "laravel-sail",
        name: "Laravel Sail",
        description: "Nginx + MySQL + Redis + Meilisearch — Laravel's official dev stack with Scout search.",
        apps: &["nginx", "mysql", "redis", "meilisearch"],
        auto_start: &["nginx", "mysql", "redis", "meilisearch"],
    },
    Preset {
        key: "django-rails",
        name: "Django / Rails",
        description: "Nginx + PostgreSQL + Redis. The default backend stack for Django, Rails, Phoenix.",
        apps: &["nginx", "postgresql", "redis"],
        auto_start: &["nginx", "postgresql", "redis"],
    },
    Preset {
        key: "mern",
        name: "MERN",
        description: "Nginx + MongoDB + Redis. Mongo, Express, React, Node — minus the JS code.",
        apps: &["nginx", "mongodb", "redis"],
        auto_start: &["nginx", "mongodb", "redis"],
    },
    Preset {
        key: "mastodon-lite",
        name: "Mastodon Lite",
        description: "Postgres + Redis + Meilisearch. Search-heavy social — same shape as Mastodon, Discourse, Lemmy.",
        apps: &["postgresql", "redis", "meilisearch"],
        auto_start: &["postgresql", "redis", "meilisearch"],
    },
    Preset {
        key: "supabase-stub",
        name: "Supabase Stub",
        description: "Postgres + MinIO. Local Backend-as-a-Service: SQL + S3-compatible storage.",
        apps: &["postgresql", "minio"],
        auto_start: &["postgresql", "minio"],
    },
    Preset {
        key: "caddy-lab",
        name: "Caddy Lab",
        description: "Caddy + PostgreSQL + Redis. Modern, low-config, auto-HTTPS web stack.",
        apps: &["caddy", "postgresql", "redis"],
        auto_start: &["caddy", "postgresql", "redis"],
    },
    Preset {
        key: "iot-edge",
        name: "IoT Edge",
        description: "Mosquitto MQTT + Postgres. Telemetry broker plus storage for time-series data.",
        apps: &["mosquitto", "postgresql"],
        auto_start: &["mosquitto", "postgresql"],
    },
    Preset {
        key: "wordpress",
        name: "WordPress",
        description: "Apache + MySQL + Memcached. Object-cache-tuned WP host that doesn't need Redis.",
        apps: &["apache", "mysql", "memcached"],
        auto_start: &["apache", "mysql", "memcached"],
    },
    Preset {
        key: "cache-only",
        name: "Cache Only",
        description: "Just Redis. For the times you only need a quick local key-value store.",
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
