pub(crate) mod cleanup;
pub(crate) mod client;
pub(crate) mod detect;
mod fabric;
mod neoforged;
mod server;
mod ssh;

use std::path::Path;

use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::remote::http_client;
use crate::settings::{read_settings, resolve_paths};

fn download_client() -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(180))
        .user_agent("mod-manager/0.1.2")
        .build()
        .map_err(|error| error.to_string())
}

fn clean_host(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_ascii_lowercase())
    }
}

fn resolve_ssh_host(settings: &crate::settings::Settings, override_host: Option<&str>) -> Option<String> {
    override_host
        .and_then(clean_host)
        .or_else(|| clean_host(&settings.server_sync.ssh_host))
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NeoForgeCheckResult {
    pub ok: bool,
    pub minecraft_version: Option<String>,
    pub client_version: Option<String>,
    pub server_version: Option<String>,
    pub latest_version: Option<String>,
    #[serde(default)]
    pub available_versions: Vec<String>,
    pub client_needs_update: bool,
    pub server_needs_update: bool,
    pub client_server_match: bool,
    pub libraries_root: Option<String>,
    pub server_root: Option<String>,
    pub message: String,
    #[serde(default)]
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NeoForgeCheckRequest {
    #[serde(default)]
    pub ssh_host: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NeoForgeApplyResult {
    pub ok: bool,
    pub message: String,
    pub client_version: Option<String>,
    pub server_version: Option<String>,
    #[serde(default)]
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NeoForgeApplyRequest {
    #[serde(default)]
    pub target_version: Option<String>,
    #[serde(default)]
    pub update_client: bool,
    #[serde(default)]
    pub update_server: bool,
    #[serde(default)]
    pub ssh_host: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NeoForgeVersionCatalog {
    pub ok: bool,
    pub loader: String,
    pub apply_supported: bool,
    pub minecraft_version: Option<String>,
    pub latest_version: Option<String>,
    pub available_versions: Vec<String>,
    pub message: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RefreshNeoForgeRowRequest {
    pub row: String,
    #[serde(default)]
    pub ssh_host: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RefreshNeoForgeRowResult {
    pub row: String,
    pub version: Option<String>,
}

fn instance_root(settings: &crate::settings::Settings) -> Result<&str, String> {
    settings
        .instance_root
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "Сначала выберите инстанс.".to_string())
}

fn fetch_version_catalog(
    loader: &str,
    minecraft_version: &str,
) -> Result<(String, Vec<String>), String> {
    let client = http_client().ok_or_else(|| "HTTP недоступен.".to_string())?;
    match loader {
        "fabric" => fabric::fetch_version_catalog(&client, minecraft_version),
        "neoforge" => {
            let versions = neoforged::fetch_versions_for_mc(&client, minecraft_version)?;
            let latest = versions
                .iter()
                .max_by(|left, right| neoforged::compare_versions(left, right))
                .cloned()
                .ok_or_else(|| "Не найдены версии NeoForge.".to_string())?;
            Ok((latest, neoforged::versions_newest_first(&versions)))
        }
        _ => Err(format!(
            "Loader «{loader}» не поддерживается. Ожидается NeoForge или Fabric."
        )),
    }
}

fn loader_label(loader: &str) -> &'static str {
    match loader {
        "fabric" => "Fabric",
        _ => "NeoForge",
    }
}

fn validate_target_version(target: &str, available: &[String]) -> Result<(), String> {
    if available.iter().any(|version| version == target) {
        Ok(())
    } else {
        Err(format!("Версия {target} недоступна для этого Minecraft."))
    }
}

fn check_inner(
    settings: &crate::settings::Settings,
    ssh_override: Option<&str>,
) -> NeoForgeCheckResult {
    let mut warnings = Vec::new();

    let instance_root = match settings.instance_root.as_deref() {
        Some(value) if !value.trim().is_empty() => value.trim().to_string(),
        _ => {
            return NeoForgeCheckResult {
                ok: false,
                message: "Сначала выберите инстанс.".to_string(),
                ..Default::default()
            };
        }
    };

    let client_info = detect::detect_client(Path::new(&instance_root));
    let loader = client_info.loader.clone();
    let minecraft_version = client_info.minecraft_version.clone();
    let client_version = client_info.loader_version.clone();

    if minecraft_version.is_none() {
        warnings.push("Не найдена версия Minecraft в mmc-pack.json.".to_string());
    }
    if client_version.is_none() {
        let label = loader_label(if loader.is_empty() { "neoforge" } else { &loader });
        warnings.push(format!("Не найден {label} в mmc-pack.json."));
    }

    let effective_loader = if loader.is_empty() {
        "neoforge".to_string()
    } else {
        loader
    };

    let libraries_root = detect::resolve_libraries_root(Path::new(&instance_root));
    let libraries_root_text = libraries_root.to_string_lossy().into_owned();

    let server_mods_path = settings.server_sync.server_mods_path.trim();
    let server_root = detect::server_root_from_mods_path(server_mods_path);
    let ssh_host = resolve_ssh_host(settings, ssh_override);

    let mut server_version = None;
    if let (Some(host), Some(root)) = (ssh_host.as_deref(), server_root.as_deref()) {
        match detect::detect_server_version(host, root) {
            Ok(version) => server_version = version,
            Err(error) => warnings.push(format!("Сервер: {error}")),
        }
    } else if server_root.is_some() {
        warnings.push("Укажите SSH host для проверки сервера.".to_string());
    }

    let mut latest_version = None;
    let mut available_versions = Vec::new();
    if let Some(mc) = minecraft_version.as_deref() {
        match fetch_version_catalog(&effective_loader, mc) {
            Ok((latest, versions)) => {
                latest_version = Some(latest);
                available_versions = versions;
            }
            Err(error) => warnings.push(error),
        }
    }

    let client_needs_update = latest_version
        .as_deref()
        .is_some_and(|latest| detect::needs_update(client_version.as_deref(), latest));
    let server_needs_update = latest_version
        .as_deref()
        .is_some_and(|latest| detect::needs_update(server_version.as_deref(), latest));
    let client_server_match = match (client_version.as_deref(), server_version.as_deref()) {
        (Some(left), Some(right)) => left == right,
        _ => false,
    };

    let mut message_parts = Vec::new();
    if let Some(latest) = latest_version.as_deref() {
        message_parts.push(format!("Актуально: {latest}"));
    }
    if client_needs_update {
        message_parts.push("Клиент устарел.".to_string());
    }
    if server_needs_update {
        message_parts.push("Сервер устарел.".to_string());
    }
    if !client_needs_update && !server_needs_update && latest_version.is_some() {
        message_parts.push(format!("{} актуален.", loader_label(&effective_loader)));
    }
    if !client_server_match && client_version.is_some() && server_version.is_some() {
        message_parts.push("Версии клиента и сервера различаются.".to_string());
    }

    NeoForgeCheckResult {
        ok: minecraft_version.is_some() && latest_version.is_some(),
        minecraft_version,
        client_version,
        server_version,
        latest_version,
        available_versions,
        client_needs_update,
        server_needs_update,
        client_server_match,
        libraries_root: Some(libraries_root_text),
        server_root,
        message: if message_parts.is_empty() {
            format!("Проверка {}.", loader_label(&effective_loader))
        } else {
            message_parts.join(" ")
        },
        warnings,
    }
}

fn apply_inner(settings: &crate::settings::Settings, request: NeoForgeApplyRequest) -> Result<NeoForgeApplyResult, String> {
    if !request.update_client && !request.update_server {
        return Err("Выберите клиент и/или сервер.".to_string());
    }

    let client_info = detect::detect_client(Path::new(instance_root(settings)?));
    if client_info.loader == "fabric" {
        return Err(
            "Обновление Fabric пока не поддерживается — доступен только каталог версий."
                .to_string(),
        );
    }

    let check = check_inner(settings, request.ssh_host.as_deref());
    if !check.ok {
        return Err(check.message);
    }

    let target_version = request
        .target_version
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| check.latest_version.clone())
        .ok_or_else(|| "Не удалось определить целевую версию.".to_string())?;

    if !check.available_versions.is_empty() {
        validate_target_version(&target_version, &check.available_versions)?;
    }

    let instance_root = instance_root(settings)?;

    let http = download_client()?;
    let mut warnings = check.warnings.clone();
    let mut client_version = check.client_version.clone();
    let mut server_version = check.server_version.clone();

    if request.update_client {
        if client_version.as_deref() == Some(target_version.as_str()) {
            warnings.push(format!("Клиент уже на {target_version}."));
        } else {
            let libraries_root = detect::resolve_libraries_root(Path::new(instance_root));
            let result = client::apply_client(
                &http,
                Path::new(instance_root),
                &libraries_root,
                &target_version,
            )?;
            client_version = Some(target_version.clone());
            let removed: Vec<String> = result
                .downloaded_files
                .iter()
                .filter_map(|item| item.strip_prefix("removed:").map(str::to_string))
                .collect();
            let jars: Vec<String> = result
                .downloaded_files
                .iter()
                .filter(|item| !item.starts_with("removed:"))
                .cloned()
                .collect();
            warnings.push(format!(
                "Клиент: mmc-pack → {target_version}, jar: {}",
                jars.join(", ")
            ));
            if !removed.is_empty() {
                warnings.push(format!("Удалены installer (клиент): {}", removed.join(", ")));
            }
        }
    }

    if request.update_server {
        let host = resolve_ssh_host(settings, request.ssh_host.as_deref())
            .ok_or_else(|| "Укажите SSH host.".to_string())?;
        let server_root = check
            .server_root
            .as_deref()
            .ok_or_else(|| "Укажите путь server mods — из него выводится корень сервера.".to_string())?;

        if server_version.as_deref() == Some(target_version.as_str()) {
            warnings.push(format!("Сервер уже на {target_version}."));
        } else {
            let removed = server::apply_server(&http, &host, server_root, &target_version)?;
            server_version = Some(target_version.clone());
            warnings.push(format!(
                "Сервер: installer {target_version} выполнен в {server_root}."
            ));
            if !removed.is_empty() {
                warnings.push(format!("Удалены installer (сервер): {}", removed.join(", ")));
            }
        }
    }

    let message = format!("NeoForge обновлён до {target_version}.");
    Ok(NeoForgeApplyResult {
        ok: true,
        message,
        client_version,
        server_version,
        warnings,
    })
}

impl Default for NeoForgeCheckResult {
    fn default() -> Self {
        Self {
            ok: false,
            minecraft_version: None,
            client_version: None,
            server_version: None,
            latest_version: None,
            available_versions: Vec::new(),
            client_needs_update: false,
            server_needs_update: false,
            client_server_match: false,
            libraries_root: None,
            server_root: None,
            message: String::new(),
            warnings: Vec::new(),
        }
    }
}

impl Default for NeoForgeVersionCatalog {
    fn default() -> Self {
        Self {
            ok: false,
            loader: "neoforge".to_string(),
            apply_supported: true,
            minecraft_version: None,
            latest_version: None,
            available_versions: Vec::new(),
            message: String::new(),
        }
    }
}

#[tauri::command]
pub(crate) async fn check_neoforge_update(
    app: AppHandle,
    request: Option<NeoForgeCheckRequest>,
) -> Result<NeoForgeCheckResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let settings = read_settings(&app)?;
        let ssh_host = request.as_ref().and_then(|item| item.ssh_host.as_deref());
        Ok(check_inner(&settings, ssh_host))
    })
    .await
    .map_err(|error| format!("NeoForge: {error}"))?
}

#[tauri::command]
pub(crate) async fn apply_neoforge_update(
    app: AppHandle,
    request: NeoForgeApplyRequest,
) -> Result<NeoForgeApplyResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let settings = read_settings(&app)?;
        let _paths = resolve_paths(&settings)?;
        apply_inner(&settings, request)
    })
    .await
    .map_err(|error| format!("NeoForge: {error}"))?
}

fn catalog_inner(settings: &crate::settings::Settings) -> NeoForgeVersionCatalog {
    let instance_root = match instance_root(settings) {
        Ok(value) => value,
        Err(message) => {
            return NeoForgeVersionCatalog {
                ok: false,
                message,
                ..Default::default()
            };
        }
    };

    let client_info = detect::detect_client(Path::new(instance_root));
    let loader = if client_info.loader.is_empty() {
        "neoforge".to_string()
    } else {
        client_info.loader.clone()
    };
    let apply_supported = loader == "neoforge";
    let loader_name = loader_label(&loader);

    let Some(minecraft_version) = client_info.minecraft_version.clone() else {
        return NeoForgeVersionCatalog {
            ok: false,
            loader,
            apply_supported,
            message: "Не найдена версия Minecraft в mmc-pack.json.".to_string(),
            minecraft_version: None,
            ..Default::default()
        };
    };

    match fetch_version_catalog(&loader, &minecraft_version) {
        Ok((latest, versions)) => NeoForgeVersionCatalog {
            ok: true,
            loader: loader.clone(),
            apply_supported,
            minecraft_version: Some(minecraft_version),
            latest_version: Some(latest),
            available_versions: versions,
            message: if apply_supported {
                format!("Список версий {loader_name} загружен.")
            } else {
                format!(
                    "Список версий {loader_name} загружен. Обновление через mod-manager пока недоступно."
                )
            },
        },
        Err(message) => NeoForgeVersionCatalog {
            ok: false,
            loader,
            apply_supported,
            minecraft_version: Some(minecraft_version),
            message,
            ..Default::default()
        },
    }
}

fn refresh_row_inner(
    settings: &crate::settings::Settings,
    request: RefreshNeoForgeRowRequest,
) -> Result<RefreshNeoForgeRowResult, String> {
    let row = request.row.trim().to_ascii_lowercase();
    let version = match row.as_str() {
        "client" => {
            let root = instance_root(settings)?;
            Ok(detect::detect_client(Path::new(root)).loader_version)
        }
        "server" => {
            let root = instance_root(settings)?;
            let client_info = detect::detect_client(Path::new(root));
            if client_info.loader == "fabric" {
                return Err("Проверка версии Fabric-сервера пока не поддерживается.".to_string());
            }
            let host = resolve_ssh_host(settings, request.ssh_host.as_deref())
                .ok_or_else(|| "Укажите SSH host.".to_string())?;
            let server_root = detect::server_root_from_mods_path(settings.server_sync.server_mods_path.trim())
                .ok_or_else(|| "Укажите путь server mods.".to_string())?;
            detect::detect_server_version(&host, &server_root)
        }
        _ => Err("Неизвестная строка loader.".to_string()),
    }?;

    Ok(RefreshNeoForgeRowResult { row, version })
}

#[tauri::command]
pub(crate) async fn get_neoforge_version_catalog(app: AppHandle) -> Result<NeoForgeVersionCatalog, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let settings = read_settings(&app)?;
        Ok(catalog_inner(&settings))
    })
    .await
    .map_err(|error| format!("NeoForge: {error}"))?
}

#[tauri::command]
pub(crate) async fn refresh_neoforge_row(
    app: AppHandle,
    request: RefreshNeoForgeRowRequest,
) -> Result<RefreshNeoForgeRowResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let settings = read_settings(&app)?;
        refresh_row_inner(&settings, request)
    })
    .await
    .map_err(|error| format!("NeoForge: {error}"))?
}

#[cfg(test)]
mod tests {
    use super::detect;

    #[test]
    fn derives_server_root_from_mods_path() {
        assert_eq!(
            detect::server_root_from_mods_path("C:/Users/Admin/Desktop/Crystal Tech 1.21.1/mods")
                .as_deref(),
            Some("C:/Users/Admin/Desktop/Crystal Tech 1.21.1")
        );
    }
}
