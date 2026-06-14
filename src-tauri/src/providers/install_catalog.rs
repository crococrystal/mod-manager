use std::{
    collections::HashSet,
    fs,
    path::Path,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::catalog;
use crate::catalog_cache;
use crate::instance_meta::{
    detect_instance_target, target_has_filters, version_matches_target, InstanceTarget,
};
use crate::mod_names::normalized_match_key;
use crate::mods::{scan_mods_for_settings, stable_key, ModEntry};
use crate::mods_watch;
use crate::remote::{
    curseforge_get, curseforge_mod_info, fetch_catalog_project_description, http_client,
    modrinth_project, modrinth_version, search_catalog_curseforge, search_catalog_modrinth,
    ProviderCandidate,
};
use crate::settings::{read_settings, resolve_paths, InstancePaths, Settings};
use crate::tags::{read_tags, write_tags};
use crate::util::{now_iso, now_millis};

use super::versions::{
    download_file, list_curseforge_versions, list_modrinth_versions, ProviderVersion,
};

const MAX_DEPENDENCY_DEPTH: usize = 10;
const PROJECT_DETAILS_CACHE_TTL_MS: u64 = 7 * 24 * 60 * 60 * 1000;

fn project_details_cache_fresh(details: &CatalogProjectDetails) -> bool {
    details.checked_at_ms.is_some_and(|checked_at| {
        now_millis().saturating_sub(checked_at) <= PROJECT_DETAILS_CACHE_TTL_MS
    })
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CatalogSearchRequest {
    pub source: String,
    pub query: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CatalogSearchResponse {
    pub target: InstanceTarget,
    pub candidates: Vec<ProviderCandidate>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CatalogInstallPreviewRequest {
    pub source: String,
    pub project_id: String,
    #[serde(default)]
    pub version_id: Option<String>,
    #[serde(default)]
    pub force_refresh: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CatalogProjectDetailsRequest {
    pub source: String,
    pub project_id: String,
    #[serde(default)]
    pub force_refresh: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CatalogInstallRequest {
    pub source: String,
    pub project_id: String,
    #[serde(default)]
    pub version_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CatalogInstallDependency {
    pub project_id: String,
    pub title: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CatalogInstallPreview {
    pub source: String,
    pub project_id: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slug: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon_url: Option<String>,
    pub target: InstanceTarget,
    pub version: ProviderVersion,
    pub dependencies: Vec<CatalogInstallDependency>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checked_at_ms: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CatalogProjectDetails {
    pub source: String,
    pub project_id: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slug: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon_url: Option<String>,
    pub target: InstanceTarget,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checked_at_ms: Option<u64>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CatalogInstallResult {
    pub main_key: String,
    pub installed_keys: Vec<String>,
}

#[derive(Clone, Debug)]
struct ResolvedInstallItem {
    project_id: String,
    source: String,
    slug: Option<String>,
    version: ProviderVersion,
}

pub(crate) async fn search_catalog(
    app: AppHandle,
    request: CatalogSearchRequest,
) -> Result<CatalogSearchResponse, String> {
    let app_handle = app.clone();
    tauri::async_runtime::spawn_blocking(move || search_catalog_blocking(&app_handle, request))
        .await
        .map_err(|error| format!("Поиск в каталоге прерван: {error}"))?
}

pub(crate) async fn preview_catalog_install(
    app: AppHandle,
    request: CatalogInstallPreviewRequest,
) -> Result<CatalogInstallPreview, String> {
    let app_handle = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        preview_catalog_install_blocking(&app_handle, request)
    })
    .await
    .map_err(|error| format!("Подготовка установки прервана: {error}"))?
}

pub(crate) async fn catalog_project_details(
    app: AppHandle,
    request: CatalogProjectDetailsRequest,
) -> Result<CatalogProjectDetails, String> {
    let app_handle = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        catalog_project_details_blocking(&app_handle, request)
    })
    .await
    .map_err(|error| format!("Загрузка описания прервана: {error}"))?
}

pub(crate) async fn install_from_catalog(
    app: AppHandle,
    request: CatalogInstallRequest,
) -> Result<CatalogInstallResult, String> {
    let app_handle = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        install_from_catalog_blocking(&app_handle, request)
    })
    .await
    .map_err(|error| format!("Установка из каталога прервана: {error}"))?
}

fn search_catalog_blocking(
    app: &AppHandle,
    request: CatalogSearchRequest,
) -> Result<CatalogSearchResponse, String> {
    let source = normalize_source(&request.source)?;
    let query = request.query.trim();
    let settings = read_settings(app)?;
    let paths = resolve_paths(&settings)?;
    let target = detect_instance_target(&paths);

    let client = http_client().ok_or_else(|| "Не удалось создать HTTP-клиент.".to_string())?;
    let candidates = match source.as_str() {
        "modrinth" => search_catalog_modrinth(&client, query, &target),
        "curseforge" => {
            if settings.curseforge_api_key.trim().is_empty() {
                return Err("Для поиска на CurseForge нужен API key.".to_string());
            }
            search_catalog_curseforge(&client, &settings.curseforge_api_key, query, &target)
        }
        _ => unreachable!(),
    };
    Ok(CatalogSearchResponse { target, candidates })
}

fn catalog_project_details_blocking(
    app: &AppHandle,
    request: CatalogProjectDetailsRequest,
) -> Result<CatalogProjectDetails, String> {
    let source = normalize_source(&request.source)?;
    let project_id =
        clean_string(&request.project_id).ok_or_else(|| "Не задан проект каталога.".to_string())?;

    let settings = read_settings(app)?;
    let paths = resolve_paths(&settings)?;
    let cache_scope = paths.instance_root.to_string_lossy().to_string();
    if !request.force_refresh {
        if let Some(cached) =
            catalog_cache::read_project_details(app, &cache_scope, &source, &project_id)?
        {
            if project_details_cache_fresh(&cached) {
                return Ok(cached);
            }
        }
    }
    let target = detect_instance_target(&paths);
    let client = http_client().ok_or_else(|| "Не удалось создать HTTP-клиент.".to_string())?;
    let (title, slug, icon_url) = project_meta(&client, &settings, &source, &project_id)?;
    let description = fetch_catalog_project_description(&client, &settings, &source, &project_id);
    let details = CatalogProjectDetails {
        source,
        project_id,
        title,
        slug,
        icon_url,
        target,
        description,
        checked_at_ms: Some(now_millis()),
    };
    let cache_source = details.source.clone();
    let cache_project_id = details.project_id.clone();
    catalog_cache::write_project_details(
        app,
        &cache_scope,
        &cache_source,
        &cache_project_id,
        details.clone(),
    )?;
    Ok(details)
}

fn preview_catalog_install_blocking(
    app: &AppHandle,
    request: CatalogInstallPreviewRequest,
) -> Result<CatalogInstallPreview, String> {
    let source = normalize_source(&request.source)?;
    let project_id = clean_string(&request.project_id)
        .ok_or_else(|| "Не задан проект для установки.".to_string())?;

    let settings = read_settings(app)?;
    let paths = resolve_paths(&settings)?;
    let cache_scope = paths.instance_root.to_string_lossy().to_string();
    if !request.force_refresh {
        if let Some(cached) =
            catalog_cache::read_install_preview(app, &cache_scope, &source, &project_id)?
        {
            return Ok(cached);
        }
    }

    let catalog_root = catalog::catalog_root(app).ok();
    let mods = scan_mods_for_settings(&settings, catalog_root)?;
    let target = detect_instance_target(&paths);
    let client = http_client().ok_or_else(|| "Не удалось создать HTTP-клиент.".to_string())?;

    let (title, slug, icon_url) = project_meta(&client, &settings, &source, &project_id)?;
    let mut visiting = HashSet::new();
    let main = resolve_install_item(
        &client,
        &settings,
        &source,
        &project_id,
        slug.clone(),
        request.version_id.as_deref(),
        &target,
        0,
        &mut visiting,
    )?;
    visiting.clear();

    let mut listed = HashSet::new();
    let dependencies = collect_dependency_statuses(
        &mods,
        &source,
        &main,
        &client,
        &settings,
        &target,
        &mut listed,
        &mut visiting,
    )?;
    let preview = CatalogInstallPreview {
        source,
        project_id,
        title,
        slug,
        icon_url,
        target,
        version: main.version,
        dependencies,
        description: None,
        checked_at_ms: Some(now_millis()),
    };
    let cache_source = preview.source.clone();
    let cache_project_id = preview.project_id.clone();
    catalog_cache::write_install_preview(
        app,
        &cache_scope,
        &cache_source,
        &cache_project_id,
        preview.clone(),
    )?;
    Ok(preview)
}

fn install_from_catalog_blocking(
    app: &AppHandle,
    request: CatalogInstallRequest,
) -> Result<CatalogInstallResult, String> {
    let source = normalize_source(&request.source)?;
    let settings = read_settings(app)?;
    let paths = resolve_paths(&settings)?;
    let catalog_root = catalog::catalog_root(app).ok();
    let mods = scan_mods_for_settings(&settings, catalog_root)?;
    let target = detect_instance_target(&paths);
    let client = http_client().ok_or_else(|| "Не удалось создать HTTP-клиент.".to_string())?;
    let project_id = clean_string(&request.project_id)
        .ok_or_else(|| "Не задан проект для установки.".to_string())?;

    let (_title, slug, _) = project_meta(&client, &settings, &source, &project_id)?;
    let mut visiting = HashSet::new();
    let main = resolve_install_item(
        &client,
        &settings,
        &source,
        &project_id,
        slug,
        request.version_id.as_deref(),
        &target,
        0,
        &mut visiting,
    )?;
    visiting.clear();

    let mut plan = Vec::new();
    let mut planned = HashSet::new();
    collect_pending_installs(
        &paths,
        &mods,
        &source,
        &main,
        &client,
        &settings,
        &target,
        &mut plan,
        &mut planned,
        &mut visiting,
    )?;
    plan.push(main.clone());

    mods_watch::suppress_events_for(Duration::from_secs(120));
    let install_dir = paths.install_mods_dir();
    let mut installed_keys = Vec::new();

    for item in &plan {
        if paths.resolve_mod_jar(&item.version.filename).is_some() {
            let key = installed_key_for_project(&mods, &item.source, &item.project_id)
                .unwrap_or_else(|| stable_key(&item.version.filename, None));
            write_catalog_tag(&paths, &key, item)?;
            if !installed_keys.contains(&key) {
                installed_keys.push(key);
            }
            continue;
        }
        install_item_jar(&client, &settings, &install_dir, item)?;
        let key = stable_key_for_item(item);
        write_catalog_tag(&paths, &key, item)?;
        installed_keys.push(key);
    }
    mods_watch::suppress_events_for(Duration::from_secs(4));

    Ok(CatalogInstallResult {
        main_key: stable_key_for_item(&main),
        installed_keys,
    })
}

fn collect_pending_installs(
    paths: &InstancePaths,
    mods: &[ModEntry],
    source: &str,
    item: &ResolvedInstallItem,
    client: &reqwest::blocking::Client,
    settings: &Settings,
    target: &InstanceTarget,
    plan: &mut Vec<ResolvedInstallItem>,
    planned: &mut HashSet<String>,
    visiting: &mut HashSet<String>,
) -> Result<(), String> {
    for dep_id in
        required_dependency_ids(client, settings, source, &item.project_id, &item.version)?
    {
        if !planned.insert(dep_id.clone()) {
            continue;
        }
        let (title, slug, _) = project_meta(client, settings, source, &dep_id)
            .unwrap_or_else(|_| ("Зависимость".to_string(), None, None));
        let dep_item = resolve_install_item(
            client, settings, source, &dep_id, slug, None, target, 0, visiting,
        )?;
        if installed_key_for_dependency(
            mods,
            source,
            &dep_id,
            &title,
            Some(&dep_item.version.filename),
        )
        .is_some()
        {
            continue;
        }
        collect_pending_installs(
            paths, mods, source, &dep_item, client, settings, target, plan, planned, visiting,
        )?;
        if paths.resolve_mod_jar(&dep_item.version.filename).is_none() {
            plan.push(dep_item);
        }
    }
    Ok(())
}

fn build_dependency_preview(
    mods: &[ModEntry],
    source: &str,
    item: &ResolvedInstallItem,
    client: &reqwest::blocking::Client,
    settings: &Settings,
    target: &InstanceTarget,
    out: &mut Vec<CatalogInstallDependency>,
    listed: &mut HashSet<String>,
    visiting: &mut HashSet<String>,
) -> Result<(), String> {
    for dep_id in
        required_dependency_ids(client, settings, source, &item.project_id, &item.version)?
    {
        if !listed.insert(dep_id.clone()) {
            continue;
        }
        let title = project_meta(client, settings, source, &dep_id)
            .map(|meta| meta.0)
            .unwrap_or_else(|_| "Зависимость".to_string());
        let dep_item = resolve_install_item(
            client, settings, source, &dep_id, None, None, target, 0, visiting,
        )?;
        if let Some(key) = installed_key_for_dependency(
            mods,
            source,
            &dep_id,
            &title,
            Some(&dep_item.version.filename),
        ) {
            let filename = mods
                .iter()
                .find(|item| item.key == key)
                .map(|item| item.filename.clone());
            out.push(CatalogInstallDependency {
                project_id: dep_id.clone(),
                title,
                status: "installed".to_string(),
                key: Some(key),
                filename,
            });
            continue;
        }
        out.push(CatalogInstallDependency {
            project_id: dep_id.clone(),
            title: title.clone(),
            status: "pending".to_string(),
            key: None,
            filename: None,
        });
        build_dependency_preview(
            mods, source, &dep_item, client, settings, target, out, listed, visiting,
        )?;
    }
    Ok(())
}

fn collect_dependency_statuses(
    mods: &[ModEntry],
    source: &str,
    main: &ResolvedInstallItem,
    client: &reqwest::blocking::Client,
    settings: &Settings,
    target: &InstanceTarget,
    listed: &mut HashSet<String>,
    visiting: &mut HashSet<String>,
) -> Result<Vec<CatalogInstallDependency>, String> {
    let mut out = Vec::new();
    build_dependency_preview(
        mods, source, main, client, settings, target, &mut out, listed, visiting,
    )?;
    Ok(out)
}

fn resolve_install_item(
    client: &reqwest::blocking::Client,
    settings: &Settings,
    source: &str,
    project_id: &str,
    slug: Option<String>,
    version_id: Option<&str>,
    target: &InstanceTarget,
    depth: usize,
    visiting: &mut HashSet<String>,
) -> Result<ResolvedInstallItem, String> {
    if depth > MAX_DEPENDENCY_DEPTH {
        return Err("Слишком глубокая цепочка зависимостей.".to_string());
    }
    if !visiting.insert(project_id.to_string()) {
        return Err(
            "Modrinth/CurseForge указали зависимость по кругу (мод A требует B, B требует A). \
             Установи зависимости вручную."
                .to_string(),
        );
    }

    let version = if let Some(version_id) = version_id.and_then(clean_string) {
        version_by_id(client, settings, source, project_id, &version_id)?
    } else {
        pick_best_version(client, settings, source, project_id, target)?
    };
    visiting.remove(project_id);

    Ok(ResolvedInstallItem {
        project_id: project_id.to_string(),
        source: source.to_string(),
        slug,
        version,
    })
}

fn pick_best_version(
    client: &reqwest::blocking::Client,
    settings: &Settings,
    source: &str,
    project_id: &str,
    target: &InstanceTarget,
) -> Result<ProviderVersion, String> {
    let mut versions = match source {
        "modrinth" => list_modrinth_versions(client, project_id, target)?,
        "curseforge" => list_curseforge_versions(client, settings, project_id, target)?,
        _ => unreachable!(),
    };
    if target_has_filters(target) {
        versions.retain(|version| {
            version_matches_target(&version.game_versions, &version.loaders, target)
        });
    }
    versions.sort_by(|left, right| right.date_published.cmp(&left.date_published));
    versions
        .into_iter()
        .next()
        .ok_or_else(|| "Нет версии под текущую сборку.".to_string())
}

fn version_by_id(
    client: &reqwest::blocking::Client,
    settings: &Settings,
    source: &str,
    project_id: &str,
    version_id: &str,
) -> Result<ProviderVersion, String> {
    match source {
        "modrinth" => {
            let payload = modrinth_version(client, version_id)
                .ok_or_else(|| "Версия Modrinth не найдена.".to_string())?;
            super::versions::modrinth_version_from_json(&payload)
                .ok_or_else(|| "Modrinth вернул неожиданный ответ.".to_string())
        }
        "curseforge" => {
            let payload = curseforge_get(
                client,
                &settings.curseforge_api_key,
                &format!("mods/{project_id}/files/{version_id}"),
            )
            .ok_or_else(|| "Версия CurseForge не найдена.".to_string())?;
            let item = payload
                .get("data")
                .ok_or_else(|| "CurseForge вернул неожиданный ответ.".to_string())?;
            super::versions::curseforge_version_from_json(item)
                .ok_or_else(|| "CurseForge вернул неожиданный ответ.".to_string())
        }
        _ => unreachable!(),
    }
}

fn required_dependency_ids(
    client: &reqwest::blocking::Client,
    settings: &Settings,
    source: &str,
    project_id: &str,
    version: &ProviderVersion,
) -> Result<Vec<String>, String> {
    match source {
        "modrinth" => {
            let payload = modrinth_version(client, &version.id)
                .ok_or_else(|| "Не удалось прочитать зависимости Modrinth.".to_string())?;
            Ok(modrinth_required_project_ids(&payload))
        }
        "curseforge" => {
            let file_id = version.file_id.as_deref().unwrap_or(version.id.as_str());
            let payload = curseforge_get(
                client,
                &settings.curseforge_api_key,
                &format!("mods/{project_id}/files/{file_id}"),
            )
            .ok_or_else(|| "Не удалось прочитать зависимости CurseForge.".to_string())?;
            Ok(curseforge_required_mod_ids(&payload))
        }
        _ => unreachable!(),
    }
}

fn modrinth_required_project_ids(payload: &serde_json::Value) -> Vec<String> {
    let mut ids = Vec::new();
    let Some(items) = payload
        .get("dependencies")
        .and_then(|value| value.as_array())
    else {
        return ids;
    };
    for dep in items {
        let required = dep
            .get("dependency_type")
            .and_then(|value| value.as_str())
            .is_some_and(|kind| kind == "required");
        if !required {
            continue;
        }
        if let Some(project_id) = dep.get("project_id").and_then(|value| value.as_str()) {
            ids.push(project_id.to_string());
        }
    }
    ids
}

fn curseforge_required_mod_ids(payload: &serde_json::Value) -> Vec<String> {
    let mut ids = Vec::new();
    let Some(items) = payload
        .get("data")
        .and_then(|data| data.get("dependencies"))
        .and_then(|value| value.as_array())
    else {
        return ids;
    };
    for dep in items {
        let required = dep
            .get("relationType")
            .and_then(|value| value.as_i64())
            .is_some_and(|kind| kind == 3);
        if !required {
            continue;
        }
        if let Some(mod_id) = dep.get("modId").and_then(|value| value.as_i64()) {
            ids.push(mod_id.to_string());
        }
    }
    ids
}

fn installed_key_for_project(mods: &[ModEntry], source: &str, project_id: &str) -> Option<String> {
    mods.iter().find_map(|item| match source {
        "modrinth" if item.modrinth_id.as_deref() == Some(project_id) => Some(item.key.clone()),
        "curseforge" if item.curseforge_id.as_deref() == Some(project_id) => Some(item.key.clone()),
        _ => None,
    })
}

fn installed_key_for_dependency(
    mods: &[ModEntry],
    source: &str,
    project_id: &str,
    title: &str,
    filename: Option<&str>,
) -> Option<String> {
    if let Some(key) = installed_key_for_project(mods, source, project_id) {
        return Some(key);
    }
    for item in mods {
        if item.modrinth_id.as_deref() == Some(project_id)
            || item.curseforge_id.as_deref() == Some(project_id)
        {
            return Some(item.key.clone());
        }
    }
    if let Some(filename) = filename.and_then(clean_string) {
        for item in mods {
            if item.filename == filename {
                return Some(item.key.clone());
            }
        }
    }
    let title_key = normalized_match_key(title);
    if !title_key.is_empty() {
        for item in mods {
            if normalized_match_key(&item.display_name) == title_key {
                return Some(item.key.clone());
            }
        }
    }
    None
}

fn install_item_jar(
    client: &reqwest::blocking::Client,
    settings: &Settings,
    install_dir: &Path,
    item: &ResolvedInstallItem,
) -> Result<(), String> {
    let filename = sanitize_download_filename(&item.version.filename)?;
    let destination = install_dir.join(&filename);
    if destination.exists() {
        return Err(format!("Файл {filename} уже есть в папке mods."));
    }
    let download_url = catalog_download_url(client, settings, item)?;
    let temp_path = install_dir.join(format!(".mod-manager-download-{}.jar", timestamp_millis()));
    download_file(client, &download_url, &temp_path)?;
    fs::rename(&temp_path, &destination).map_err(|error| error.to_string())?;
    Ok(())
}

fn catalog_download_url(
    client: &reqwest::blocking::Client,
    settings: &Settings,
    item: &ResolvedInstallItem,
) -> Result<String, String> {
    if let Some(url) = item.version.download_url.as_deref().and_then(clean_string) {
        return Ok(url);
    }
    if item.source != "curseforge" {
        return Err("У выбранной версии нет ссылки на скачивание.".to_string());
    }
    let file_id = item
        .version
        .file_id
        .as_deref()
        .and_then(clean_string)
        .or_else(|| clean_string(&item.version.id))
        .ok_or_else(|| "У версии CurseForge нет file id.".to_string())?;
    let payload = curseforge_get(
        client,
        &settings.curseforge_api_key,
        &format!("mods/{}/files/{}/download-url", item.project_id, file_id),
    )
    .ok_or_else(|| "CurseForge не вернул ссылку на скачивание.".to_string())?;
    payload
        .get("data")
        .and_then(|value| value.as_str())
        .and_then(clean_string)
        .ok_or_else(|| "У этой версии CurseForge нет доступной ссылки на скачивание.".to_string())
}

fn write_catalog_tag(
    paths: &InstancePaths,
    key: &str,
    item: &ResolvedInstallItem,
) -> Result<(), String> {
    let mut tags = read_tags(&paths.tags_path)?;
    let tag = tags.mods.entry(key.to_string()).or_default();
    tag.source = item.source.clone();
    match item.source.as_str() {
        "modrinth" => {
            tag.modrinth_id = item.project_id.clone();
            tag.modrinth_version_id = item.version.id.clone();
        }
        "curseforge" => {
            tag.curseforge_id = item.project_id.clone();
            tag.curseforge_file_id = item
                .version
                .file_id
                .clone()
                .unwrap_or_else(|| item.version.id.clone());
            if let Some(slug) = item.slug.as_deref().and_then(clean_string) {
                tag.curseforge_slug = slug;
            }
        }
        _ => {}
    }
    let filename = item.version.filename.trim();
    if !filename.is_empty() && !tag.aliases.iter().any(|alias| alias == filename) {
        tag.aliases.push(filename.to_string());
    }
    tag.updated_at = now_iso();
    tags.updated_at = now_iso();
    write_tags(&paths.tags_path, &tags)
}

fn stable_key_for_item(item: &ResolvedInstallItem) -> String {
    match item.source.as_str() {
        "modrinth" => format!("modrinth:{}", item.project_id),
        "curseforge" => format!("curseforge:{}", item.project_id),
        _ => stable_key(&item.version.filename, None),
    }
}

fn project_meta(
    client: &reqwest::blocking::Client,
    settings: &Settings,
    source: &str,
    project_id: &str,
) -> Result<(String, Option<String>, Option<String>), String> {
    match source {
        "modrinth" => {
            let payload = modrinth_project(client, project_id)
                .ok_or_else(|| "Проект Modrinth не найден.".to_string())?;
            let title = payload
                .get("title")
                .and_then(|value| value.as_str())
                .map(str::to_string)
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| "Без названия".to_string());
            let slug = payload
                .get("slug")
                .and_then(|value| value.as_str())
                .map(str::to_string);
            let icon_url = payload
                .get("icon_url")
                .and_then(|value| value.as_str())
                .map(str::to_string)
                .filter(|value| !value.is_empty());
            Ok((title, slug, icon_url))
        }
        "curseforge" => {
            let project = curseforge_mod_info(client, &settings.curseforge_api_key, project_id)
                .ok_or_else(|| "Проект CurseForge не найден.".to_string())?;
            Ok((
                project.title.unwrap_or_else(|| "Без названия".to_string()),
                project.slug,
                None,
            ))
        }
        _ => unreachable!(),
    }
}

fn normalize_source(source: &str) -> Result<String, String> {
    let source = source.trim().to_ascii_lowercase();
    match source.as_str() {
        "modrinth" | "curseforge" => Ok(source),
        _ => Err("Каталог доступен только для Modrinth или CurseForge.".to_string()),
    }
}

fn clean_string(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn sanitize_download_filename(filename: &str) -> Result<String, String> {
    let trimmed = filename.trim();
    if trimmed.is_empty() {
        return Err("Поставщик не вернул имя файла.".to_string());
    }
    let path = Path::new(trimmed);
    if path.file_name().and_then(|value| value.to_str()) != Some(trimmed) {
        return Err("Поставщик вернул небезопасное имя файла.".to_string());
    }
    if path.extension().and_then(|value| value.to_str()) != Some("jar") {
        return Err("Можно устанавливать только jar-файлы модов.".to_string());
    }
    Ok(trimmed.to_string())
}

fn timestamp_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn curseforge_required_mod_ids_keeps_only_required_dependencies() {
        let payload = serde_json::json!({
            "data": {
                "dependencies": [
                    { "modId": 238222, "relationType": 3 },
                    { "modId": 306612, "relationType": 2 },
                    { "modId": 890303, "relationType": 3 }
                ]
            }
        });

        assert_eq!(
            curseforge_required_mod_ids(&payload),
            vec!["238222".to_string(), "890303".to_string()]
        );
    }
}
