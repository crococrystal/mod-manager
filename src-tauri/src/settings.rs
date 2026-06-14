use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};
use tauri::{AppHandle, Manager};

use crate::instance_registry;

fn default_true() -> bool {
    true
}

fn default_server_os() -> String {
    "auto".to_string()
}

#[derive(Clone, Debug, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ServerSyncSettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub ssh_host: String,
    #[serde(default)]
    pub server_mods_path: String,
    #[serde(default)]
    pub distribution_mods_path: String,
    #[serde(default = "default_true")]
    pub delete_extra_remote_jars: bool,
    #[serde(default = "default_server_os")]
    pub server_os: String,
    #[serde(default)]
    pub server_start_script: String,
    #[serde(default)]
    pub server_root_path: String,
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
    #[serde(default)]
    pub server_sync: ServerSyncSettings,
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
            server_sync: ServerSyncSettings::default(),
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
    pub server_sync: ServerSyncSettings,
    pub cache_status: Option<instance_registry::InstanceCacheStatus>,
}

#[derive(Clone, Debug)]
pub(crate) struct InstancePaths {
    pub instance_root: PathBuf,
    pub mods_dir: PathBuf,
    pub extra_mods_dirs: Vec<PathBuf>,
    pub index_dir: PathBuf,
    pub data_root: PathBuf,
    pub tags_path: PathBuf,
}

impl InstancePaths {
    pub(crate) fn all_mods_dirs(&self) -> impl Iterator<Item = &Path> {
        std::iter::once(self.mods_dir.as_path())
            .chain(self.extra_mods_dirs.iter().map(|path| path.as_path()))
    }

    pub(crate) fn mod_jar_candidates(&self, filename: &str) -> Vec<PathBuf> {
        let mut candidates = Vec::new();
        for dir in self.all_mods_dirs() {
            let path = dir.join(filename);
            if path.is_file() {
                candidates.push(path);
            }
        }
        candidates
    }

    pub(crate) fn resolve_mod_jar(&self, filename: &str) -> Option<PathBuf> {
        let candidates = self.mod_jar_candidates(filename);
        select_canonical_mod_jar(self, filename, &candidates)
    }

    pub(crate) fn mod_jar_dir(&self, filename: &str) -> PathBuf {
        self.resolve_mod_jar(filename)
            .and_then(|path| path.parent().map(Path::to_path_buf))
            .unwrap_or_else(|| self.mods_dir.clone())
    }

    pub(crate) fn install_mods_dir(&self) -> PathBuf {
        self.extra_mods_dirs
            .first()
            .cloned()
            .unwrap_or_else(|| self.mods_dir.clone())
    }
}

fn jar_modified_ms(path: &Path) -> Option<u128> {
    fs::metadata(path)
        .ok()?
        .modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_millis())
}

pub(crate) fn select_canonical_mod_jar(
    paths: &InstancePaths,
    filename: &str,
    candidates: &[PathBuf],
) -> Option<PathBuf> {
    if candidates.is_empty() {
        return None;
    }

    let primary = paths.mods_dir.join(filename);
    if primary.is_file() {
        return Some(primary);
    }

    let mut automodpack: Vec<PathBuf> = candidates
        .iter()
        .filter(|path| path.is_file() && *path != &primary)
        .cloned()
        .collect();
    automodpack.sort_by(|left, right| left.as_os_str().cmp(right.as_os_str()));

    if automodpack.is_empty() {
        return None;
    }

    let mut best_path = automodpack[0].clone();
    let mut best_mtime = jar_modified_ms(&best_path);
    for path in automodpack.iter().skip(1) {
        let mtime = jar_modified_ms(path);
        let replace = match (best_mtime, mtime) {
            (None, None) => path.as_os_str() < best_path.as_os_str(),
            (None, Some(_)) => true,
            (Some(_), None) => false,
            (Some(left), Some(right)) => {
                right > left || (right == left && path.as_os_str() < best_path.as_os_str())
            }
        };
        if replace {
            best_path = path.clone();
            best_mtime = mtime;
        }
    }

    Some(best_path)
}

fn selected_automodpack_modpack(minecraft_dir: &Path) -> Option<String> {
    let config = minecraft_dir
        .join("automodpack")
        .join("automodpack-client.json");
    let text = fs::read_to_string(config).ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    value
        .get("selectedModpack")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn discover_automodpack_mods_dirs(minecraft_dir: &Path) -> Vec<PathBuf> {
    let modpacks = minecraft_dir.join("automodpack").join("modpacks");
    let Ok(entries) = fs::read_dir(&modpacks) else {
        return Vec::new();
    };

    let mut dirs = Vec::new();
    for entry in entries.flatten() {
        let mods = entry.path().join("mods");
        if mods.is_dir() {
            dirs.push(mods);
        }
    }
    dirs.sort_by(|left, right| left.as_os_str().cmp(right.as_os_str()));
    if let Some(selected) = selected_automodpack_modpack(minecraft_dir) {
        let selected_mods = modpacks.join(selected).join("mods");
        if let Some(index) = dirs.iter().position(|path| path == &selected_mods) {
            let selected_dir = dirs.remove(index);
            dirs.insert(0, selected_dir);
        }
    }
    dirs
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
    } else if selected
        .join("automodpack")
        .join("host-modpack")
        .join("main")
        .join("mods")
        .is_dir()
    {
        (
            selected.clone(),
            selected
                .join("automodpack")
                .join("host-modpack")
                .join("main")
                .join("mods"),
        )
    } else if selected
        .join("host-modpack")
        .join("main")
        .join("mods")
        .is_dir()
    {
        (
            selected.clone(),
            selected.join("host-modpack").join("main").join("mods"),
        )
    } else if selected.join("main").join("mods").is_dir()
        && selected.file_name().and_then(|name| name.to_str()) == Some("host-modpack")
    {
        (selected.clone(), selected.join("main").join("mods"))
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

    let minecraft_dir = mods_dir
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| instance_root.clone());
    let extra_mods_dirs = discover_automodpack_mods_dirs(&minecraft_dir);

    Ok(InstancePaths {
        instance_root: instance_root.clone(),
        index_dir: mods_dir.join(".index"),
        data_root: instance_root.join(".mod-manager"),
        tags_path: instance_root.join(".mod-manager").join("mod-tags.json"),
        mods_dir,
        extra_mods_dirs,
    })
}

pub(crate) fn settings_view(app: &AppHandle, settings: Settings) -> Result<SettingsView, String> {
    let paths = resolve_paths(&settings).ok();
    let fingerprint = paths
        .as_ref()
        .map(|value| instance_registry::mods_fingerprint_dirs(value.all_mods_dirs()))
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
        server_sync: settings.server_sync.clone(),
        cache_status,
    })
}
