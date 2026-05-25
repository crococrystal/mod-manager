use std::collections::HashMap;

use crate::mods::ModEntry;
use crate::settings::Settings;

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

pub(crate) fn modrinth_search_icon(
    client: &reqwest::blocking::Client,
    display_name: &str,
) -> Option<String> {
    let query = urlencoding::encode(display_name.trim());
    if query.is_empty() {
        return None;
    }
    let payload = client
        .get(format!(
            "https://api.modrinth.com/v2/search?query={query}&limit=1"
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
        .and_then(|hits| hits.first())
        .and_then(|hit| hit.get("icon_url"))
        .and_then(|value| value.as_str())
        .map(str::to_string)
}

pub(crate) fn fetch_api_dependencies(
    item: &ModEntry,
    client: &reqwest::blocking::Client,
    settings: &Settings,
    modrinth_lookup: &HashMap<String, String>,
    curseforge_lookup: &HashMap<String, String>,
) -> Vec<String> {
    let mut auto_dependencies = Vec::new();

    if item.modrinth_id.is_some() && settings.auto_prefetch_dependencies {
        if let Some(version_id) = item.modrinth_version_id.as_deref() {
            if let Some(payload) = modrinth_version(client, version_id) {
                if let Some(deps) = payload.get("dependencies").and_then(|value| value.as_array()) {
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

    auto_dependencies
}

pub(crate) fn resolve_cover_url(
    item: &ModEntry,
    client: &reqwest::blocking::Client,
    api_key: &str,
) -> Option<String> {
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
    if let Some(project_id) = item.curseforge_id.as_deref() {
        if let Some(url) =
            curseforge_get(client, api_key, &format!("mods/{project_id}")).and_then(|payload| {
                payload
                    .get("data")
                    .and_then(|data| data.get("logo"))
                    .and_then(|logo| {
                        logo.get("thumbnailUrl")
                            .or_else(|| logo.get("url"))
                            .and_then(|value| value.as_str())
                            .map(str::to_string)
                    })
            })
        {
            return Some(url);
        }
        if let Some(url) = modrinth_search_icon(client, &item.display_name) {
            return Some(url);
        }
    }
    None
}
