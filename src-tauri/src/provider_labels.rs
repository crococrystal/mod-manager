use serde::Serialize;

use crate::mods::{normalize_side, ModEntry, UNKNOWN_SIDE};
use crate::remote::{curseforge_get, curseforge_mod_info, http_client, modrinth_project, modrinth_version};
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
        return UNKNOWN_SIDE.to_string();
    }
    map_provider_side(&tag.provider_labels).unwrap_or_else(|| UNKNOWN_SIDE.to_string())
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
        return (UNKNOWN_SIDE.to_string(), false, false);
    }
    (
        map_provider_side(&tag.provider_labels).unwrap_or_else(|| UNKNOWN_SIDE.to_string()),
        map_provider_library(&tag.provider_labels),
        map_provider_technical(&tag.provider_labels),
    )
}

fn manual_side_is_unset(tag: &ModTags) -> bool {
    if side_mode_for(tag) == "manual" {
        return false;
    }
    if tag.library || tag.technical {
        return false;
    }
    tag.side.is_empty() || tag.side == "universal" || tag.side == UNKNOWN_SIDE
}

pub(crate) fn sync_stored_side_from_provider(tag: &mut ModTags) {
    if side_mode_for(tag) == "manual" || !manual_side_is_unset(tag) {
        return;
    }
    let (provider_side, provider_library, provider_technical) = provider_tags_for(tag);
    tag.side = provider_side;
    tag.library = provider_library;
    tag.technical = provider_technical;
}

pub(crate) fn refresh_provider_labels_bulk(
    settings: &Settings,
    tags: &mut crate::tags::TagFile,
    mods: &[ModEntry],
    only_missing: bool,
) -> Result<usize, String> {
    let candidates: Vec<&ModEntry> = mods
        .iter()
        .filter(|item| item.modrinth_id.is_some() || item.curseforge_id.is_some())
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
    let snapshot = if let Some(project_id) = item.modrinth_id.as_deref() {
        fetch_modrinth_labels(&client, project_id, item.modrinth_version_id.as_deref())?
    } else if item.source.as_str() == "curseforge" {
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
            None,
        )?
    } else {
        return Err("Метки поставщика доступны только для Modrinth и CurseForge.".to_string());
    };
    tag.provider_labels = snapshot;
    tag.updated_at = now_iso();
    sync_stored_side_from_provider(tag);
    Ok(())
}

fn stored_side(tag: &ModTags) -> String {
    if tag.side.is_empty() {
        return UNKNOWN_SIDE.to_string();
    }
    normalize_side(tag.side.as_str())
}

pub(crate) fn link_modrinth_ids_for_curseforge_mods(
    client: &reqwest::blocking::Client,
    curseforge_api_key: &str,
    tags: &mut crate::tags::TagFile,
    mods: &[ModEntry],
) -> bool {
    let mut changed = false;
    for item in mods {
        let Some(curseforge_id) = item.curseforge_id.as_deref() else {
            continue;
        };
        let tag = tags.mods.entry(item.key.clone()).or_default();
        if !tag.modrinth_id.is_empty() {
            continue;
        }
        let mut slug = if !tag.curseforge_slug.is_empty() {
            tag.curseforge_slug.clone()
        } else {
            String::new()
        };
        if slug.is_empty() && !curseforge_api_key.trim().is_empty() {
            if let Some(info) = curseforge_mod_info(client, curseforge_api_key, curseforge_id) {
                slug = info.slug.unwrap_or_default();
                if !slug.is_empty() && tag.curseforge_slug.is_empty() {
                    tag.curseforge_slug = slug.clone();
                }
            }
        }
        if slug.is_empty() {
            continue;
        }
        let Some(project) = modrinth_project(client, &slug) else {
            continue;
        };
        let Some(project_id) = json_string(project.get("id")) else {
            continue;
        };
        tag.modrinth_id = project_id;
        tag.updated_at = now_iso();
        changed = true;
    }
    if changed {
        tags.updated_at = now_iso();
    }
    changed
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
    modrinth_fallback: Option<&ProviderLabelsStore>,
) -> Result<ProviderLabelsStore, String> {
    let payload = curseforge_get(client, api_key, &format!("mods/{project_id}"))
        .ok_or_else(|| "CurseForge не вернул данные проекта.".to_string())?;
    let file_payload = file_id
        .filter(|value| !value.is_empty())
        .and_then(|id| curseforge_get(client, api_key, &format!("mods/{project_id}/files/{id}")));
    let store = build_curseforge_labels(&payload, file_payload.as_ref())
        .ok_or_else(|| "CurseForge вернул пустой ответ.".to_string())?;

    if !curseforge_side_is_ambiguous(&store) {
        return Ok(store);
    }

    let mut store = finalize_curseforge_labels(store, modrinth_fallback, Some(&payload), None);
    if curseforge_side_is_ambiguous(&store) {
        let files_payload =
            curseforge_get(client, api_key, &format!("mods/{project_id}/files?pageSize=50"));
        store = finalize_curseforge_labels(store, None, None, files_payload.as_ref());
    }
    Ok(store)
}

#[derive(Clone, Copy, Default)]
struct CurseforgeSideTags {
    client: bool,
    server: bool,
}

fn curseforge_side_tags_from_game_versions(game_versions: &[String]) -> CurseforgeSideTags {
    let mut tags = CurseforgeSideTags::default();
    for version in game_versions {
        if version.eq_ignore_ascii_case("client") {
            tags.client = true;
        } else if version.eq_ignore_ascii_case("server") {
            tags.server = true;
        }
    }
    tags
}

fn curseforge_side_tags_from_file(file: &serde_json::Value) -> CurseforgeSideTags {
    let mut tags = curseforge_side_tags_from_game_versions(&json_string_array(file.get("gameVersions")));
    if let Some(items) = file.get("sortableGameVersions").and_then(|value| value.as_array()) {
        for item in items {
            let name = item
                .get("gameVersionName")
                .or_else(|| item.get("gameVersion"))
                .and_then(|value| value.as_str())
                .unwrap_or_default();
            if name.eq_ignore_ascii_case("client") {
                tags.client = true;
            } else if name.eq_ignore_ascii_case("server") {
                tags.server = true;
            }
        }
    }
    tags
}

fn merge_curseforge_side_tags(into: &mut CurseforgeSideTags, other: CurseforgeSideTags) {
    into.client |= other.client;
    into.server |= other.server;
}

fn infer_curseforge_side_from_file_entries(
    files: &[serde_json::Value],
) -> Option<(String, String)> {
    let mut tags = CurseforgeSideTags::default();
    for file in files {
        merge_curseforge_side_tags(&mut tags, curseforge_side_tags_from_file(file));
    }
    if !tags.client && !tags.server {
        return None;
    }
    Some(curseforge_sides_from_tags(tags))
}

fn curseforge_sides_from_tags(tags: CurseforgeSideTags) -> (String, String) {
    match (tags.client, tags.server) {
        (true, false) => ("required".to_string(), "unsupported".to_string()),
        (false, true) => ("unsupported".to_string(), "required".to_string()),
        (true, true) => ("required".to_string(), "required".to_string()),
        (false, false) => (String::new(), String::new()),
    }
}

pub(crate) fn curseforge_side_is_ambiguous(store: &ProviderLabelsStore) -> bool {
    store.client_side.is_empty() && store.server_side.is_empty()
        || (store.client_side == "optional" && store.server_side == "optional")
}

fn provider_side_is_definitive(store: &ProviderLabelsStore) -> bool {
    !store.client_side.is_empty() || !store.server_side.is_empty()
}

fn infer_curseforge_side_from_project_files(
    payload: &serde_json::Value,
) -> Option<(String, String)> {
    if let Some(files) = payload.get("data").and_then(|value| value.as_array()) {
        return infer_curseforge_side_from_file_entries(files);
    }
    None
}

fn infer_curseforge_side_from_project_data(
    project_data: &serde_json::Value,
) -> Option<(String, String)> {
    let mut tags = CurseforgeSideTags::default();
    for key in ["latestFiles", "latestEarlyAccessFilesIndexes"] {
        if let Some(files) = project_data.get(key).and_then(|value| value.as_array()) {
            for file in files {
                merge_curseforge_side_tags(&mut tags, curseforge_side_tags_from_file(file));
            }
        }
    }
    if !tags.client && !tags.server {
        return None;
    }
    Some(curseforge_sides_from_tags(tags))
}

fn apply_curseforge_optional_side_default(store: &mut ProviderLabelsStore) {
    if store.client_side.is_empty() && store.server_side.is_empty() {
        store.client_side = "optional".to_string();
        store.server_side = "optional".to_string();
    }
}

pub(crate) fn finalize_curseforge_labels(
    mut store: ProviderLabelsStore,
    modrinth_fallback: Option<&ProviderLabelsStore>,
    project_payload: Option<&serde_json::Value>,
    project_files_payload: Option<&serde_json::Value>,
) -> ProviderLabelsStore {
    if !curseforge_side_is_ambiguous(&store) {
        return store;
    }

    if let Some(mr) = modrinth_fallback {
        if provider_side_is_definitive(mr) {
            store.client_side = mr.client_side.clone();
            store.server_side = mr.server_side.clone();
            if !curseforge_side_is_ambiguous(&store) {
                return store;
            }
        }
    }

    if let Some(project_data) = project_payload.and_then(|value| value.get("data")) {
        if let Some((client, server)) = infer_curseforge_side_from_project_data(project_data) {
            store.client_side = client;
            store.server_side = server;
            return store;
        }
    }

    if let Some((client, server)) = project_files_payload
        .and_then(infer_curseforge_side_from_project_files)
    {
        store.client_side = client;
        store.server_side = server;
        return store;
    }

    apply_curseforge_optional_side_default(&mut store);
    store
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
        let file_tags = curseforge_side_tags_from_file(file);
        if file_tags.client {
            client_side = "required".to_string();
        }
        if file_tags.server {
            server_side = "required".to_string();
        }
        for version in json_string_array(file.get("gameVersions")) {
            if version.eq_ignore_ascii_case("client") || version.eq_ignore_ascii_case("server") {
                continue;
            }
            push_unique(&mut game_versions, version);
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

    if client_side == "required" && server_side.is_empty() {
        server_side = "unsupported".to_string();
    } else if server_side == "required" && client_side.is_empty() {
        client_side = "unsupported".to_string();
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
        ("required", "unsupported") | ("required", "") => "client",
        ("unsupported", "required") | ("", "required") => "server",
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
    use crate::tags::{LabelOverridesStore, ModTags};

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

    #[test]
    fn maps_curseforge_client_only_from_game_versions() {
        let project = serde_json::json!({ "data": { "categories": [] } });
        let file = serde_json::json!({
            "data": {
                "id": 6662069,
                "gameVersions": ["1.21", "Client", "NeoForge", "1.21.1"]
            }
        });
        let store = build_curseforge_labels(&project, Some(&file)).expect("labels");
        assert_eq!(store.client_side, "required");
        assert_eq!(store.server_side, "unsupported");
        assert_eq!(map_provider_side(&store).as_deref(), Some("client"));
    }

    #[test]
    fn maps_curseforge_server_only_from_game_versions() {
        let project = serde_json::json!({ "data": { "categories": [] } });
        let file = serde_json::json!({
            "data": {
                "id": 1,
                "gameVersions": ["1.21", "Server", "NeoForge"]
            }
        });
        let store = build_curseforge_labels(&project, Some(&file)).expect("labels");
        assert_eq!(store.client_side, "unsupported");
        assert_eq!(store.server_side, "required");
        assert_eq!(map_provider_side(&store).as_deref(), Some("server"));
    }

    #[test]
    fn maps_curseforge_sounds_neoforge_via_project_files() {
        let project = serde_json::json!({ "data": { "categories": [] } });
        let file = serde_json::json!({
            "data": {
                "id": 7218618,
                "gameVersions": ["1.21", "1.21.1", "NeoForge"]
            }
        });
        let project_files = serde_json::json!({
            "data": [
                {
                    "gameVersions": ["1.21", "1.21.1", "NeoForge"]
                },
                {
                    "gameVersions": ["Client", "Fabric", "26.1", "26.1.1"]
                }
            ]
        });
        let store = build_curseforge_labels(&project, Some(&file)).expect("labels");
        assert!(curseforge_side_is_ambiguous(&store));
        let store = finalize_curseforge_labels(store, None, None, Some(&project_files));
        assert_eq!(store.client_side, "required");
        assert_eq!(store.server_side, "unsupported");
        assert_eq!(map_provider_side(&store).as_deref(), Some("client"));
    }

    #[test]
    fn maps_curseforge_sounds_neoforge_via_modrinth_fallback() {
        let project = serde_json::json!({ "data": { "categories": [] } });
        let file = serde_json::json!({
            "data": {
                "id": 7218618,
                "gameVersions": ["1.21", "1.21.1", "NeoForge"]
            }
        });
        let modrinth = ProviderLabelsStore {
            fetched_at: now_iso(),
            source: "modrinth".to_string(),
            client_side: "required".to_string(),
            server_side: "unsupported".to_string(),
            ..Default::default()
        };
        let store = build_curseforge_labels(&project, Some(&file)).expect("labels");
        let store = finalize_curseforge_labels(store, Some(&modrinth), None, None);
        assert_eq!(store.client_side, "required");
        assert_eq!(store.server_side, "unsupported");
        assert_eq!(map_provider_side(&store).as_deref(), Some("client"));
    }

    #[test]
    fn resolve_side_without_provider_labels_is_unknown() {
        let tag = ModTags::default();
        assert_eq!(resolve_side(&tag), UNKNOWN_SIDE);
    }

    #[test]
    fn sync_stored_side_replaces_scan_default_universal_with_provider_client() {
        let mut tag = ModTags {
            side: "universal".to_string(),
            ..Default::default()
        };
        tag.provider_labels = ProviderLabelsStore {
            fetched_at: now_iso(),
            client_side: "required".to_string(),
            server_side: "unsupported".to_string(),
            ..Default::default()
        };
        sync_stored_side_from_provider(&mut tag);
        assert_eq!(tag.side, "client");
        assert_eq!(resolve_side(&tag), "client");
    }

    #[test]
    fn sync_stored_side_does_not_override_manual_side() {
        let mut tag = ModTags {
            side: "universal".to_string(),
            label_overrides: LabelOverridesStore {
                side_mode: "manual".to_string(),
                ..Default::default()
            },
            ..Default::default()
        };
        tag.provider_labels = ProviderLabelsStore {
            fetched_at: now_iso(),
            client_side: "required".to_string(),
            server_side: "unsupported".to_string(),
            ..Default::default()
        };
        sync_stored_side_from_provider(&mut tag);
        assert_eq!(tag.side, "universal");
        assert_eq!(resolve_side(&tag), "universal");
    }

    #[test]
    fn maps_curseforge_client_only_from_sortable_game_versions() {
        let project = serde_json::json!({ "data": { "categories": [] } });
        let file = serde_json::json!({
            "data": {
                "id": 7806535,
                "gameVersions": ["Fabric", "26.1", "26.1.1", "26.1.2"],
                "sortableGameVersions": [
                    {
                        "gameVersionName": "Client",
                        "gameVersionTypeId": 75208
                    }
                ]
            }
        });
        let store = build_curseforge_labels(&project, Some(&file)).expect("labels");
        assert_eq!(store.client_side, "required");
        assert_eq!(store.server_side, "unsupported");
        assert_eq!(map_provider_side(&store).as_deref(), Some("client"));
    }
}
