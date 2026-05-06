//! Scoop catalog: walks bucket manifests and the installed-apps tree, exposes
//! a normalized list of `AppEntry` values for the UI.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use rayon::prelude::*;
use serde::Serialize;
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
    })
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

// ─────────────────────────────────────────── filter + stats ────────────────

pub fn filter(
    catalog: &[AppEntry],
    query: Option<&str>,
    bucket: Option<&str>,
    installed_only: bool,
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

    if let Some(q) = q {
        // Rank: name-prefix > name-contains > description-contains
        hits.sort_by_key(|a| {
            let n = a.name.to_lowercase();
            if n.starts_with(&q) {
                0
            } else if n.contains(&q) {
                1
            } else {
                2
            }
        });
    }

    hits.into_iter().take(cap).cloned().collect()
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
    inner: parking_lot::Mutex<Option<Vec<AppEntry>>>,
}

impl CatalogCache {
    /// Borrow the catalog, building it on first access. Returns an empty Vec
    /// if Scoop is not installed.
    pub fn ensure(&self) -> Vec<AppEntry> {
        let mut guard = self.inner.lock();
        if let Some(c) = guard.as_ref() {
            return c.clone();
        }
        let catalog = match scoop_root() {
            Some(root) => build_catalog(&root),
            None => Vec::new(),
        };
        *guard = Some(catalog.clone());
        catalog
    }

    pub fn refresh(&self) -> Vec<AppEntry> {
        let catalog = match scoop_root() {
            Some(root) => build_catalog(&root),
            None => Vec::new(),
        };
        *self.inner.lock() = Some(catalog.clone());
        catalog
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

    #[test]
    fn filter_ranks_name_prefix_higher_than_description() {
        let entries = vec![
            AppEntry {
                name: "alpha".into(),
                bucket: "main".into(),
                version: "1".into(),
                description: Some("redis client".into()),
                homepage: None,
                license: None,
                depends: vec![],
                suggest: vec![],
                bins: vec![],
                supports_arch: vec!["64bit".into()],
                installed: None,
            },
            AppEntry {
                name: "redis".into(),
                bucket: "main".into(),
                version: "7".into(),
                description: Some("server".into()),
                homepage: None,
                license: None,
                depends: vec![],
                suggest: vec![],
                bins: vec![],
                supports_arch: vec!["64bit".into()],
                installed: None,
            },
        ];
        let result = filter(&entries, Some("redis"), None, false, 10);
        assert_eq!(result[0].name, "redis");
    }
}
