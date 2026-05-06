//! Live search against the public scoopsearch.search.windows.net Azure
//! Cognitive Search index — the same backend scoop.sh queries. This gets us
//! real BM25 relevance, official-repo filtering, last-commit timestamps, and
//! match highlights without maintaining our own index.
//!
//! API key is the published query-only key embedded in scoopinstaller.github.io's
//! frontend. If they ever rotate it, the call falls back to the local catalog
//! (handled by the caller).

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::catalog::SortBy;

const ENDPOINT: &str =
    "https://scoopsearch.search.windows.net/indexes/apps/docs/search?api-version=2020-06-30";
const API_KEY: &str = "DC6D2BBE65FC7313F2C52BBD2B0286ED";

// ─────────────────────────────────────────── public DTOs ──────────────────

#[derive(Debug, Clone)]
pub struct SearchOptions<'a> {
    pub query: Option<&'a str>,
    pub sort: SortBy,
    pub skip: u32,
    pub top: u32,
    pub include_community: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OnlineApp {
    pub name: String,
    pub description: Option<String>,
    pub homepage: Option<String>,
    pub license: Option<String>,
    pub version: String,
    pub repository: Option<String>,
    pub repository_stars: Option<u32>,
    pub committed: Option<String>,
    pub bucket: String,
    pub highlights: HashMap<String, Vec<String>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OnlineSearchResult {
    pub total: u64,
    pub items: Vec<OnlineApp>,
}

// ─────────────────────────────────────────── network ──────────────────────

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RequestBody {
    count: bool,
    search: String,
    search_mode: &'static str,
    filter: String,
    orderby: &'static str,
    skip: u32,
    top: u32,
    select: &'static str,
    highlight: &'static str,
    highlight_pre_tag: &'static str,
    highlight_post_tag: &'static str,
}

#[derive(Deserialize)]
struct RawResponse {
    #[serde(rename = "@odata.count", default)]
    count: u64,
    value: Vec<RawApp>,
}

#[derive(Deserialize)]
struct RawApp {
    #[serde(default, rename = "Name")]
    name: String,
    #[serde(default, rename = "Description")]
    description: Option<String>,
    #[serde(default, rename = "Homepage")]
    homepage: Option<String>,
    #[serde(default, rename = "License")]
    license: Option<String>,
    #[serde(default, rename = "Version")]
    version: String,
    #[serde(default, rename = "Metadata")]
    metadata: Option<RawMetadata>,
    #[serde(default, rename = "@search.highlights")]
    highlights: Option<HashMap<String, Vec<String>>>,
}

#[derive(Deserialize)]
struct RawMetadata {
    #[serde(default, rename = "Repository")]
    repository: Option<String>,
    #[serde(default, rename = "RepositoryStars")]
    repository_stars: Option<u32>,
    #[serde(default, rename = "Committed")]
    committed: Option<String>,
}

/// `https://github.com/ScoopInstaller/Main` → `main`. Falls back to a
/// best-effort lowercase last segment.
fn bucket_from_repo(url: &str) -> String {
    url.rsplit('/').next().unwrap_or("").to_ascii_lowercase()
}

/// Build the OData orderby clause for the requested sort.
fn orderby_for(sort: SortBy) -> &'static str {
    match sort {
        SortBy::BestMatch => {
            "search.score() desc, Metadata/OfficialRepositoryNumber desc, NameSortable asc"
        }
        SortBy::Popular => {
            "Metadata/RepositoryStars desc, Metadata/OfficialRepositoryNumber desc, NameSortable asc"
        }
        SortBy::Recent => "Metadata/Committed desc, NameSortable asc",
        SortBy::Name => "NameSortable asc",
    }
}

pub async fn search(opts: SearchOptions<'_>) -> Result<OnlineSearchResult, String> {
    let mut filter = String::from("Metadata/DuplicateOf eq null");
    if !opts.include_community {
        filter.insert_str(0, "Metadata/OfficialRepositoryNumber eq 1 and ");
    }

    // Azure Cognitive Search rejects empty `search` strings; substitute a
    // wildcard so an empty query still returns ranked results.
    let search_term = opts
        .query
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("*")
        .to_string();

    let body = RequestBody {
        count: true,
        search: search_term,
        search_mode: "all",
        filter,
        orderby: orderby_for(opts.sort),
        skip: opts.skip,
        top: opts.top.min(100),
        select: "Name,Description,Homepage,License,Version,Metadata/Repository,\
                 Metadata/RepositoryStars,Metadata/Committed,Metadata/OfficialRepository,\
                 Metadata/DuplicateOf,Metadata/OfficialRepositoryNumber",
        highlight: "Name,NamePartial,NameSuffix,Description",
        highlight_pre_tag: "<mark>",
        highlight_post_tag: "</mark>",
    };

    let resp = reqwest::Client::new()
        .post(ENDPOINT)
        .header("api-key", API_KEY)
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("scoopsearch request failed: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("scoopsearch returned HTTP {}", resp.status()));
    }

    let raw: RawResponse = resp
        .json()
        .await
        .map_err(|e| format!("scoopsearch response decode failed: {e}"))?;

    let items = raw
        .value
        .into_iter()
        .map(|r| {
            let metadata = r.metadata.unwrap_or(RawMetadata {
                repository: None,
                repository_stars: None,
                committed: None,
            });
            let bucket = metadata
                .repository
                .as_deref()
                .map(bucket_from_repo)
                .unwrap_or_default();
            OnlineApp {
                name: r.name,
                description: r.description,
                homepage: r.homepage,
                license: r.license,
                version: r.version,
                repository: metadata.repository,
                repository_stars: metadata.repository_stars,
                committed: metadata.committed,
                bucket,
                highlights: r.highlights.unwrap_or_default(),
            }
        })
        .collect();

    Ok(OnlineSearchResult {
        total: raw.count,
        items,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bucket_parsing() {
        assert_eq!(
            bucket_from_repo("https://github.com/ScoopInstaller/Main"),
            "main"
        );
        assert_eq!(
            bucket_from_repo("https://github.com/ScoopInstaller/Extras"),
            "extras"
        );
        assert_eq!(
            bucket_from_repo("https://github.com/ScoopInstaller/Versions"),
            "versions"
        );
        assert_eq!(bucket_from_repo("just-a-string"), "just-a-string");
    }

    #[test]
    fn orderby_for_each_sort() {
        for sort in [SortBy::BestMatch, SortBy::Popular, SortBy::Recent, SortBy::Name] {
            assert!(!orderby_for(sort).is_empty());
        }
    }
}
