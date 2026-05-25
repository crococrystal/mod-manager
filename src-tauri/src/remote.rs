use std::collections::HashMap;

use crate::mods::ModEntry;
use crate::settings::Settings;

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
        .user_agent("mod-manager/0.1.0")
        .build()
        .ok()
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

pub(crate) fn modrinth_project_info(
    client: &reqwest::blocking::Client,
    project_id: &str,
) -> Option<ProviderProject> {
    let payload = modrinth_project(client, project_id)?;
    let id = payload
        .get("id")
        .or_else(|| payload.get("slug"))
        .and_then(|value| value.as_str())?;
    Some(ProviderProject {
        id: id.to_string(),
        slug: payload
            .get("slug")
            .and_then(|value| value.as_str())
            .map(str::to_string),
        title: payload
            .get("title")
            .and_then(|value| value.as_str())
            .map(str::to_string),
        project_type: payload
            .get("project_type")
            .and_then(|value| value.as_str())
            .map(str::to_string),
    })
}

fn strip_qualifiers(value: &str) -> String {
    let mut result = String::new();
    let mut depth = 0u32;
    for ch in value.chars() {
        match ch {
            '(' | '[' => depth = depth.saturating_add(1),
            ')' | ']' => depth = depth.saturating_sub(1),
            _ if depth == 0 => result.push(ch),
            _ => {}
        }
    }
    result.trim().to_string()
}

fn normalized_match_key(value: &str) -> String {
    value
        .to_ascii_lowercase()
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .collect()
}

fn slug_key(value: &str) -> String {
    let mut result = String::new();
    let mut previous_dash = false;
    for ch in value.to_ascii_lowercase().chars() {
        if ch.is_ascii_alphanumeric() {
            result.push(ch);
            previous_dash = false;
        } else if !previous_dash {
            result.push('-');
            previous_dash = true;
        }
    }
    result.trim_matches('-').to_string()
}

pub(crate) fn provider_project_matches(project: &ProviderProject, display_name: &str) -> bool {
    let clean = strip_qualifiers(display_name);
    let mut name_keys = vec![normalized_match_key(display_name)];
    let clean_key = normalized_match_key(&clean);
    if !clean_key.is_empty() && !name_keys.contains(&clean_key) {
        name_keys.push(clean_key);
    }

    if let Some(title) = project.title.as_deref() {
        let title_key = normalized_match_key(title);
        if !title_key.is_empty() && name_keys.contains(&title_key) {
            return true;
        }
    }

    let mut slug_keys = vec![slug_key(display_name)];
    let clean_slug = slug_key(&clean);
    if !clean_slug.is_empty() && !slug_keys.contains(&clean_slug) {
        slug_keys.push(clean_slug);
    }
    project
        .slug
        .as_deref()
        .map(slug_key)
        .is_some_and(|slug| !slug.is_empty() && slug_keys.contains(&slug))
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
            let fingerprint = item
                .get("id")
                .and_then(|value| value.as_u64())
                .and_then(|value| u32::try_from(value).ok())?;
            let file = item.get("file")?;
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
    let query = urlencoding::encode(display_name.trim());
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

pub(crate) fn modrinth_search_project(
    client: &reqwest::blocking::Client,
    display_name: &str,
) -> Option<ProviderProject> {
    let query = urlencoding::encode(display_name.trim());
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
    let hits = payload
        .get("hits")
        .and_then(|hits| hits.as_array())
        .cloned()
        .unwrap_or_default();
    hits.into_iter().find_map(|hit| {
        let id = hit
            .get("project_id")
            .or_else(|| hit.get("slug"))
            .and_then(|value| value.as_str())?;
        let project = ProviderProject {
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
        };
        modrinth_project_matches(&project, display_name).then_some(project)
    })
}

pub(crate) fn curseforge_search_mod(
    client: &reqwest::blocking::Client,
    api_key: &str,
    display_name: &str,
) -> Option<ProviderProject> {
    let query = urlencoding::encode(display_name.trim());
    if query.is_empty() || api_key.trim().is_empty() {
        return None;
    }
    let payload = curseforge_get(
        client,
        api_key,
        &format!("mods/search?gameId=432&classId=6&searchFilter={query}&pageSize=10"),
    )?;
    let items = payload
        .get("data")
        .and_then(|data| data.as_array())
        .cloned()
        .unwrap_or_default();
    items.into_iter().find_map(|item| {
        let id = item.get("id").and_then(|value| value.as_i64())?;
        let project = ProviderProject {
            id: id.to_string(),
            slug: item
                .get("slug")
                .and_then(|value| value.as_str())
                .map(str::to_string),
            title: item
                .get("name")
                .and_then(|value| value.as_str())
                .map(str::to_string),
            project_type: None,
        };
        provider_project_matches(&project, display_name).then_some(project)
    })
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
mod tests {
    use super::*;

    fn project(title: &str, slug: &str, project_type: &str) -> ProviderProject {
        ProviderProject {
            id: slug.to_string(),
            slug: Some(slug.to_string()),
            title: Some(title.to_string()),
            project_type: Some(project_type.to_string()),
        }
    }

    #[test]
    fn modrinth_match_accepts_exact_mod() {
        let project = project("FTB Quests", "ftb-quests", "mod");
        assert!(modrinth_project_matches(&project, "FTB Quests"));
    }

    #[test]
    fn modrinth_match_rejects_resource_packs() {
        let project = project("FTB Quests 中文翻译", "ftb-quests-zh_cn", "resourcepack");
        assert!(!modrinth_project_matches(&project, "FTB Quests"));
    }

    #[test]
    fn modrinth_match_rejects_similar_addons() {
        let project = project("FTB Quests Freeze Fix", "ftb-quests-freeze-fix", "mod");
        assert!(!modrinth_project_matches(&project, "FTB Quests"));
    }
}
