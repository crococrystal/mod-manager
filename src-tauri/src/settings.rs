use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};
use tauri::{AppHandle, Manager};

use crate::instance_registry;

fn default_true() -> bool {
    true
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Settings {
    #[serde(default)]
    pub instance_root: Option<String>,
    #[serde(default)]
    pub curseforge_api_key: String,
    #[serde(default = "default_true")]
    pub auto_prefetch_covers: bool,
    #[serde(default = "default_true")]
    pub auto_prefetch_dependencies: bool,
    #[serde(default = "default_true")]
    pub auto_check_updates: bool,
    #[serde(default)]
    pub recent_instances: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            instance_root: None,
            curseforge_api_key: String::new(),
            auto_prefetch_covers: true,
            auto_prefetch_dependencies: true,
            auto_check_updates: true,
            recent_instances: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SettingsView {
    pub instance_root: Option<String>,
    pub mods_dir: Option<String>,
    pub data_root: Option<String>,
    pub curseforge_api_key: String,
    pub curseforge_api_key_set: bool,
    pub auto_prefetch_covers: bool,
    pub auto_prefetch_dependencies: bool,
    pub auto_check_updates: bool,
    pub recent_instances: Vec<String>,
    pub cache_status: Option<instance_registry::InstanceCacheStatus>,
}

#[derive(Clone, Debug)]
pub(crate) struct InstancePaths {
    pub instance_root: PathBuf,
    pub mods_dir: PathBuf,
    pub index_dir: PathBuf,
    pub data_root: PathBuf,
    pub tags_path: PathBuf,
}

pub(crate) fn app_settings_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|error| error.to_string())?;
    fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
    Ok(dir.join("settings.json"))
}

pub(crate) fn read_settings(app: &AppHandle) -> Result<Settings, String> {
    let path = app_settings_path(app)?;
    if !path.exists() {
        return Ok(Settings::default());
    }
    let text = fs::read_to_string(path).map_err(|error| error.to_string())?;
    serde_json::from_str(&text).map_err(|error| error.to_string())
}

pub(crate) fn write_settings(app: &AppHandle, settings: &Settings) -> Result<(), String> {
    let path = app_settings_path(app)?;
    let text = serde_json::to_string_pretty(settings).map_err(|error| error.to_string())?;
    fs::write(path, format!("{text}\n")).map_err(|error| error.to_string())
}

pub(crate) fn remember_instance(settings: &mut Settings, instance_root: &str) {
    let mut recent: Vec<String> = settings
        .recent_instances
        .iter()
        .filter(|item| item.as_str() != instance_root)
        .cloned()
        .collect();
    recent.insert(0, instance_root.to_string());
    settings.recent_instances = recent.into_iter().take(12).collect();
}

pub(crate) fn resolve_paths(settings: &Settings) -> Result<InstancePaths, String> {
    let selected = settings
        .instance_root
        .as_ref()
        .map(PathBuf::from)
        .ok_or_else(|| "Выбери папку сборки в настройках.".to_string())?;

    let selected = selected.canonicalize().unwrap_or_else(|_| selected.clone());

    let (instance_root, mods_dir) = if selected.join("minecraft").join("mods").is_dir() {
        (selected.clone(), selected.join("minecraft").join("mods"))
    } else if selected.join("mods").is_dir() {
        let instance = selected
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| selected.clone());
        (instance, selected.join("mods"))
    } else if selected.file_name().and_then(|name| name.to_str()) == Some("mods") {
        let instance = selected
            .parent()
            .and_then(Path::parent)
            .map(Path::to_path_buf)
            .unwrap_or_else(|| selected.clone());
        (instance, selected.clone())
    } else {
        return Err("В выбранной папке не нашлась minecraft/mods.".to_string());
    };

    Ok(InstancePaths {
        instance_root: instance_root.clone(),
        index_dir: mods_dir.join(".index"),
        data_root: instance_root.join(".mod-manager"),
        tags_path: instance_root.join(".mod-manager").join("mod-tags.json"),
        mods_dir,
    })
}

pub(crate) fn settings_view(app: &AppHandle, settings: Settings) -> Result<SettingsView, String> {
    let paths = resolve_paths(&settings).ok();
    let fingerprint = paths
        .as_ref()
        .map(|value| instance_registry::mods_fingerprint(&value.mods_dir))
        .transpose()?
        .unwrap_or_default();
    let registry = instance_registry::read_registry(app)?;
    let cache_status = paths.as_ref().map(|value| {
        instance_registry::cache_status(
            &registry,
            Some(value.instance_root.as_path()),
            &fingerprint,
        )
    });
    Ok(SettingsView {
        instance_root: settings.instance_root.clone(),
        mods_dir: paths
            .as_ref()
            .map(|value| value.mods_dir.to_string_lossy().to_string()),
        data_root: paths
            .as_ref()
            .map(|value| value.data_root.to_string_lossy().to_string()),
        curseforge_api_key: settings.curseforge_api_key.clone(),
        curseforge_api_key_set: !settings.curseforge_api_key.trim().is_empty(),
        auto_prefetch_covers: settings.auto_prefetch_covers,
        auto_prefetch_dependencies: settings.auto_prefetch_dependencies,
        auto_check_updates: settings.auto_check_updates,
        recent_instances: settings.recent_instances.clone(),
        cache_status,
    })
}
