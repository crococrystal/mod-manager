use tauri::AppHandle;

use crate::{
    catalog,
    mods::{scan_mods_for_settings, UNKNOWN_SIDE},
    provider_labels::{fetch_and_store_provider_labels, resolve_side},
    settings::{ServerSyncSettings, Settings},
    tags::{read_tags, write_tags},
};

pub(super) fn clean(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

pub(super) fn normalize_remote_dir(value: &str) -> String {
    let mut trimmed = value.trim();
    while trimmed.len() >= 2 {
        let starts = trimmed.starts_with('"') || trimmed.starts_with('\'');
        let ends = trimmed.ends_with('"') || trimmed.ends_with('\'');
        if starts && ends {
            trimmed = trimmed[1..trimmed.len() - 1].trim();
        } else {
            break;
        }
    }
    trimmed.replace('\\', "/")
}

pub(super) fn normalize_remote_path(path: &str) -> String {
    normalize_remote_dir(path)
}

pub(super) fn clean_remote_dir(value: &str) -> Option<String> {
    let normalized = normalize_remote_dir(value);
    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

pub(super) fn join_remote_path(dir: &str, filename: &str) -> String {
    let base = normalize_remote_path(dir).trim_end_matches('/').to_string();
    format!("{base}/{filename}")
}

pub(super) fn sync_config(settings: &Settings) -> Option<ServerSyncSettings> {
    let config = settings.server_sync.clone();
    if !config.enabled {
        return None;
    }
    let host = clean(&config.ssh_host)?.to_ascii_lowercase();
    if config.server_mods_path.trim().is_empty() && config.distribution_mods_path.trim().is_empty() {
        return None;
    }
    Some(ServerSyncSettings {
        enabled: true,
        ssh_host: host,
        server_mods_path: normalize_remote_dir(&config.server_mods_path),
        distribution_mods_path: normalize_remote_dir(&config.distribution_mods_path),
        delete_extra_remote_jars: config.delete_extra_remote_jars,
        server_os: config.server_os.clone(),
        server_start_script: config.server_start_script.clone(),
        server_root_path: config.server_root_path.clone(),
    })
}

pub(super) fn sync_config_error(settings: &Settings) -> String {
    let config = &settings.server_sync;
    if !config.enabled {
        return "Включите синхронизацию.".to_string();
    }
    if clean(&config.ssh_host).is_none() {
        return "Укажите SSH host.".to_string();
    }
    if config.server_mods_path.trim().is_empty() && config.distribution_mods_path.trim().is_empty() {
        return "Укажите путь mods.".to_string();
    }
    "Синхронизация не настроена.".to_string()
}

pub(super) fn resolve_ssh_host(settings: &Settings, override_host: Option<&str>) -> Option<String> {
    override_host
        .and_then(clean)
        .or_else(|| clean(&settings.server_sync.ssh_host))
        .map(|host| host.to_ascii_lowercase())
}

fn mod_side(paths: &crate::settings::InstancePaths, key: &str) -> String {
    read_tags(&paths.tags_path)
        .ok()
        .and_then(|tags| tags.mods.get(key).cloned())
        .map(|tag| resolve_side(&tag))
        .unwrap_or_else(|| UNKNOWN_SIDE.to_string())
}

pub(super) fn ensure_sync_side_labels(
    app: &AppHandle,
    paths: &crate::settings::InstancePaths,
    settings: &Settings,
    key: &str,
) {
    let Ok(mut tags) = read_tags(&paths.tags_path) else {
        return;
    };
    let labels_missing = tags
        .mods
        .get(key)
        .map(|tag| tag.provider_labels.fetched_at.is_empty())
        .unwrap_or(true);
    if !labels_missing {
        return;
    }
    let source = tags
        .mods
        .get(key)
        .map(|tag| tag.source.clone())
        .unwrap_or_default();
    if source != "modrinth" && source != "curseforge" {
        return;
    }
    let catalog_root = catalog::catalog_root(app).ok();
    let Ok(mods) = scan_mods_for_settings(settings, catalog_root) else {
        return;
    };
    let Some(item) = mods.iter().find(|entry| entry.key == key) else {
        return;
    };
    let tag = tags.mods.entry(key.to_string()).or_default();
    if fetch_and_store_provider_labels(tag, item, settings).is_ok() {
        let _ = write_tags(&paths.tags_path, &tags);
    }
}

fn should_defer_server_upload(paths: &crate::settings::InstancePaths, key: &str, side: &str) -> bool {
    if side == UNKNOWN_SIDE {
        return true;
    }
    if side != "universal" {
        return false;
    }
    read_tags(&paths.tags_path)
        .ok()
        .and_then(|tags| tags.mods.get(key).cloned())
        .map(|tag| {
            tag.provider_labels.fetched_at.is_empty()
                && matches!(tag.source.as_str(), "modrinth" | "curseforge")
        })
        .unwrap_or(false)
}

pub(crate) fn sync_upload_side<'a>(
    paths: &crate::settings::InstancePaths,
    key: &str,
    side: &'a str,
) -> &'a str {
    if side == UNKNOWN_SIDE || should_defer_server_upload(paths, key, side) {
        "client"
    } else {
        side
    }
}

pub(super) fn mod_side_for_key(paths: &crate::settings::InstancePaths, key: &str) -> String {
    mod_side(paths, key)
}
