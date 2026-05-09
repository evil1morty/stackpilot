//! Scoop catalog: walks bucket manifests and the installed-apps tree, exposes
//! a normalized list of `AppEntry` values for the UI.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use walkdir::WalkDir;

use crate::scoop::scoop_root;

// ─────────────────────────────────────────── public types ──────────────────

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AppEntry {
    pub name: String,
    pub bucket: String,
    pub version: String,
    pub description: Option<String>,
    pub homepage: Option<String>,
    pub license: Option<String>,
    pub depends: Vec<String>,
    pub suggest: Vec<String>,
    pub bins: Vec<String>,
    pub supports_arch: Vec<String>,
    pub installed: Option<InstalledInfo>,
    /// ISO 8601 date of the last manifest commit. Populated by online search;
    /// `None` for results read off the local disk (we don't track mtime).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub committed: Option<String>,
    /// Bucket repo URL (e.g. `https://github.com/ScoopInstaller/Main`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repository: Option<String>,
    /// GitHub stars on the bucket repo. Online-only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repository_stars: Option<u32>,
    /// Match highlight fragments keyed by field name (`<mark>` wrapped).
    #[serde(default, skip_serializing_if = "std::collections::HashMap::is_empty")]
    pub highlights: std::collections::HashMap<String, Vec<String>>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct InstalledInfo {
    pub version: String,
    pub bucket: Option<String>,
    pub architecture: Option<String>,
    pub hold: bool,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BucketSummary {
    pub name: String,
    pub count: usize,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CatalogStats {
    pub total: usize,
    pub installed: usize,
    pub buckets: Vec<BucketSummary>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ScoopStatus {
    pub installed: bool,
    pub root: Option<String>,
    pub buckets: Vec<String>,
}

// ─────────────────────────────────────────── walking ───────────────────────

/// Walk every bucket and produce a normalized catalog. Parses manifests in
/// parallel via rayon — Windows Defender adds 5–15 ms per small-file read, so
/// going wide is the difference between 1 s and 60+ s on a fresh install.
pub fn build_catalog(root: &Path) -> Vec<AppEntry> {
    let installed = installed_index(root);

    let buckets_dir = root.join("buckets");
    let Ok(rd) = fs::read_dir(&buckets_dir) else {
        return Vec::new();
    };

    // Pass 1 (cheap): enumerate every manifest path, tagged with its bucket.
    let mut tasks: Vec<(PathBuf, String, String)> = Vec::with_capacity(4096);
    for entry in rd.flatten() {
        let bucket_path = entry.path();
        if !bucket_path.is_dir() {
            continue;
        }
        let Some(bucket_name) = bucket_path
            .file_name()
            .and_then(|s| s.to_str())
            .map(str::to_string)
        else {
            continue;
        };

        // Modern layout: <bucket>/bucket/*.json. Legacy: <bucket>/*.json.
        let manifest_dir = bucket_path.join("bucket");
        let scan_dir = if manifest_dir.is_dir() {
            manifest_dir
        } else {
            bucket_path.clone()
        };

        for f in WalkDir::new(&scan_dir)
            .max_depth(1)
            .into_iter()
            .flatten()
            .filter(|e| e.file_type().is_file())
        {
            let path = f.path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            let Some(stem) = path
                .file_stem()
                .and_then(|s| s.to_str())
                .map(str::to_string)
            else {
                continue;
            };
            tasks.push((path.to_path_buf(), bucket_name.clone(), stem));
        }
    }

    // Pass 2 (expensive): read + parse in parallel.
    let mut out: Vec<AppEntry> = tasks
        .par_iter()
        .filter_map(|(path, bucket, stem)| parse_manifest(path, bucket, stem, &installed))
        .collect();

    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out
}

// ─────────────────────────────────────────── installed map ─────────────────

/// Map of installed-app-name → InstalledInfo (lowercase keys).
fn installed_index(root: &Path) -> HashMap<String, InstalledInfo> {
    let mut map = HashMap::new();
    let apps_dir = root.join("apps");
    let Ok(rd) = fs::read_dir(&apps_dir) else {
        return map;
    };

    for entry in rd.flatten() {
        let p = entry.path();
        if !p.is_dir() {
            continue;
        }
        let Some(name) = p.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        if name.eq_ignore_ascii_case("scoop") {
            continue; // Scoop manages itself; not surfaced in the catalog.
        }
        let current = p.join("current");
        if !current.exists() {
            continue;
        }

        let install_json = current.join("install.json");
        let manifest_json = current.join("manifest.json");

        let mut info = InstalledInfo {
            version: read_version(&manifest_json).unwrap_or_default(),
            bucket: None,
            architecture: None,
            hold: false,
        };

        if let Ok(text) = fs::read_to_string(&install_json) {
            if let Ok(v) = serde_json::from_str::<Value>(&text) {
                info.bucket = v.get("bucket").and_then(|x| x.as_str()).map(String::from);
                info.architecture = v
                    .get("architecture")
                    .and_then(|x| x.as_str())
                    .map(String::from);
                info.hold = v.get("hold").and_then(|x| x.as_bool()).unwrap_or(false);
            }
        }

        map.insert(name.to_lowercase(), info);
    }

    map
}

fn read_version(path: &Path) -> Option<String> {
    let text = fs::read_to_string(path).ok()?;
    let v: Value = serde_json::from_str(&text).ok()?;
    v.get("version").and_then(|x| x.as_str()).map(String::from)
}

// ─────────────────────────────────────────── manifest parse ────────────────

fn parse_manifest(
    path: &Path,
    bucket: &str,
    stem: &str,
    installed: &HashMap<String, InstalledInfo>,
) -> Option<AppEntry> {
    let text = fs::read_to_string(path).ok()?;
    let v: Value = serde_json::from_str(&text).ok()?;

    let version = v
        .get("version")
        .and_then(|x| x.as_str())
        .unwrap_or("?")
        .to_string();

    let description = v
        .get("description")
        .and_then(|x| x.as_str())
        .map(|s| s.trim().to_string());
    let homepage = v
        .get("homepage")
        .and_then(|x| x.as_str())
        .map(String::from);

    let license = match v.get("license") {
        Some(Value::String(s)) => Some(s.clone()),
        Some(Value::Object(o)) => o
            .get("identifier")
            .and_then(|x| x.as_str())
            .map(String::from),
        _ => None,
    };

    let depends = string_list(v.get("depends"));
    let suggest = suggest_list(v.get("suggest"));
    let bins = bin_list(v.get("bin"));
    let supports_arch = supports_arch(&v);

    Some(AppEntry {
        name: stem.to_string(),
        bucket: bucket.to_string(),
        version,
        description,
        homepage,
        license,
        depends,
        suggest,
        bins,
        supports_arch,
        installed: installed.get(&stem.to_lowercase()).cloned(),
        committed: None,
        repository: None,
        repository_stars: None,
        highlights: HashMap::new(),
    })
}

/// Used by online search to enrich API results with installed-state probed
/// fresh from disk (no full catalog walk).
pub fn lookup_installed(name: &str) -> Option<InstalledInfo> {
    let root = scoop_root()?;
    installed_index(&root).remove(&name.to_lowercase())
}

/// Normalize a value that is either a string, an array of strings, or absent.
fn string_list(v: Option<&Value>) -> Vec<String> {
    match v {
        Some(Value::String(s)) => vec![s.clone()],
        Some(Value::Array(arr)) => arr
            .iter()
            .filter_map(|x| x.as_str().map(String::from))
            .collect(),
        _ => Vec::new(),
    }
}

/// `suggest` is `{key: string|string[]}` per Scoop schema. Flatten to a
/// deduplicated list of suggested app names.
fn suggest_list(v: Option<&Value>) -> Vec<String> {
    let Some(Value::Object(map)) = v else {
        return Vec::new();
    };
    let mut out: Vec<String> = Vec::new();
    for (_k, val) in map {
        for s in string_list(Some(val)) {
            // Keep only the bare app name (Scoop accepts "bucket/name" too).
            let bare = s.rsplit('/').next().unwrap_or(&s).to_string();
            if !out.contains(&bare) {
                out.push(bare);
            }
        }
    }
    out
}

/// `bin` may be a string, an array of strings, or an array containing
/// arrays of `[path, alias?, args?]`. We surface just the binary basenames.
fn bin_list(v: Option<&Value>) -> Vec<String> {
    fn from_path(s: &str) -> String {
        // strip leading directories, keep just the binary name
        let last = s.rsplit(['/', '\\']).next().unwrap_or(s);
        last.to_string()
    }
    match v {
        Some(Value::String(s)) => vec![from_path(s)],
        Some(Value::Array(arr)) => arr
            .iter()
            .filter_map(|item| match item {
                Value::String(s) => Some(from_path(s)),
                Value::Array(inner) => inner
                    .first()
                    .and_then(|x| x.as_str())
                    .map(from_path),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// Architectures supported by this manifest.
fn supports_arch(v: &Value) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(Value::Object(map)) = v.get("architecture") {
        for k in ["64bit", "32bit", "arm64"] {
            if map.contains_key(k) {
                out.push(k.to_string());
            }
        }
    } else if v.get("url").is_some() {
        // arch-agnostic — treat as supporting everything common.
        out.push("64bit".into());
    }
    if out.is_empty() {
        out.push("64bit".into()); // sensible default for filtering UX
    }
    out
}

// ─────────────────────────────────────────── filter + sort ────────────────

#[derive(Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SortBy {
    /// Query-relevance first (name-prefix > name-contains > desc-contains),
    /// with bucket priority as a tiebreaker. The default.
    #[default]
    BestMatch,
    /// Curated ordering. Online: bucket repository stars desc. Offline:
    /// bucket priority + metadata richness heuristic.
    Popular,
    /// Most recently committed manifests first. Online-only signal.
    Recent,
    /// Pure alphabetical, case-insensitive.
    Name,
}

/// Lower = ranks earlier. main is canonically the curated bucket.
fn bucket_priority(bucket: &str) -> u8 {
    match bucket {
        "main" => 0,
        "extras" => 1,
        "versions" => 2,
        _ => 3,
    }
}

/// Tuple of secondary tiebreakers used for both Popular and BestMatch.
/// Lower wins.
fn quality_score(a: &AppEntry) -> (u8, u8) {
    let no_desc = if a.description.as_deref().map(str::trim).unwrap_or("").is_empty() {
        1
    } else {
        0
    };
    let no_license = if a.license.is_none() { 1 } else { 0 };
    (no_desc, no_license)
}

pub fn filter(
    catalog: &[AppEntry],
    query: Option<&str>,
    bucket: Option<&str>,
    installed_only: bool,
    sort: SortBy,
    cap: usize,
) -> Vec<AppEntry> {
    let q = query.map(|s| s.trim().to_lowercase()).filter(|s| !s.is_empty());

    let mut hits: Vec<&AppEntry> = catalog
        .iter()
        .filter(|a| {
            if installed_only && a.installed.is_none() {
                return false;
            }
            if let Some(b) = bucket {
                if a.bucket != b {
                    return false;
                }
            }
            if let Some(q) = &q {
                let name = a.name.to_lowercase();
                let desc = a
                    .description
                    .as_deref()
                    .map(str::to_lowercase)
                    .unwrap_or_default();
                if !name.contains(q) && !desc.contains(q) {
                    return false;
                }
            }
            true
        })
        .collect();

    let q_ref = q.as_deref();
    match sort {
        SortBy::Name => {
            hits.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        }
        SortBy::Popular => {
            hits.sort_by(|a, b| {
                let key_a = (
                    bucket_priority(&a.bucket),
                    quality_score(a),
                    a.name.to_lowercase(),
                );
                let key_b = (
                    bucket_priority(&b.bucket),
                    quality_score(b),
                    b.name.to_lowercase(),
                );
                key_a.cmp(&key_b)
            });
        }
        SortBy::BestMatch | SortBy::Recent => {
            // Recent has no offline signal (we don't store mtime), so it
            // falls back to BestMatch ordering when invoked locally.
            hits.sort_by(|a, b| {
                let rank_a = match_rank(a, q_ref);
                let rank_b = match_rank(b, q_ref);
                let key_a = (
                    rank_a,
                    bucket_priority(&a.bucket),
                    quality_score(a),
                    a.name.to_lowercase(),
                );
                let key_b = (
                    rank_b,
                    bucket_priority(&b.bucket),
                    quality_score(b),
                    b.name.to_lowercase(),
                );
                key_a.cmp(&key_b)
            });
        }
    }

    hits.into_iter().take(cap).cloned().collect()
}

/// Lower = better match. 0 = name == query, 1 = name starts with query,
/// 2 = name contains, 3 = description contains, 4 = no query (neutral).
fn match_rank(a: &AppEntry, q: Option<&str>) -> u8 {
    let Some(q) = q else {
        return 4;
    };
    let name = a.name.to_lowercase();
    if name == q {
        0
    } else if name.starts_with(q) {
        1
    } else if name.contains(q) {
        2
    } else {
        3
    }
}

pub fn stats(catalog: &[AppEntry]) -> CatalogStats {
    let mut by_bucket: HashMap<String, usize> = HashMap::new();
    let mut installed = 0usize;
    for a in catalog {
        *by_bucket.entry(a.bucket.clone()).or_insert(0) += 1;
        if a.installed.is_some() {
            installed += 1;
        }
    }
    let mut buckets: Vec<BucketSummary> = by_bucket
        .into_iter()
        .map(|(name, count)| BucketSummary { name, count })
        .collect();
    buckets.sort_by(|a, b| b.count.cmp(&a.count));

    CatalogStats {
        total: catalog.len(),
        installed,
        buckets,
    }
}

// ─────────────────────────────────────────── scoop status ──────────────────

pub fn current_status() -> ScoopStatus {
    let Some(root) = scoop_root() else {
        return ScoopStatus {
            installed: false,
            root: None,
            buckets: Vec::new(),
        };
    };

    let buckets_dir = root.join("buckets");
    let buckets = fs::read_dir(&buckets_dir)
        .map(|rd| {
            rd.flatten()
                .filter(|e| e.path().is_dir())
                .filter_map(|e| e.file_name().into_string().ok())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    ScoopStatus {
        installed: true,
        root: Some(root.display().to_string()),
        buckets,
    }
}

// ─────────────────────────────────────────── cache ─────────────────────────

#[derive(Default)]
pub struct CatalogCache {
    inner: parking_lot::Mutex<Option<Arc<Vec<AppEntry>>>>,
}

impl CatalogCache {
    /// Borrow the catalog, building it on first access. Returns an empty Vec
    /// if Scoop is not installed. The returned `Arc` lets callers iterate
    /// without copying the underlying ~3,800-entry vector.
    pub fn ensure(&self) -> Arc<Vec<AppEntry>> {
        {
            let guard = self.inner.lock();
            if let Some(c) = guard.as_ref() {
                return Arc::clone(c);
            }
        }
        let catalog = match scoop_root() {
            Some(root) => build_catalog(&root),
            None => Vec::new(),
        };
        let arc = Arc::new(catalog);
        *self.inner.lock() = Some(Arc::clone(&arc));
        arc
    }

    pub fn refresh(&self) -> Arc<Vec<AppEntry>> {
        let catalog = match scoop_root() {
            Some(root) => build_catalog(&root),
            None => Vec::new(),
        };
        let arc = Arc::new(catalog);
        *self.inner.lock() = Some(Arc::clone(&arc));
        arc
    }

    #[allow(dead_code)]
    pub fn clear(&self) {
        *self.inner.lock() = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn license_string_or_object() {
        let v_str = json!({"license": "MIT"});
        let v_obj = json!({"license": {"identifier": "Apache-2.0", "url": "..."}});
        let v_none = json!({});

        let parsed_str = match v_str.get("license") {
            Some(Value::String(s)) => Some(s.clone()),
            Some(Value::Object(o)) => o.get("identifier").and_then(|x| x.as_str()).map(String::from),
            _ => None,
        };
        let parsed_obj = match v_obj.get("license") {
            Some(Value::String(s)) => Some(s.clone()),
            Some(Value::Object(o)) => o.get("identifier").and_then(|x| x.as_str()).map(String::from),
            _ => None,
        };
        let parsed_none: Option<String> = match v_none.get("license") {
            Some(Value::String(s)) => Some(s.clone()),
            _ => None,
        };

        assert_eq!(parsed_str.as_deref(), Some("MIT"));
        assert_eq!(parsed_obj.as_deref(), Some("Apache-2.0"));
        assert!(parsed_none.is_none());
    }

    #[test]
    fn bin_normalization() {
        assert_eq!(bin_list(Some(&json!("bin/redis-server.exe"))), vec!["redis-server.exe"]);
        assert_eq!(
            bin_list(Some(&json!(["redis-cli.exe", "redis-server.exe"]))),
            vec!["redis-cli.exe", "redis-server.exe"]
        );
        assert_eq!(
            bin_list(Some(&json!([["bin/foo.exe", "foo", "--flag"]]))),
            vec!["foo.exe"]
        );
    }

    #[test]
    fn suggest_flatten() {
        let v = json!({"core": ["7zip", "git"], "extra": "vim"});
        let mut got = suggest_list(Some(&v));
        got.sort();
        let mut want = vec!["7zip".to_string(), "git".to_string(), "vim".to_string()];
        want.sort();
        assert_eq!(got, want);
    }

    #[test]
    fn arch_detection() {
        let v = json!({"architecture": {"64bit": {"url": "..."}, "arm64": {"url": "..."}}});
        let mut got = supports_arch(&v);
        got.sort();
        let mut want = vec!["64bit".to_string(), "arm64".to_string()];
        want.sort();
        assert_eq!(got, want);

        let v_simple = json!({"url": "..."});
        assert_eq!(supports_arch(&v_simple), vec!["64bit"]);
    }

    fn entry(name: &str, bucket: &str, desc: Option<&str>, license: Option<&str>) -> AppEntry {
        AppEntry {
            name: name.into(),
            bucket: bucket.into(),
            version: "1".into(),
            description: desc.map(String::from),
            homepage: None,
            license: license.map(String::from),
            depends: vec![],
            suggest: vec![],
            bins: vec![],
            supports_arch: vec!["64bit".into()],
            installed: None,
            committed: None,
            repository: None,
            repository_stars: None,
            highlights: HashMap::new(),
        }
    }

    #[test]
    fn best_match_ranks_exact_name_first() {
        let entries = vec![
            entry("alpha", "main", Some("redis client"), None),
            entry("redis", "main", Some("server"), None),
        ];
        let result = filter(&entries, Some("redis"), None, false, SortBy::BestMatch, 10);
        assert_eq!(result[0].name, "redis");
    }

    #[test]
    fn best_match_breaks_ties_with_bucket_priority() {
        let entries = vec![
            entry("redis-extra", "extras", Some("variant"), None),
            entry("redis-fork", "main", Some("variant"), None),
        ];
        let result = filter(&entries, Some("redis"), None, false, SortBy::BestMatch, 10);
        // Both contain "redis" as prefix; main bucket should rank first.
        assert_eq!(result[0].bucket, "main");
        assert_eq!(result[1].bucket, "extras");
    }

    #[test]
    fn popular_puts_main_bucket_first() {
        let entries = vec![
            entry("zzz", "extras", Some("desc"), Some("MIT")),
            entry("aaa", "main", None, None),
        ];
        let result = filter(&entries, None, None, false, SortBy::Popular, 10);
        assert_eq!(result[0].bucket, "main");
    }

    #[test]
    fn popular_uses_metadata_bonus_within_bucket() {
        let entries = vec![
            entry("a", "main", None, None),
            entry("b", "main", Some("has desc"), Some("MIT")),
        ];
        let result = filter(&entries, None, None, false, SortBy::Popular, 10);
        // Same bucket → richer metadata wins over alphabetical "a < b".
        assert_eq!(result[0].name, "b");
    }

    #[test]
    fn name_sort_is_pure_alphabetical() {
        let entries = vec![
            entry("zlib", "main", None, None),
            entry("apache", "extras", Some("desc"), Some("Apache-2.0")),
        ];
        let result = filter(&entries, None, None, false, SortBy::Name, 10);
        assert_eq!(result[0].name, "apache");
        assert_eq!(result[1].name, "zlib");
    }
}
