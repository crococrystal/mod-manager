use std::{
    fs,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::instance_meta::{
    detect_instance_target, target_has_filters, version_matches_target, InstanceTarget,
};
use crate::mod_names::installed_version_from_filename;
use crate::mods_watch;
use crate::remote::{curseforge_get, http_client};
use crate::settings::{read_settings, resolve_paths, InstancePaths, Settings};
use crate::tags::{read_tags, write_tags};
use crate::util::{now_iso, system_time_iso};

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ListProviderVersionsRequest {
    pub key: String,
    pub source: String,
    #[serde(default)]
    pub project_id: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    pub filename: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct InstallProviderVersionRequest {
    pub key: String,
    pub source: String,
    pub project_id: String,
    pub filename: String,
    pub version_id: String,
    #[serde(default)]
    pub file_id: Option<String>,
    #[serde(default)]
    pub download_url: Option<String>,
    #[serde(default)]
    pub download_filename: Option<String>,
    #[serde(default)]
    pub version_number: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProviderVersion {
    pub id: String,
    pub file_id: Option<String>,
    pub version_number: String,
    pub name: String,
    pub filename: String,
    pub download_url: Option<String>,
    pub game_versions: Vec<String>,
    pub loaders: Vec<String>,
    pub date_published: Option<String>,
    pub downloads: Option<u64>,
    pub size: Option<u64>,
    pub release_type: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProviderVersionsPayload {
    pub target: InstanceTarget,
    pub versions: Vec<ProviderVersion>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct InstallProviderVersionResult {
    pub key: String,
    pub filename: String,
    pub base: String,
    pub modified_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub installed_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modrinth_version_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub curseforge_file_id: Option<String>,
}

pub(crate) async fn list_versions(
    app: AppHandle,
    request: ListProviderVersionsRequest,
) -> Result<ProviderVersionsPayload, String> {
    let app_handle = app.clone();
    tauri::async_runtime::spawn_blocking(move || list_versions_blocking(&app_handle, request))
        .await
        .map_err(|error| format!("Загрузка версий прервана: {error}"))?
}

pub(crate) async fn install_version(
    app: AppHandle,
    request: InstallProviderVersionRequest,
) -> Result<InstallProviderVersionResult, String> {
    let app_handle = app.clone();
    tauri::async_runtime::spawn_blocking(move || install_project(&app_handle, request))
        .await
        .map_err(|error| format!("Установка версии прервана: {error}"))?
}

fn clean_string(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn normalize_source(source: &str) -> Result<String, String> {
    let source = source.trim().to_ascii_lowercase();
    match source.as_str() {
        "modrinth" | "curseforge" => Ok(source),
        _ => Err("Версии доступны только для Modrinth или CurseForge.".to_string()),
    }
}

fn list_versions_blocking(
    app: &AppHandle,
    request: ListProviderVersionsRequest,
) -> Result<ProviderVersionsPayload, String> {
    let source = normalize_source(&request.source)?;
    let settings = read_settings(app)?;
    let paths = resolve_paths(&settings)?;
    let target = detect_instance_target(&paths);
    let client = http_client().ok_or_else(|| "Не удалось создать HTTP-клиент.".to_string())?;
    let project_id = request
        .project_id
        .as_deref()
        .and_then(clean_string)
        .or_else(|| project_id_from_tags(&paths, &request.key, &source))
        .ok_or_else(|| "Сначала выбери проект мода у поставщика.".to_string())?;

    let mut versions = match source.as_str() {
        "modrinth" => list_modrinth_versions(&client, &project_id, &target)?,
        "curseforge" => {
            if settings.curseforge_api_key.trim().is_empty() {
                return Err("Для CurseForge нужен API key.".to_string());
            }
            list_curseforge_versions(&client, &settings, &project_id, &target)?
        }
        _ => unreachable!(),
    };

    if target_has_filters(&target) {
        versions.retain(|version| {
            version_matches_target(&version.game_versions, &version.loaders, &target)
        });
    }

    versions.sort_by(|left, right| right.date_published.cmp(&left.date_published));
    Ok(ProviderVersionsPayload { target, versions })
}

fn project_id_from_tags(paths: &InstancePaths, key: &str, source: &str) -> Option<String> {
    let tags = read_tags(&paths.tags_path).ok()?;
    let tag = tags.mods.get(key)?;
    match source {
        "modrinth" => clean_string(&tag.modrinth_id),
        "curseforge" => clean_string(&tag.curseforge_id),
        _ => None,
    }
}

pub(crate) fn list_modrinth_versions(
    client: &reqwest::blocking::Client,
    project_id: &str,
    target: &InstanceTarget,
) -> Result<Vec<ProviderVersion>, String> {
    list_modrinth_versions_limited(client, project_id, target, None)
}

pub(crate) fn list_modrinth_versions_limited(
    client: &reqwest::blocking::Client,
    project_id: &str,
    target: &InstanceTarget,
    limit: Option<u32>,
) -> Result<Vec<ProviderVersion>, String> {
    let versions = fetch_modrinth_versions(client, project_id, target, limit)?;
    let loader_filtered = target
        .loader
        .as_deref()
        .map(str::trim)
        .is_some_and(|value| !value.is_empty());
    if !versions.is_empty() || !loader_filtered {
        return Ok(versions);
    }
    let fallback_target = InstanceTarget {
        minecraft_version: target.minecraft_version.clone(),
        loader: None,
    };
    fetch_modrinth_versions(client, project_id, &fallback_target, limit)
}

fn fetch_modrinth_versions(
    client: &reqwest::blocking::Client,
    project_id: &str,
    target: &InstanceTarget,
    limit: Option<u32>,
) -> Result<Vec<ProviderVersion>, String> {
    let mut request = client.get(format!(
        "https://api.modrinth.com/v2/project/{project_id}/version"
    ));
    let mut query = vec![("include_changelog", "false".to_string())];
    if let Some(limit) = limit {
        query.push(("limit", limit.to_string()));
    }
    if let Some(version) = target.minecraft_version.as_deref().and_then(clean_string) {
        query.push((
            "game_versions",
            serde_json::to_string(&vec![version]).map_err(|error| error.to_string())?,
        ));
    }
    if let Some(loader) = target.loader.as_deref().and_then(clean_string) {
        query.push((
            "loaders",
            serde_json::to_string(&vec![loader]).map_err(|error| error.to_string())?,
        ));
    }
    request = request.query(&query);
    let payload = request
        .send()
        .and_then(|response| response.error_for_status())
        .and_then(|response| response.json::<serde_json::Value>())
        .map_err(|error| format!("Не удалось загрузить версии Modrinth: {error}"))?;
    let items = payload
        .as_array()
        .ok_or_else(|| "Modrinth вернул неожиданный ответ.".to_string())?;

    Ok(items
        .iter()
        .filter_map(modrinth_version_from_json)
        .collect())
}

pub(crate) fn modrinth_version_from_json(item: &serde_json::Value) -> Option<ProviderVersion> {
    let id = item.get("id").and_then(|value| value.as_str())?.to_string();
    let version_number = item
        .get("version_number")
        .and_then(|value| value.as_str())
        .unwrap_or("Без версии")
        .to_string();
    let name = item
        .get("name")
        .and_then(|value| value.as_str())
        .unwrap_or(&version_number)
        .to_string();
    let files = item.get("files").and_then(|value| value.as_array())?;
    let file = files
        .iter()
        .find(|file| {
            file.get("primary")
                .and_then(|value| value.as_bool())
                .unwrap_or(false)
        })
        .or_else(|| files.first())?;
    let filename = file
        .get("filename")
        .and_then(|value| value.as_str())
        .unwrap_or(&version_number)
        .to_string();
    let download_url = file
        .get("url")
        .and_then(|value| value.as_str())
        .and_then(clean_string);
    Some(ProviderVersion {
        id,
        file_id: None,
        version_number,
        name,
        filename,
        download_url,
        game_versions: json_string_array(item.get("game_versions")),
        loaders: json_string_array(item.get("loaders")),
        date_published: item
            .get("date_published")
            .and_then(|value| value.as_str())
            .map(str::to_string),
        downloads: item.get("downloads").and_then(|value| value.as_u64()),
        size: file.get("size").and_then(|value| value.as_u64()),
        release_type: item
            .get("version_type")
            .and_then(|value| value.as_str())
            .map(str::to_string),
    })
}

pub(crate) fn list_curseforge_versions(
    client: &reqwest::blocking::Client,
    settings: &Settings,
    project_id: &str,
    target: &InstanceTarget,
) -> Result<Vec<ProviderVersion>, String> {
    list_curseforge_versions_limited(client, settings, project_id, target, None)
}

pub(crate) fn list_curseforge_versions_limited(
    client: &reqwest::blocking::Client,
    settings: &Settings,
    project_id: &str,
    target: &InstanceTarget,
    page_size: Option<u32>,
) -> Result<Vec<ProviderVersion>, String> {
    let page_size = page_size.unwrap_or(50);
    let mut path = format!("mods/{project_id}/files?pageSize={page_size}");
    if let Some(version) = target.minecraft_version.as_deref().and_then(clean_string) {
        path.push_str("&gameVersion=");
        path.push_str(&urlencoding::encode(&version));
    }
    if let Some(loader) = target
        .loader
        .as_deref()
        .and_then(clean_string)
        .and_then(|loader| curseforge_loader_type(&loader).map(|kind| kind.to_string()))
    {
        path.push_str("&modLoaderType=");
        path.push_str(&loader);
    }

    let payload = curseforge_get(client, &settings.curseforge_api_key, &path)
        .ok_or_else(|| "Не удалось загрузить версии CurseForge.".to_string())?;
    let items = payload
        .get("data")
        .and_then(|value| value.as_array())
        .ok_or_else(|| "CurseForge вернул неожиданный ответ.".to_string())?;

    Ok(items
        .iter()
        .filter_map(curseforge_version_from_json)
        .collect())
}

pub(crate) fn curseforge_version_from_json(item: &serde_json::Value) -> Option<ProviderVersion> {
    let file_id = item.get("id").and_then(|value| value.as_i64())?.to_string();
    let display_name = item
        .get("displayName")
        .or_else(|| item.get("name"))
        .and_then(|value| value.as_str())
        .unwrap_or("Без версии")
        .to_string();
    let filename = item
        .get("fileName")
        .and_then(|value| value.as_str())
        .unwrap_or(&display_name)
        .to_string();
    let game_versions = json_string_array(item.get("gameVersions"));
    let loaders = curseforge_loaders_from_versions(&game_versions);
    let minecraft_versions = game_versions
        .into_iter()
        .filter(|version| !is_loader_name(version))
        .collect();

    Some(ProviderVersion {
        id: file_id.clone(),
        file_id: Some(file_id),
        version_number: display_name.clone(),
        name: display_name,
        filename,
        download_url: item
            .get("downloadUrl")
            .and_then(|value| value.as_str())
            .and_then(clean_string),
        game_versions: minecraft_versions,
        loaders,
        date_published: item
            .get("fileDate")
            .and_then(|value| value.as_str())
            .map(str::to_string),
        downloads: item.get("downloadCount").and_then(|value| value.as_u64()),
        size: item
            .get("fileLength")
            .or_else(|| item.get("fileSizeOnDisk"))
            .and_then(|value| value.as_u64()),
        release_type: item
            .get("releaseType")
            .and_then(|value| value.as_u64())
            .map(curseforge_release_type),
    })
}

fn json_string_array(value: Option<&serde_json::Value>) -> Vec<String> {
    value
        .and_then(|value| value.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn curseforge_loader_type(loader: &str) -> Option<u8> {
    match loader.trim().to_ascii_lowercase().as_str() {
        "forge" => Some(1),
        "fabric" => Some(4),
        "quilt" => Some(5),
        "neoforge" => Some(6),
        _ => None,
    }
}

fn is_loader_name(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "forge" | "fabric" | "quilt" | "neoforge"
    )
}

fn curseforge_loaders_from_versions(versions: &[String]) -> Vec<String> {
    let mut loaders = Vec::new();
    for value in versions {
        let normalized = value.trim().to_ascii_lowercase();
        let loader = match normalized.as_str() {
            "forge" => "forge",
            "fabric" => "fabric",
            "quilt" => "quilt",
            "neoforge" => "neoforge",
            _ => continue,
        };
        if !loaders.iter().any(|item| item == loader) {
            loaders.push(loader.to_string());
        }
    }
    loaders
}

fn curseforge_release_type(value: u64) -> String {
    match value {
        1 => "release",
        2 => "beta",
        3 => "alpha",
        _ => "unknown",
    }
    .to_string()
}

fn install_project(
    app: &AppHandle,
    request: InstallProviderVersionRequest,
) -> Result<InstallProviderVersionResult, String> {
    let source = normalize_source(&request.source)?;
    let settings = read_settings(app)?;
    let paths = resolve_paths(&settings)?;
    let old_filename =
        clean_string(&request.filename).ok_or_else(|| "Не задан текущий файл мода.".to_string())?;
    let old_path = paths
        .resolve_mod_jar(&old_filename)
        .ok_or_else(|| "Текущий файл мода не найден в папке mods.".to_string())?;
    let install_dir = paths.mod_jar_dir(&old_filename);

    let client = http_client().ok_or_else(|| "Не удалось создать HTTP-клиент.".to_string())?;
    let download_url = resolve_download_url(&client, &settings, &request, &source)?;
    let next_filename = request
        .download_filename
        .as_deref()
        .and_then(clean_string)
        .unwrap_or_else(|| old_filename.clone());
    let next_filename = sanitize_download_filename(&next_filename)?;
    let next_path = install_dir.join(&next_filename);
    if next_path != old_path && next_path.exists() {
        return Err("Файл выбранной версии уже есть в папке mods.".to_string());
    }

    let temp_path = install_dir.join(format!(".mod-manager-download-{}.jar", timestamp_millis()));
    mods_watch::suppress_events_for(Duration::from_secs(60));
    download_file(&client, &download_url, &temp_path)?;
    replace_installed_file(&old_path, &next_path, &temp_path)?;
    mods_watch::suppress_events_for(Duration::from_secs(4));
    update_installed_tags(&paths, &request, &source, &old_filename, &next_filename)?;

    let modified_at = fs::metadata(&next_path)
        .and_then(|metadata| metadata.modified())
        .map(system_time_iso)
        .unwrap_or_else(|_| now_iso());
    let installed_version = request
        .version_number
        .as_deref()
        .and_then(clean_string)
        .or_else(|| installed_version_from_filename(&next_filename));
    let (modrinth_version_id, curseforge_file_id) = match source.as_str() {
        "modrinth" => (Some(request.version_id.trim().to_string()), None),
        "curseforge" => (
            None,
            Some(
                request
                    .file_id
                    .as_deref()
                    .and_then(clean_string)
                    .unwrap_or_else(|| request.version_id.trim().to_string()),
            ),
        ),
        _ => (None, None),
    };

    Ok(InstallProviderVersionResult {
        key: request.key,
        filename: next_filename.clone(),
        base: next_filename,
        modified_at,
        installed_version,
        modrinth_version_id,
        curseforge_file_id,
    })
}

fn resolve_download_url(
    client: &reqwest::blocking::Client,
    settings: &Settings,
    request: &InstallProviderVersionRequest,
    source: &str,
) -> Result<String, String> {
    if let Some(url) = request.download_url.as_deref().and_then(clean_string) {
        return Ok(url);
    }
    if source != "curseforge" {
        return Err("У выбранной версии нет ссылки на скачивание.".to_string());
    }
    let file_id = request
        .file_id
        .as_deref()
        .and_then(clean_string)
        .or_else(|| clean_string(&request.version_id))
        .ok_or_else(|| "У версии CurseForge нет file id.".to_string())?;
    let payload = curseforge_get(
        client,
        &settings.curseforge_api_key,
        &format!(
            "mods/{}/files/{}/download-url",
            request.project_id.trim(),
            file_id
        ),
    )
    .ok_or_else(|| "CurseForge не вернул ссылку на скачивание.".to_string())?;
    payload
        .get("data")
        .and_then(|value| value.as_str())
        .and_then(clean_string)
        .ok_or_else(|| "У этой версии CurseForge нет доступной ссылки на скачивание.".to_string())
}

pub(crate) fn download_file(
    client: &reqwest::blocking::Client,
    url: &str,
    destination: &Path,
) -> Result<(), String> {
    let mut response = client
        .get(url)
        .send()
        .and_then(|response| response.error_for_status())
        .map_err(|error| format!("Не удалось скачать файл: {error}"))?;
    let mut file = fs::File::create(destination).map_err(|error| error.to_string())?;
    response
        .copy_to(&mut file)
        .map_err(|error| format!("Не удалось сохранить файл: {error}"))?;
    Ok(())
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

fn replace_installed_file(
    old_path: &Path,
    next_path: &Path,
    temp_path: &Path,
) -> Result<(), String> {
    let backup_path = backup_path_for(old_path);
    fs::rename(old_path, &backup_path).map_err(|error| error.to_string())?;
    if let Err(error) = fs::rename(temp_path, next_path) {
        let _ = fs::rename(&backup_path, old_path);
        let _ = fs::remove_file(temp_path);
        return Err(error.to_string());
    }
    let _ = fs::remove_file(&backup_path);
    Ok(())
}

fn backup_path_for(path: &Path) -> PathBuf {
    let filename = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("mod.jar");
    path.with_file_name(format!(
        "{filename}.mod-manager-backup-{}",
        timestamp_millis()
    ))
}

fn timestamp_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}

fn update_installed_tags(
    paths: &InstancePaths,
    request: &InstallProviderVersionRequest,
    source: &str,
    old_filename: &str,
    next_filename: &str,
) -> Result<(), String> {
    let mut tags = read_tags(&paths.tags_path)?;
    let tag = tags.mods.entry(request.key.clone()).or_default();
    tag.source = source.to_string();
    match source {
        "modrinth" => {
            tag.modrinth_id = request.project_id.trim().to_string();
            tag.modrinth_version_id = request.version_id.trim().to_string();
        }
        "curseforge" => {
            tag.curseforge_id = request.project_id.trim().to_string();
            tag.curseforge_file_id = request
                .file_id
                .as_deref()
                .and_then(clean_string)
                .unwrap_or_else(|| request.version_id.trim().to_string());
        }
        _ => {}
    }
    for filename in [old_filename, next_filename] {
        if !tag.aliases.iter().any(|alias| alias == filename) {
            tag.aliases.push(filename.to_string());
        }
    }
    tag.updated_at = now_iso();
    tags.updated_at = now_iso();
    write_tags(&paths.tags_path, &tags)
}
