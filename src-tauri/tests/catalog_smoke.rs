//! Integration smoke test: walks the real ~/scoop install when present.
//! Skipped automatically when Scoop is not installed on the host.

use std::path::PathBuf;

#[path = "../src/scoop.rs"]
mod scoop;

#[path = "../src/catalog.rs"]
mod catalog;

#[test]
fn walks_real_scoop_install_when_present() {
    let Some(root) = real_scoop_root() else {
        eprintln!("skipping: ~/scoop not present");
        return;
    };

    let entries = catalog::build_catalog(&root);
    let stats = catalog::stats(&entries);

    assert!(stats.total > 0, "expected manifests, got {}", stats.total);
    assert!(
        !stats.buckets.is_empty(),
        "expected at least one bucket, got {:?}",
        stats.buckets
    );

    // Verify our service-app coverage: at least one of the well-known services
    // should be in the catalog.
    let service_names = ["redis", "postgresql", "mysql", "nginx", "mongodb"];
    let found: Vec<&str> = service_names
        .iter()
        .copied()
        .filter(|name| entries.iter().any(|a| a.name == *name))
        .collect();
    assert!(
        !found.is_empty(),
        "expected at least one service in catalog; checked {:?}",
        service_names
    );

    eprintln!(
        "catalog: {} apps across {} buckets ({} installed). Found services: {:?}",
        stats.total,
        stats.buckets.len(),
        stats.installed,
        found,
    );

    // Spot-check filter: searching "redis" should rank "redis" first.
    let hits = catalog::filter(
        &entries,
        Some("redis"),
        None,
        false,
        catalog::SortBy::BestMatch,
        10,
    );
    assert!(!hits.is_empty(), "redis search returned nothing");
    assert_eq!(hits[0].name, "redis", "redis should rank first by name match");

    // Spot-check shape of redis entry.
    let redis = &hits[0];
    assert!(!redis.version.is_empty());
    assert!(redis.description.is_some());
    assert!(redis.bins.iter().any(|b| b.contains("redis-server")));
}

fn real_scoop_root() -> Option<PathBuf> {
    let home = std::env::var("USERPROFILE")
        .ok()
        .or_else(|| std::env::var("HOME").ok())?;
    let p = PathBuf::from(home).join("scoop");
    if p.is_dir() { Some(p) } else { None }
}
