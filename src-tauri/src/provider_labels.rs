use serde::Serialize;

use crate::mods::{normalize_side, ModEntry};
use crate::remote::{curseforge_get, http_client, modrinth_project, modrinth_version};
use crate::settings::Settings;
use crate::tags::{ModTags, ProviderLabelsStore};
use crate::util::now_iso;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RefreshProviderLabelsResult {
    pub key: String,
    pub side: String,
    pub library: bool,
    pub technical: bool,
    pub side_mode: String,
    pub manual_side: String,
    pub manual_library: bool,
    pub manual_technical: bool,
    pub provider_side: String,
    pub provider_library: bool,
    pub provider_technical: bool,
}

pub(crate) fn side_mode_for(tag: &ModTags) -> String {
    if tag.label_overrides.side_mode.trim() == "manual" {
        "manual".to_string()
    } else {
        "auto".to_string()
    }
}

pub(crate) fn resolve_side(tag: &ModTags) -> String {
    if side_mode_for(tag) == "manual" {
        return stored_side(tag);
    }
    if tag.provider_labels.fetched_at.is_empty() {
        return normalize_side("universal");
    }
    map_provider_side(&tag.provider_labels).unwrap_or_else(|| normalize_side("universal"))
}

pub(crate) fn resolve_library(tag: &ModTags) -> bool {
    if side_mode_for(tag) == "manual" {
        return tag.library;
    }
    if tag.provider_labels.fetched_at.is_empty() {
        return false;
    }
    map_provider_library(&tag.provider_labels)
}

pub(crate) fn resolve_technical(tag: &ModTags) -> bool {
    if side_mode_for(tag) == "manual" {
        return tag.technical;
    }
    if tag.provider_labels.fetched_at.is_empty() {
        return false;
    }
    map_provider_technical(&tag.provider_labels)
}

pub(crate) fn refresh_result_for(tag: &ModTags, key: &str) -> RefreshProviderLabelsResult {
    let (manual_side, manual_library, manual_technical) = manual_tags_for(tag);
    let (provider_side, provider_library, provider_technical) = provider_tags_for(tag);
    RefreshProviderLabelsResult {
        key: key.to_string(),
        side: resolve_side(tag),
        library: resolve_library(tag),
        technical: resolve_technical(tag),
        side_mode: side_mode_for(tag),
        manual_side,
        manual_library,
        manual_technical,
        provider_side,
        provider_library,
        provider_technical,
    }
}

pub(crate) fn manual_tags_for(tag: &ModTags) -> (String, bool, bool) {
    (stored_side(tag), tag.library, tag.technical)
}

pub(crate) fn provider_tags_for(tag: &ModTags) -> (String, bool, bool) {
    if tag.provider_labels.fetched_at.is_empty() {
        return (normalize_side("universal"), false, false);
    }
    (
        map_provider_side(&tag.provider_labels).unwrap_or_else(|| normalize_side("universal")),
        map_provider_library(&tag.provider_labels),
        map_provider_technical(&tag.provider_labels),
    )
}

pub(crate) fn refresh_provider_labels_bulk(
    settings: &Settings,
    tags: &mut crate::tags::TagFile,
    mods: &[ModEntry],
    only_missing: bool,
) -> Result<usize, String> {
    let candidates: Vec<&ModEntry> = mods
        .iter()
        .filter(|item| {
            (item.source == "modrinth" || item.source == "curseforge")
                && (item.modrinth_id.is_some() || item.curseforge_id.is_some())
        })
        .collect();

    let mut updated = 0usize;
    for item in candidates.iter() {
        if only_missing {
            let already = tags
                .mods
                .get(&item.key)
                .map(|tag| !tag.provider_labels.fetched_at.is_empty())
                .unwrap_or(false);
            if already {
                continue;
            }
        }
        let tag = tags.mods.entry(item.key.clone()).or_default();
        if fetch_and_store_provider_labels(tag, item, settings).is_ok() {
            updated += 1;
        }
    }
    if updated > 0 {
        tags.updated_at = now_iso();
    }
    Ok(updated)
}

pub(crate) fn fetch_and_store_provider_labels(
    tag: &mut ModTags,
    item: &ModEntry,
    settings: &Settings,
) -> Result<(), String> {
    let Some(client) = http_client() else {
        return Err("Не удалось создать HTTP-клиент.".to_string());
    };
    let snapshot = match item.source.as_str() {
        "modrinth" => {
            let project_id = item
                .modrinth_id
                .as_deref()
                .ok_or_else(|| "У мода нет Modrinth ID.".to_string())?;
            fetch_modrinth_labels(&client, project_id, item.modrinth_version_id.as_deref())?
        }
        "curseforge" => {
            if settings.curseforge_api_key.trim().is_empty() {
                return Err("Для CurseForge нужен API key.".to_string());
            }
            let project_id = item
                .curseforge_id
                .as_deref()
                .ok_or_else(|| "У мода нет CurseForge ID.".to_string())?;
            fetch_curseforge_labels(
                &client,
                &settings.curseforge_api_key,
                project_id,
                item.curseforge_file_id.as_deref(),
            )?
        }
        _ => {
            return Err("Метки поставщика доступны только для Modrinth и CurseForge.".to_string());
        }
    };
    tag.provider_labels = snapshot;
    tag.updated_at = now_iso();
    Ok(())
}

fn stored_side(tag: &ModTags) -> String {
    normalize_side(if tag.side.is_empty() {
        "universal"
    } else {
        tag.side.as_str()
    })
}

fn fetch_modrinth_labels(
    client: &reqwest::blocking::Client,
    project_id: &str,
    version_id: Option<&str>,
) -> Result<ProviderLabelsStore, String> {
    let payload = modrinth_project(client, project_id)
        .ok_or_else(|| "Modrinth не вернул данные проекта.".to_string())?;
    let version_payload = version_id
        .filter(|value| !value.is_empty())
        .and_then(|version_id| modrinth_version(client, version_id));
    Ok(build_modrinth_labels(&payload, version_payload.as_ref()))
}

pub(crate) fn build_modrinth_labels(
    project: &serde_json::Value,
    version: Option<&serde_json::Value>,
) -> ProviderLabelsStore {
    let categories = json_string_array(project.get("categories"));
    let additional = json_string_array(project.get("additional_categories"));
    let mut loaders = json_string_array(project.get("loaders"));
    let mut game_versions = json_string_array(project.get("game_versions"));
    let client_side = json_string(project.get("client_side")).unwrap_or_default();
    let server_side = json_string(project.get("server_side")).unwrap_or_default();

    if let Some(version) = version {
        merge_unique(&mut loaders, json_string_array(version.get("loaders")));
        merge_unique(
            &mut game_versions,
            json_string_array(version.get("game_versions")),
        );
    }

    ProviderLabelsStore {
        source: "modrinth".to_string(),
        fetched_at: now_iso(),
        categories,
        additional_categories: additional,
        loaders,
        game_versions,
        client_side,
        server_side,
    }
}

fn fetch_curseforge_labels(
    client: &reqwest::blocking::Client,
    api_key: &str,
    project_id: &str,
    file_id: Option<&str>,
) -> Result<ProviderLabelsStore, String> {
    let payload = curseforge_get(client, api_key, &format!("mods/{project_id}"))
        .ok_or_else(|| "CurseForge не вернул данные проекта.".to_string())?;
    let file_payload = file_id
        .filter(|value| !value.is_empty())
        .and_then(|id| curseforge_get(client, api_key, &format!("mods/{project_id}/files/{id}")));
    build_curseforge_labels(&payload, file_payload.as_ref())
        .ok_or_else(|| "CurseForge вернул пустой ответ.".to_string())
}

pub(crate) fn build_curseforge_labels(
    project: &serde_json::Value,
    file: Option<&serde_json::Value>,
) -> Option<ProviderLabelsStore> {
    let data = project.get("data")?;

    let mut categories = Vec::new();
    if let Some(items) = data.get("categories").and_then(|value| value.as_array()) {
        for item in items {
            if let Some(slug) = item
                .get("slug")
                .and_then(|value| value.as_str())
                .map(category_slug)
            {
                push_unique(&mut categories, slug);
            } else if let Some(name) = item.get("name").and_then(|value| value.as_str()) {
                push_unique(&mut categories, category_slug(name));
            }
        }
    }

    let mut loaders = Vec::new();
    let mut game_versions = Vec::new();
    let mut client_side = String::new();
    let mut server_side = String::new();

    let file_data = file.and_then(|value| value.get("data"));

    if let Some(file) = file_data {
        for version in json_string_array(file.get("gameVersions")) {
            if version.eq_ignore_ascii_case("client") {
                client_side = "required".to_string();
            } else if version.eq_ignore_ascii_case("server") {
                server_side = "required".to_string();
            } else {
                push_unique(&mut game_versions, version);
            }
        }
        if let Some(indexes) = data.get("latestFilesIndexes").and_then(|v| v.as_array()) {
            if let Some(file_id) = file.get("id").and_then(|v| v.as_i64()) {
                for index in indexes {
                    if index.get("fileId").and_then(|v| v.as_i64()) == Some(file_id) {
                        if let Some(loader) = index.get("modLoader").and_then(|v| v.as_i64()) {
                            if let Some(name) = curseforge_loader_name(loader) {
                                push_unique(&mut loaders, name);
                            }
                        }
                        if let Some(version) = index.get("gameVersion").and_then(|v| v.as_str()) {
                            push_unique(&mut game_versions, version.to_string());
                        }
                    }
                }
            }
        }
    }

    if client_side.is_empty() && server_side.is_empty() {
        client_side = "optional".to_string();
        server_side = "optional".to_string();
    }

    Some(ProviderLabelsStore {
        source: "curseforge".to_string(),
        fetched_at: now_iso(),
        categories,
        additional_categories: Vec::new(),
        loaders,
        game_versions,
        client_side,
        server_side,
    })
}

fn category_slugs(store: &ProviderLabelsStore) -> Vec<String> {
    let mut slugs = store.categories.clone();
    merge_unique(&mut slugs, store.additional_categories.clone());
    slugs
}

fn map_provider_library(store: &ProviderLabelsStore) -> bool {
    category_slugs(store)
        .iter()
        .any(|slug| slug == "library" || slug.contains("library"))
}

fn map_provider_technical(store: &ProviderLabelsStore) -> bool {
    category_slugs(store)
        .iter()
        .any(|slug| slug == "optimization" || slug == "performance")
}

fn map_provider_side(store: &ProviderLabelsStore) -> Option<String> {
    if store.fetched_at.is_empty() {
        return None;
    }
    let client = store.client_side.as_str();
    let server = store.server_side.as_str();
    let side = match (client, server) {
        ("required", "unsupported") => "client",
        ("unsupported", "required") => "server",
        ("required", "required")
        | ("optional", "optional")
        | ("required", "optional")
        | ("optional", "required") => "universal",
        _ => "universal",
    };
    Some(normalize_side(side))
}

fn json_string(value: Option<&serde_json::Value>) -> Option<String> {
    value
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn json_string_array(value: Option<&serde_json::Value>) -> Vec<String> {
    value
        .and_then(|value| value.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

fn merge_unique(target: &mut Vec<String>, items: Vec<String>) {
    for item in items {
        push_unique(target, item);
    }
}

fn push_unique(target: &mut Vec<String>, value: String) {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return;
    }
    if !target.iter().any(|item| item == trimmed) {
        target.push(trimmed.to_string());
    }
}

fn category_slug(value: &str) -> String {
    value.trim().to_ascii_lowercase().replace(' ', "-")
}

fn curseforge_loader_name(value: i64) -> Option<String> {
    Some(
        match value {
            1 => "forge",
            2 => "cauldron",
            3 => "liteloader",
            4 => "fabric",
            5 => "quilt",
            6 => "neoforge",
            _ => return None,
        }
        .to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_modrinth_client_only() {
        let store = ProviderLabelsStore {
            fetched_at: now_iso(),
            client_side: "required".to_string(),
            server_side: "unsupported".to_string(),
            ..Default::default()
        };
        assert_eq!(map_provider_side(&store).as_deref(), Some("client"));
    }

    #[test]
    fn maps_modrinth_optimization_flag() {
        let store = ProviderLabelsStore {
            fetched_at: now_iso(),
            categories: vec!["optimization".to_string()],
            ..Default::default()
        };
        assert!(map_provider_technical(&store));
    }
}
