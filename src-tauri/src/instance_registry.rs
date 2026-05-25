use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    time::SystemTime,
};

use tauri::{AppHandle, Manager};

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceRegistry {
    #[serde(default = "registry_version")]
    pub version: u8,
    #[serde(default)]
    pub instances: HashMap<String, InstanceRecord>,
}

fn registry_version() -> u8 {
    1
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceRecord {
    pub instance_root: String,
    pub display_name: String,
    pub mods_fingerprint: String,
    pub covers_ready: bool,
    pub dependencies_ready: bool,
    pub last_prepared_at: String,
    pub last_opened_at: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceCacheStatus {
    pub instance_root: Option<String>,
    pub display_name: Option<String>,
    pub mods_fingerprint: String,
    pub covers_ready: bool,
    pub dependencies_ready: bool,
    pub ready: bool,
    pub needs_covers: bool,
    pub needs_dependencies: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BootstrapResult {
    pub skipped: bool,
    pub ran_covers: bool,
    pub ran_dependencies: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClearDataResult {
    pub removed_catalog_files: u32,
    pub cleared_instances: u32,
    pub cleared_instance_dirs: u32,
}

pub fn registry_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join("instance-registry.json"))
}

pub fn read_registry(app: &AppHandle) -> Result<InstanceRegistry, String> {
    let path = registry_path(app)?;
    if !path.exists() {
        return Ok(InstanceRegistry::default());
    }
    let text = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    serde_json::from_str(&text).map_err(|e| e.to_string())
}

pub fn write_registry(app: &AppHandle, registry: &InstanceRegistry) -> Result<(), String> {
    let path = registry_path(app)?;
    let text = serde_json::to_string_pretty(registry).map_err(|e| e.to_string())?;
    fs::write(path, format!("{text}\n")).map_err(|e| e.to_string())
}

pub fn registry_key(instance_root: &Path) -> String {
    instance_root
        .canonicalize()
        .unwrap_or_else(|_| instance_root.to_path_buf())
        .to_string_lossy()
        .to_string()
}

pub fn display_name_for(instance_root: &Path) -> String {
    instance_root
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("Сборка")
        .to_string()
}

pub fn mods_fingerprint(mods_dir: &Path) -> Result<String, String> {
    let mut parts = Vec::new();
    let entries = fs::read_dir(mods_dir).map_err(|e| e.to_string())?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("jar") {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        let mtime_ms = fs::metadata(&path)
            .ok()
            .and_then(|meta| meta.modified().ok())
            .and_then(|time| time.duration_since(SystemTime::UNIX_EPOCH).ok())
            .map(|duration| duration.as_millis())
            .unwrap_or(0);
        parts.push(format!("{name}:{mtime_ms}"));
    }
    parts.sort();
    let digest = Sha256::digest(parts.join("\n").as_bytes());
    Ok(format!("{:x}", digest))
}

pub fn touch_opened(registry: &mut InstanceRegistry, instance_root: &Path, now: &str) {
    let key = registry_key(instance_root);
    let display_name = display_name_for(instance_root);
    let record = registry.instances.entry(key).or_insert_with(|| InstanceRecord {
        instance_root: instance_root.to_string_lossy().to_string(),
        display_name: display_name.clone(),
        mods_fingerprint: String::new(),
        covers_ready: false,
        dependencies_ready: false,
        last_prepared_at: String::new(),
        last_opened_at: now.to_string(),
    });
    record.instance_root = instance_root.to_string_lossy().to_string();
    record.display_name = display_name;
    record.last_opened_at = now.to_string();
}

pub fn cache_status(
    registry: &InstanceRegistry,
    instance_root: Option<&Path>,
    mods_fingerprint: &str,
) -> InstanceCacheStatus {
    let Some(root) = instance_root else {
        return InstanceCacheStatus {
            instance_root: None,
            display_name: None,
            mods_fingerprint: mods_fingerprint.to_string(),
            covers_ready: false,
            dependencies_ready: false,
            ready: false,
            needs_covers: false,
            needs_dependencies: false,
        };
    };

    let key = registry_key(root);
    let record = registry.instances.get(&key);
    let fingerprint_changed = record
        .map(|item| item.mods_fingerprint != mods_fingerprint)
        .unwrap_or(true);

    let covers_ready = record.map(|item| item.covers_ready).unwrap_or(false) && !fingerprint_changed;
    let dependencies_ready = record
        .map(|item| item.dependencies_ready)
        .unwrap_or(false)
        && !fingerprint_changed;

    InstanceCacheStatus {
        instance_root: Some(root.to_string_lossy().to_string()),
        display_name: Some(display_name_for(root)),
        mods_fingerprint: mods_fingerprint.to_string(),
        covers_ready,
        dependencies_ready,
        ready: covers_ready && dependencies_ready,
        needs_covers: !covers_ready,
        needs_dependencies: !dependencies_ready,
    }
}

pub fn plan_bootstrap(
    registry: &InstanceRegistry,
    instance_root: &Path,
    mods_fingerprint: &str,
    force: bool,
) -> (bool, bool) {
    if force {
        return (true, true);
    }
    let status = cache_status(registry, Some(instance_root), mods_fingerprint);
    (status.needs_covers, status.needs_dependencies)
}

pub fn mark_prepared(
    registry: &mut InstanceRegistry,
    instance_root: &Path,
    mods_fingerprint: &str,
    covers: bool,
    dependencies: bool,
    now: &str,
) {
    let key = registry_key(instance_root);
    let record = registry.instances.entry(key).or_insert_with(|| InstanceRecord {
        instance_root: instance_root.to_string_lossy().to_string(),
        display_name: display_name_for(instance_root),
        mods_fingerprint: String::new(),
        covers_ready: false,
        dependencies_ready: false,
        last_prepared_at: String::new(),
        last_opened_at: now.to_string(),
    });
    record.mods_fingerprint = mods_fingerprint.to_string();
    if covers {
        record.covers_ready = true;
    }
    if dependencies {
        record.dependencies_ready = true;
    }
    record.last_prepared_at = now.to_string();
    record.last_opened_at = now.to_string();
}

pub fn clear_all(app: &AppHandle, extra_data_roots: Vec<PathBuf>) -> Result<ClearDataResult, String> {
    let registry = read_registry(app)?;
    let cleared_instances = registry.instances.len() as u32;
    let mut cleared_instance_dirs = 0u32;
    let mut data_roots: Vec<PathBuf> = registry
        .instances
        .values()
        .map(|record| PathBuf::from(&record.instance_root).join(".mod-manager"))
        .collect();
    data_roots.extend(extra_data_roots);
    data_roots.sort();
    data_roots.dedup();
    for data_root in data_roots {
        if clear_instance_downloads(&data_root)? {
            cleared_instance_dirs += 1;
        }
    }

    let mut removed_catalog_files = 0u32;
    let catalog_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("catalog");
    if catalog_dir.exists() {
        removed_catalog_files = count_files(&catalog_dir);
        fs::remove_dir_all(&catalog_dir).map_err(|e| e.to_string())?;
    }

    write_registry(app, &InstanceRegistry::default())?;

    Ok(ClearDataResult {
        removed_catalog_files,
        cleared_instances,
        cleared_instance_dirs,
    })
}

fn clear_instance_downloads(data_root: &Path) -> Result<bool, String> {
    if !data_root.exists() {
        return Ok(false);
    }
    let mut touched = false;
    for sub in ["cache", "covers"] {
        let path = data_root.join(sub);
        if path.exists() {
            fs::remove_dir_all(&path).map_err(|e| e.to_string())?;
            touched = true;
        }
    }
    let _ = fs::create_dir_all(data_root.join("covers"));
    let _ = fs::create_dir_all(data_root.join("cache"));
    Ok(touched)
}

fn count_files(dir: &Path) -> u32 {
    let mut total = 0u32;
    let Ok(entries) = fs::read_dir(dir) else {
        return 0;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            total = total.saturating_add(count_files(&path));
        } else {
            total += 1;
        }
    }
    total
}
