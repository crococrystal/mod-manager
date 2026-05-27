use std::collections::HashMap;

use serde::Serialize;

use crate::mod_names::{
    hyphenated_to_spaced, is_version_or_loader_segment, mod_name_tokens, normalized_match_key,
    slug_key, spaced_camel_case, strip_filename_decorations, strip_qualifiers,
    strip_version_suffixes,
};
use crate::mods::ModEntry;
use crate::settings::Settings;

pub(crate) const PROVIDER_SEARCH_LIMIT: usize = 5;
/// Один запрос search к API поставщика (limit=5 внутри ответа).
const PROVIDER_SEARCH_QUERY_LIMIT: usize = 1;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderCandidate {
    pub id: String,
    pub slug: Option<String>,
    pub title: String,
    pub icon_url: Option<String>,
    #[serde(default)]
    pub exact_file_match: bool,
    #[serde(default)]
    pub match_score: u32,
}

#[derive(Clone, Debug)]
pub(crate) struct ModrinthVersionMatch {
    pub project_id: String,
    pub version_id: String,
}

#[derive(Clone, Debug)]
pub(crate) struct CurseForgeFileMatch {
    pub project_id: String,
    pub file_id: String,
}

#[derive(Clone, Debug)]
pub(crate) struct ProviderProject {
    pub id: String,
    pub slug: Option<String>,
    pub title: Option<String>,
    pub project_type: Option<String>,
}

pub(crate) fn http_client() -> Option<reqwest::blocking::Client> {
    reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(8))
        .user_agent("mod-manager/0.1.2")
        .build()
        .ok()
}

pub(crate) fn search_http_client() -> Option<reqwest::blocking::Client> {
    reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .user_agent("mod-manager/0.1.2")
        .build()
        .ok()
}

fn search_queries_for_api(display_name: &str) -> Vec<String> {
    search_queries_from_display_name(display_name)
        .into_iter()
        .take(PROVIDER_SEARCH_QUERY_LIMIT)
        .collect()
}

pub(crate) fn modrinth_project(
    client: &reqwest::blocking::Client,
    project_id: &str,
) -> Option<serde_json::Value> {
    client
        .get(format!("https://api.modrinth.com/v2/project/{project_id}"))
        .send()
        .ok()?
        .error_for_status()
        .ok()?
        .json()
        .ok()
}

fn push_unique_query(queries: &mut Vec<String>, candidate: &str) {
    let trimmed = candidate.trim();
    if trimmed.is_empty() {
        return;
    }
    if !queries.iter().any(|existing| existing == trimmed) {
        queries.push(trimmed.to_string());
    }
}

fn normalized_prefix_match(jar_key: &str, catalog_key: &str) -> bool {
    const MIN_LEN: usize = 8;
    const OK_EXTRA_SUFFIXES: &[&str] = &[
        "edition",
        "jeiedition",
        "lite",
        "plus",
        "extra",
        "fix",
        "api",
        "lib",
    ];
    if jar_key.len() < MIN_LEN || catalog_key.len() < MIN_LEN {
        return false;
    }
    if jar_key == catalog_key {
        return true;
    }
    if jar_key.starts_with(catalog_key) {
        return true;
    }
    if !catalog_key.starts_with(jar_key) {
        return false;
    }
    let extra = &catalog_key[jar_key.len()..];
    if extra.is_empty() {
        return true;
    }
    if extra.len() <= 12 {
        return OK_EXTRA_SUFFIXES.iter().any(|suffix| extra == *suffix);
    }
    false
}

fn search_queries_from_display_name(display_name: &str) -> Vec<String> {
    let trimmed = strip_filename_decorations(display_name);
    let clean = strip_qualifiers(&trimmed);
    let tokens = mod_name_tokens(display_name);
    let stripped = strip_version_suffixes(&trimmed);
    let spaced = tokens.join(" ");
    let underscored = tokens.join("_");
    let hyphen_spaced = if spaced.is_empty() {
        hyphenated_to_spaced(&stripped)
    } else {
        spaced.clone()
    };
    let camel = spaced_camel_case(&stripped);
    let mut queries = Vec::new();
    for candidate in [
        spaced.as_str(),
        underscored.as_str(),
        hyphen_spaced.as_str(),
        camel.as_str(),
        stripped.as_str(),
        clean.as_str(),
        trimmed.as_str(),
    ] {
        push_unique_query(&mut queries, candidate);
    }
    queries
}

pub(crate) fn candidate_match_score(display_name: &str, candidate: &ProviderCandidate) -> u32 {
    let project = ProviderProject {
        id: candidate.id.clone(),
        slug: candidate.slug.clone(),
        title: Some(candidate.title.clone()),
        project_type: Some("mod".to_string()),
    };
    if modrinth_project_matches(&project, display_name) {
        return 1000;
    }
    if provider_project_matches(&project, display_name) {
        return 800;
    }
    let stem = strip_version_suffixes(&strip_filename_decorations(display_name));
    let tokens: Vec<&str> = stem
        .split(&['-', '_', ' '][..])
        .filter(|part| part.len() >= 2 && !is_version_or_loader_segment(part))
        .collect();
    if tokens.is_empty() {
        return 0;
    }
    let hay = format!(
        "{} {}",
        candidate.title,
        candidate.slug.as_deref().unwrap_or_default()
    )
    .to_ascii_lowercase();
    let matched = tokens
        .iter()
        .filter(|token| hay.contains(&token.to_ascii_lowercase()))
        .count();
    (matched as u32) * 50
}

fn sort_candidates_by_relevance(display_name: &str, candidates: &mut [ProviderCandidate]) {
    for candidate in candidates.iter_mut() {
        candidate.match_score = candidate_match_score(display_name, candidate);
    }
    candidates.sort_by(|left, right| right.match_score.cmp(&left.match_score));
}

fn match_keys_for_display_name(display_name: &str) -> (Vec<String>, Vec<String>) {
    let trimmed = strip_filename_decorations(display_name);
    let clean = strip_qualifiers(&trimmed);
    let stripped = strip_version_suffixes(&trimmed);
    let mut name_keys = Vec::new();
    let mut slug_keys = Vec::new();
    for candidate in [trimmed.as_str(), clean.as_str(), stripped.as_str()] {
        let normalized = normalized_match_key(candidate);
        if !normalized.is_empty() && !name_keys.contains(&normalized) {
            name_keys.push(normalized);
        }
        let slug = slug_key(candidate);
        if !slug.is_empty() && !slug_keys.contains(&slug) {
            slug_keys.push(slug);
        }
    }
    let stem = normalized_match_key(&stripped);
    if !stem.is_empty() && !name_keys.contains(&stem) {
        name_keys.push(stem);
    }
    (name_keys, slug_keys)
}

pub(crate) fn provider_project_matches(project: &ProviderProject, display_name: &str) -> bool {
    let (name_keys, slug_keys) = match_keys_for_display_name(display_name);
    let jar_stem = normalized_match_key(&strip_version_suffixes(&strip_filename_decorations(
        display_name,
    )));

    if let Some(title) = project.title.as_deref() {
        let title_key = normalized_match_key(title);
        if !title_key.is_empty() && name_keys.contains(&title_key) {
            return true;
        }
        if normalized_prefix_match(&jar_stem, &title_key) {
            return true;
        }
    }

    if let Some(slug) = project.slug.as_deref() {
        let slug_norm = normalized_match_key(slug);
        if !slug_norm.is_empty() && name_keys.contains(&slug_norm) {
            return true;
        }
        if normalized_prefix_match(&jar_stem, &slug_norm) {
            return true;
        }
        let slug_slug = slug_key(slug);
        if !slug_slug.is_empty() && slug_keys.contains(&slug_slug) {
            return true;
        }
    }

    false
}

pub(crate) fn modrinth_project_matches(project: &ProviderProject, display_name: &str) -> bool {
    project
        .project_type
        .as_deref()
        .is_some_and(|kind| kind == "mod")
        && provider_project_matches(project, display_name)
}

pub(crate) fn modrinth_version(
    client: &reqwest::blocking::Client,
    version_id: &str,
) -> Option<serde_json::Value> {
    client
        .get(format!("https://api.modrinth.com/v2/version/{version_id}"))
        .send()
        .ok()?
        .error_for_status()
        .ok()?
        .json()
        .ok()
}

pub(crate) fn modrinth_versions_by_sha512(
    client: &reqwest::blocking::Client,
    hashes: &[String],
) -> HashMap<String, ModrinthVersionMatch> {
    if hashes.is_empty() {
        return HashMap::new();
    }
    let Ok(payload) = client
        .post("https://api.modrinth.com/v2/version_files")
        .json(&serde_json::json!({
            "hashes": hashes,
            "algorithm": "sha512"
        }))
        .send()
        .and_then(|response| response.error_for_status())
        .and_then(|response| response.json::<serde_json::Value>())
    else {
        return HashMap::new();
    };

    let Some(items) = payload.as_object() else {
        return HashMap::new();
    };
    items
        .iter()
        .filter_map(|(hash, version)| {
            let project_id = version.get("project_id").and_then(|value| value.as_str())?;
            let version_id = version.get("id").and_then(|value| value.as_str())?;
            Some((
                hash.clone(),
                ModrinthVersionMatch {
                    project_id: project_id.to_string(),
                    version_id: version_id.to_string(),
                },
            ))
        })
        .collect()
}

pub(crate) fn modrinth_version_by_sha512(
    client: &reqwest::blocking::Client,
    hash: &str,
) -> Option<ModrinthVersionMatch> {
    let hashes = vec![hash.to_string()];
    modrinth_versions_by_sha512(client, &hashes)
        .into_values()
        .next()
}

pub(crate) fn curseforge_get(
    client: &reqwest::blocking::Client,
    api_key: &str,
    path: &str,
) -> Option<serde_json::Value> {
    if api_key.trim().is_empty() {
        return None;
    }
    client
        .get(format!("https://api.curseforge.com/v1/{path}"))
        .header("x-api-key", api_key.trim())
        .send()
        .ok()?
        .error_for_status()
        .ok()?
        .json()
        .ok()
}

pub(crate) fn curseforge_mod_info(
    client: &reqwest::blocking::Client,
    api_key: &str,
    project_id: &str,
) -> Option<ProviderProject> {
    let payload = curseforge_get(client, api_key, &format!("mods/{project_id}"))?;
    let data = payload.get("data")?;
    let id = data.get("id").and_then(|value| value.as_i64())?;
    Some(ProviderProject {
        id: id.to_string(),
        slug: data
            .get("slug")
            .and_then(|value| value.as_str())
            .map(str::to_string),
        title: data
            .get("name")
            .and_then(|value| value.as_str())
            .map(str::to_string),
        project_type: None,
    })
}

pub(crate) fn curseforge_fingerprint_matches(
    client: &reqwest::blocking::Client,
    api_key: &str,
    fingerprints: &[u32],
) -> HashMap<u32, CurseForgeFileMatch> {
    if fingerprints.is_empty() || api_key.trim().is_empty() {
        return HashMap::new();
    }
    let Ok(payload) = client
        .post("https://api.curseforge.com/v1/fingerprints/432")
        .header("x-api-key", api_key.trim())
        .json(&serde_json::json!({ "fingerprints": fingerprints }))
        .send()
        .and_then(|response| response.error_for_status())
        .and_then(|response| response.json::<serde_json::Value>())
    else {
        return HashMap::new();
    };

    let matches = payload
        .get("data")
        .and_then(|data| data.get("exactMatches"))
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default();

    matches
        .into_iter()
        .filter_map(|item| {
            let file = item.get("file")?;
            let fingerprint = file
                .get("fileFingerprint")
                .or_else(|| item.get("id"))
                .and_then(|value| value.as_u64())
                .and_then(|value| u32::try_from(value).ok())?;
            let project_id = file.get("modId").and_then(|value| value.as_i64())?;
            let file_id = file.get("id").and_then(|value| value.as_i64())?;
            Some((
                fingerprint,
                CurseForgeFileMatch {
                    project_id: project_id.to_string(),
                    file_id: file_id.to_string(),
                },
            ))
        })
        .collect()
}

pub(crate) fn curseforge_fingerprint_match(
    client: &reqwest::blocking::Client,
    api_key: &str,
    fingerprint: u32,
) -> Option<CurseForgeFileMatch> {
    curseforge_fingerprint_matches(client, api_key, &[fingerprint])
        .into_values()
        .next()
}

fn non_empty_json_string(value: Option<&serde_json::Value>) -> Option<String> {
    value
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub(crate) fn modrinth_search_icon(
    client: &reqwest::blocking::Client,
    display_name: &str,
) -> Option<String> {
    for query_text in search_queries_from_display_name(display_name) {
        if let Some(url) = modrinth_search_icon_query(client, &query_text, display_name) {
            return Some(url);
        }
    }
    None
}

fn modrinth_search_icon_query(
    client: &reqwest::blocking::Client,
    query_text: &str,
    display_name: &str,
) -> Option<String> {
    let query = urlencoding::encode(query_text.trim());
    if query.is_empty() {
        return None;
    }
    let facets = urlencoding::encode(r#"[["project_type:mod"]]"#);
    let payload = client
        .get(format!(
            "https://api.modrinth.com/v2/search?query={query}&limit=10&index=relevance&facets={facets}"
        ))
        .send()
        .ok()?
        .error_for_status()
        .ok()?
        .json::<serde_json::Value>()
        .ok()?;
    payload
        .get("hits")
        .and_then(|hits| hits.as_array())
        .and_then(|hits| {
            hits.iter().find_map(|hit| {
                let project = ProviderProject {
                    id: hit
                        .get("project_id")
                        .or_else(|| hit.get("slug"))
                        .and_then(|value| value.as_str())
                        .unwrap_or_default()
                        .to_string(),
                    slug: hit
                        .get("slug")
                        .and_then(|value| value.as_str())
                        .map(str::to_string),
                    title: hit
                        .get("title")
                        .and_then(|value| value.as_str())
                        .map(str::to_string),
                    project_type: hit
                        .get("project_type")
                        .and_then(|value| value.as_str())
                        .map(str::to_string),
                };
                modrinth_project_matches(&project, display_name)
                    .then(|| non_empty_json_string(hit.get("icon_url")))
                    .flatten()
            })
        })
}

fn modrinth_hit_to_project(hit: &serde_json::Value) -> Option<ProviderProject> {
    let id = hit
        .get("project_id")
        .or_else(|| hit.get("slug"))
        .and_then(|value| value.as_str())?;
    Some(ProviderProject {
        id: id.to_string(),
        slug: hit
            .get("slug")
            .and_then(|value| value.as_str())
            .map(str::to_string),
        title: hit
            .get("title")
            .and_then(|value| value.as_str())
            .map(str::to_string),
        project_type: hit
            .get("project_type")
            .and_then(|value| value.as_str())
            .map(str::to_string),
    })
}

fn fetch_modrinth_search_batch(
    client: &reqwest::blocking::Client,
    query_text: &str,
    limit: usize,
) -> Vec<ProviderCandidate> {
    let query = urlencoding::encode(query_text.trim());
    if query.is_empty() {
        return Vec::new();
    }
    let facets = urlencoding::encode(r#"[["project_type:mod"]]"#);
    let Ok(payload) = client
        .get(format!(
            "https://api.modrinth.com/v2/search?query={query}&limit={limit}&index=relevance&facets={facets}"
        ))
        .send()
        .and_then(|response| response.error_for_status())
        .and_then(|response| response.json::<serde_json::Value>())
    else {
        return Vec::new();
    };
    let hits = payload
        .get("hits")
        .and_then(|hits| hits.as_array())
        .cloned()
        .unwrap_or_default();
    hits.iter()
        .filter_map(|hit| {
            let project = modrinth_hit_to_project(hit)?;
            if project.project_type.as_deref() != Some("mod") {
                return None;
            }
            Some(ProviderCandidate {
                id: project.id,
                slug: project.slug,
                title: project
                    .title
                    .filter(|value| !value.is_empty())
                    .unwrap_or_else(|| "Без названия".to_string()),
                icon_url: non_empty_json_string(hit.get("icon_url")),
                exact_file_match: false,
                match_score: 0,
            })
        })
        .collect()
}

pub(crate) fn list_modrinth_candidates(
    client: &reqwest::blocking::Client,
    display_name: &str,
) -> Vec<ProviderCandidate> {
    let mut seen = std::collections::HashSet::new();
    let mut candidates = Vec::new();
    for query_text in search_queries_for_api(display_name) {
        for candidate in fetch_modrinth_search_batch(client, &query_text, PROVIDER_SEARCH_LIMIT) {
            if seen.insert(candidate.id.clone()) {
                candidates.push(candidate);
            }
        }
        if candidates.len() >= PROVIDER_SEARCH_LIMIT {
            break;
        }
    }
    sort_candidates_by_relevance(display_name, &mut candidates);
    candidates.truncate(PROVIDER_SEARCH_LIMIT);
    candidates
}

fn fetch_curseforge_search_batch(
    client: &reqwest::blocking::Client,
    api_key: &str,
    query_text: &str,
    limit: usize,
) -> Vec<ProviderCandidate> {
    if api_key.trim().is_empty() {
        return Vec::new();
    }
    let query = urlencoding::encode(query_text.trim());
    if query.is_empty() {
        return Vec::new();
    }
    let Some(payload) = curseforge_get(
        client,
        api_key,
        &format!("mods/search?gameId=432&classId=6&searchFilter={query}&pageSize={limit}"),
    ) else {
        return Vec::new();
    };
    let items = payload
        .get("data")
        .and_then(|data| data.as_array())
        .cloned()
        .unwrap_or_default();
    items
        .into_iter()
        .filter_map(|item| {
            let id = item.get("id").and_then(|value| value.as_i64())?;
            let title = item
                .get("name")
                .and_then(|value| value.as_str())
                .map(str::to_string)
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| "Без названия".to_string());
            let icon_url = item.get("logo").and_then(|logo| {
                non_empty_json_string(logo.get("thumbnailUrl"))
                    .or_else(|| non_empty_json_string(logo.get("url")))
            });
            Some(ProviderCandidate {
                id: id.to_string(),
                slug: item
                    .get("slug")
                    .and_then(|value| value.as_str())
                    .map(str::to_string),
                title,
                icon_url,
                exact_file_match: false,
                match_score: 0,
            })
        })
        .collect()
}

pub(crate) fn list_curseforge_candidates(
    client: &reqwest::blocking::Client,
    api_key: &str,
    display_name: &str,
) -> Vec<ProviderCandidate> {
    if api_key.trim().is_empty() {
        return Vec::new();
    }
    let mut seen = std::collections::HashSet::new();
    let mut candidates = Vec::new();
    for query_text in search_queries_for_api(display_name) {
        for candidate in
            fetch_curseforge_search_batch(client, api_key, &query_text, PROVIDER_SEARCH_LIMIT)
        {
            if seen.insert(candidate.id.clone()) {
                candidates.push(candidate);
            }
        }
        if candidates.len() >= PROVIDER_SEARCH_LIMIT {
            break;
        }
    }
    sort_candidates_by_relevance(display_name, &mut candidates);
    candidates.truncate(PROVIDER_SEARCH_LIMIT);
    candidates
}

pub(crate) fn curseforge_candidate_for_project(
    client: &reqwest::blocking::Client,
    api_key: &str,
    project_id: &str,
) -> Option<ProviderCandidate> {
    let payload = curseforge_get(client, api_key, &format!("mods/{project_id}"))?;
    let data = payload.get("data")?;
    let title = data
        .get("name")
        .and_then(|value| value.as_str())
        .map(str::to_string)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "Без названия".to_string());
    let icon_url = data.get("logo").and_then(|logo| {
        non_empty_json_string(logo.get("thumbnailUrl"))
            .or_else(|| non_empty_json_string(logo.get("url")))
    });
    Some(ProviderCandidate {
        id: project_id.to_string(),
        slug: data
            .get("slug")
            .and_then(|value| value.as_str())
            .map(str::to_string),
        title,
        icon_url,
        exact_file_match: false,
        match_score: 0,
    })
}

pub(crate) fn modrinth_candidate_for_project(
    client: &reqwest::blocking::Client,
    project_id: &str,
) -> Option<ProviderCandidate> {
    let payload = modrinth_project(client, project_id)?;
    Some(ProviderCandidate {
        id: project_id.to_string(),
        slug: payload
            .get("slug")
            .and_then(|value| value.as_str())
            .map(str::to_string),
        title: payload
            .get("title")
            .and_then(|value| value.as_str())
            .map(str::to_string)
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "Без названия".to_string()),
        icon_url: non_empty_json_string(payload.get("icon_url")),
        exact_file_match: false,
        match_score: 0,
    })
}

/// Modrinth: `GET /v2/projects?ids=[…]` — до ~100 ID за запрос.
/// Ключи в результате — оба `id` и `slug`, чтобы локальные `modrinth_id` любой формы попадали.
pub(crate) fn modrinth_projects_batch(
    client: &reqwest::blocking::Client,
    ids: &[String],
) -> HashMap<String, serde_json::Value> {
    let mut result = HashMap::new();
    if ids.is_empty() {
        return result;
    }
    let unique: Vec<String> = {
        let mut seen = std::collections::HashSet::new();
        ids.iter()
            .filter(|id| !id.trim().is_empty())
            .filter(|id| seen.insert((*id).clone()))
            .cloned()
            .collect()
    };

    for chunk in unique.chunks(50) {
        let Ok(ids_json) = serde_json::to_string(chunk) else {
            continue;
        };
        let Some(payload) = client
            .get("https://api.modrinth.com/v2/projects")
            .query(&[("ids", ids_json.as_str())])
            .send()
            .ok()
            .and_then(|response| response.error_for_status().ok())
            .and_then(|response| response.json::<serde_json::Value>().ok())
        else {
            continue;
        };
        let Some(array) = payload.as_array() else {
            continue;
        };
        for project in array {
            if let Some(id) = project.get("id").and_then(|v| v.as_str()) {
                result.insert(id.to_string(), project.clone());
            }
            if let Some(slug) = project.get("slug").and_then(|v| v.as_str()) {
                result.entry(slug.to_string()).or_insert_with(|| project.clone());
            }
        }
    }
    result
}

/// Modrinth: `GET /v2/versions?ids=[…]` — до ~100 ID за запрос.
pub(crate) fn modrinth_versions_batch(
    client: &reqwest::blocking::Client,
    ids: &[String],
) -> HashMap<String, serde_json::Value> {
    let mut result = HashMap::new();
    if ids.is_empty() {
        return result;
    }
    let unique: Vec<String> = {
        let mut seen = std::collections::HashSet::new();
        ids.iter()
            .filter(|id| !id.trim().is_empty())
            .filter(|id| seen.insert((*id).clone()))
            .cloned()
            .collect()
    };

    for chunk in unique.chunks(50) {
        let Ok(ids_json) = serde_json::to_string(chunk) else {
            continue;
        };
        let Some(payload) = client
            .get("https://api.modrinth.com/v2/versions")
            .query(&[("ids", ids_json.as_str())])
            .send()
            .ok()
            .and_then(|response| response.error_for_status().ok())
            .and_then(|response| response.json::<serde_json::Value>().ok())
        else {
            continue;
        };
        let Some(array) = payload.as_array() else {
            continue;
        };
        for version in array {
            if let Some(id) = version.get("id").and_then(|v| v.as_str()) {
                result.insert(id.to_string(), version.clone());
            }
        }
    }
    result
}

/// CurseForge: `POST /v1/mods` `{"modIds":[…]}` — до 50 ID за запрос.
/// Возвращает payload, обёрнутый в `{"data": …}`, чтобы быть совместимым с одиночным GET.
pub(crate) fn curseforge_mods_batch(
    client: &reqwest::blocking::Client,
    api_key: &str,
    ids: &[String],
) -> HashMap<String, serde_json::Value> {
    let mut result = HashMap::new();
    if api_key.trim().is_empty() || ids.is_empty() {
        return result;
    }
    let parsed: Vec<i64> = {
        let mut seen = std::collections::HashSet::new();
        ids.iter()
            .filter_map(|id| id.parse::<i64>().ok())
            .filter(|id| seen.insert(*id))
            .collect()
    };

    for chunk in parsed.chunks(50) {
        let body = serde_json::json!({ "modIds": chunk });
        let Some(payload) = client
            .post("https://api.curseforge.com/v1/mods")
            .header("x-api-key", api_key.trim())
            .json(&body)
            .send()
            .ok()
            .and_then(|response| response.error_for_status().ok())
            .and_then(|response| response.json::<serde_json::Value>().ok())
        else {
            continue;
        };
        let Some(array) = payload.get("data").and_then(|v| v.as_array()) else {
            continue;
        };
        for item in array {
            if let Some(id) = item.get("id").and_then(|v| v.as_i64()) {
                let wrapped = serde_json::json!({ "data": item });
                result.insert(id.to_string(), wrapped);
            }
        }
    }
    result
}

/// CurseForge: `POST /v1/mods/files` `{"fileIds":[…]}` — до 50 ID за запрос.
/// Возвращает payload, обёрнутый в `{"data": …}`.
pub(crate) fn curseforge_files_batch(
    client: &reqwest::blocking::Client,
    api_key: &str,
    ids: &[String],
) -> HashMap<String, serde_json::Value> {
    let mut result = HashMap::new();
    if api_key.trim().is_empty() || ids.is_empty() {
        return result;
    }
    let parsed: Vec<i64> = {
        let mut seen = std::collections::HashSet::new();
        ids.iter()
            .filter_map(|id| id.parse::<i64>().ok())
            .filter(|id| seen.insert(*id))
            .collect()
    };

    for chunk in parsed.chunks(50) {
        let body = serde_json::json!({ "fileIds": chunk });
        let Some(payload) = client
            .post("https://api.curseforge.com/v1/mods/files")
            .header("x-api-key", api_key.trim())
            .json(&body)
            .send()
            .ok()
            .and_then(|response| response.error_for_status().ok())
            .and_then(|response| response.json::<serde_json::Value>().ok())
        else {
            continue;
        };
        let Some(array) = payload.get("data").and_then(|v| v.as_array()) else {
            continue;
        };
        for item in array {
            if let Some(id) = item.get("id").and_then(|v| v.as_i64()) {
                let wrapped = serde_json::json!({ "data": item });
                result.insert(id.to_string(), wrapped);
            }
        }
    }
    result
}

pub(crate) fn modrinth_cover_url_from_payload(payload: &serde_json::Value) -> Option<String> {
    payload
        .get("icon_url")
        .and_then(|value| value.as_str())
        .map(str::to_string)
        .filter(|value| !value.is_empty())
}

pub(crate) fn curseforge_cover_url_from_payload(payload: &serde_json::Value) -> Option<String> {
    payload
        .get("data")
        .and_then(|data| data.get("logo"))
        .and_then(|logo| {
            non_empty_json_string(logo.get("thumbnailUrl"))
                .or_else(|| non_empty_json_string(logo.get("url")))
        })
}

pub(crate) fn modrinth_dependencies_from_payload(
    payload: &serde_json::Value,
    modrinth_lookup: &HashMap<String, String>,
) -> Vec<String> {
    let mut deps = Vec::new();
    let Some(items) = payload.get("dependencies").and_then(|value| value.as_array()) else {
        return deps;
    };
    for dep in items {
        let required = dep
            .get("dependency_type")
            .and_then(|value| value.as_str())
            .is_some_and(|kind| kind == "required");
        if !required {
            continue;
        }
        let Some(project_id) = dep.get("project_id").and_then(|value| value.as_str()) else {
            continue;
        };
        if let Some(key) = modrinth_lookup.get(project_id) {
            deps.push(key.clone());
        }
    }
    deps
}

pub(crate) fn curseforge_dependencies_from_payload(
    payload: &serde_json::Value,
    curseforge_lookup: &HashMap<String, String>,
) -> Vec<String> {
    let mut deps = Vec::new();
    let Some(items) = payload
        .get("data")
        .and_then(|data| data.get("dependencies"))
        .and_then(|value| value.as_array())
    else {
        return deps;
    };
    for dep in items {
        let required = dep
            .get("relationType")
            .and_then(|value| value.as_i64())
            .is_some_and(|kind| kind == 3);
        if !required {
            continue;
        }
        let Some(dep_id) = dep.get("modId").and_then(|value| value.as_i64()) else {
            continue;
        };
        if let Some(key) = curseforge_lookup.get(&dep_id.to_string()) {
            deps.push(key.clone());
        }
    }
    deps
}

pub(crate) fn fetch_api_dependencies(
    item: &ModEntry,
    client: &reqwest::blocking::Client,
    settings: &Settings,
    modrinth_lookup: &HashMap<String, String>,
    curseforge_lookup: &HashMap<String, String>,
) -> Vec<String> {
    let mut auto_dependencies = Vec::new();

    let prefer_curseforge = item.source == "curseforge";

    if !prefer_curseforge && item.modrinth_id.is_some() && settings.auto_prefetch_dependencies {
        if let Some(version_id) = item.modrinth_version_id.as_deref() {
            if let Some(payload) = modrinth_version(client, version_id) {
                if let Some(deps) = payload
                    .get("dependencies")
                    .and_then(|value| value.as_array())
                {
                    for dep in deps {
                        let required = dep
                            .get("dependency_type")
                            .and_then(|value| value.as_str())
                            .is_some_and(|kind| kind == "required");
                        let Some(project_id) =
                            dep.get("project_id").and_then(|value| value.as_str())
                        else {
                            continue;
                        };
                        if required {
                            if let Some(key) = modrinth_lookup.get(project_id) {
                                auto_dependencies.push(key.clone());
                            }
                        }
                    }
                }
            }
        }
    }

    if let Some(project_id) = item.curseforge_id.as_deref() {
        if settings.auto_prefetch_dependencies {
            if let Some(file_id) = item.curseforge_file_id.as_deref() {
                if let Some(payload) = curseforge_get(
                    client,
                    &settings.curseforge_api_key,
                    &format!("mods/{project_id}/files/{file_id}"),
                ) {
                    if let Some(deps) = payload
                        .get("data")
                        .and_then(|data| data.get("dependencies"))
                        .and_then(|value| value.as_array())
                    {
                        for dep in deps {
                            let required = dep
                                .get("relationType")
                                .and_then(|value| value.as_i64())
                                .is_some_and(|kind| kind == 3);
                            let Some(dep_id) = dep.get("modId").and_then(|value| value.as_i64())
                            else {
                                continue;
                            };
                            if required {
                                if let Some(key) = curseforge_lookup.get(&dep_id.to_string()) {
                                    auto_dependencies.push(key.clone());
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    if prefer_curseforge {
        if item.modrinth_id.is_some() && settings.auto_prefetch_dependencies {
            if let Some(version_id) = item.modrinth_version_id.as_deref() {
                if let Some(payload) = modrinth_version(client, version_id) {
                    if let Some(deps) = payload
                        .get("dependencies")
                        .and_then(|value| value.as_array())
                    {
                        for dep in deps {
                            let required = dep
                                .get("dependency_type")
                                .and_then(|value| value.as_str())
                                .is_some_and(|kind| kind == "required");
                            let Some(project_id) =
                                dep.get("project_id").and_then(|value| value.as_str())
                            else {
                                continue;
                            };
                            if required {
                                if let Some(key) = modrinth_lookup.get(project_id) {
                                    auto_dependencies.push(key.clone());
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    auto_dependencies
}

pub(crate) fn resolve_cover_url(
    item: &ModEntry,
    client: &reqwest::blocking::Client,
    api_key: &str,
) -> Option<String> {
    let prefer_curseforge = item.source == "curseforge";

    if !prefer_curseforge {
        if let Some(project_id) = item.modrinth_id.as_deref() {
            if let Some(url) = modrinth_project(client, project_id).and_then(|payload| {
                payload
                    .get("icon_url")
                    .and_then(|value| value.as_str())
                    .map(str::to_string)
            }) {
                return Some(url);
            }
        }
    }
    if let Some(project_id) = item.curseforge_id.as_deref() {
        if let Some(url) =
            curseforge_get(client, api_key, &format!("mods/{project_id}")).and_then(|payload| {
                payload
                    .get("data")
                    .and_then(|data| data.get("logo"))
                    .and_then(|logo| {
                        non_empty_json_string(logo.get("thumbnailUrl"))
                            .or_else(|| non_empty_json_string(logo.get("url")))
                    })
            })
        {
            return Some(url);
        }
        if let Some(url) = modrinth_search_icon(client, &item.display_name) {
            return Some(url);
        }
    }
    if prefer_curseforge {
        if let Some(project_id) = item.modrinth_id.as_deref() {
            if let Some(url) = modrinth_project(client, project_id).and_then(|payload| {
                payload
                    .get("icon_url")
                    .and_then(|value| value.as_str())
                    .map(str::to_string)
            }) {
                return Some(url);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests;
