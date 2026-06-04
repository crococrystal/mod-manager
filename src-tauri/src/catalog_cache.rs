use std::sync::{Mutex, OnceLock};
use std::{collections::HashMap, fs, path::PathBuf};

use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::catalog;
use crate::providers::{CatalogInstallPreview, CatalogProjectDetails};

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct CatalogCacheFile {
    #[serde(default = "catalog_cache_version")]
    version: u8,
    #[serde(default)]
    entries: HashMap<String, CatalogCacheEntry>,
}

fn catalog_cache_version() -> u8 {
    2
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct CatalogCacheEntry {
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<CatalogProjectDetails>,
    #[serde(skip_serializing_if = "Option::is_none")]
    preview: Option<CatalogInstallPreview>,
}

fn cache_key(scope: &str, source: &str, project_id: &str) -> String {
    format!(
        "{}\u{0000}{}\u{0000}{}",
        scope.trim(),
        source.trim().to_ascii_lowercase(),
        project_id.trim()
    )
}

fn cache_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(catalog::catalog_root(app)?.join("catalog-cache.json"))
}

fn cache_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn load_cache(path: &PathBuf) -> CatalogCacheFile {
    let Ok(text) = fs::read_to_string(path) else {
        return CatalogCacheFile {
            version: catalog_cache_version(),
            entries: HashMap::new(),
        };
    };
    let cache = serde_json::from_str::<CatalogCacheFile>(&text).unwrap_or_default();
    if cache.version != catalog_cache_version() {
        return CatalogCacheFile {
            version: catalog_cache_version(),
            entries: HashMap::new(),
        };
    }
    cache
}

fn save_cache(path: &PathBuf, cache: &CatalogCacheFile) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let text = serde_json::to_string_pretty(cache).map_err(|error| error.to_string())?;
    fs::write(path, format!("{text}\n")).map_err(|error| error.to_string())
}

pub(crate) fn read_project_details(
    app: &AppHandle,
    scope: &str,
    source: &str,
    project_id: &str,
) -> Result<Option<CatalogProjectDetails>, String> {
    let path = cache_path(app)?;
    let _guard = cache_lock()
        .lock()
        .map_err(|_| "Не удалось заблокировать кэш каталога.".to_string())?;
    let cache = load_cache(&path);
    Ok(cache
        .entries
        .get(&cache_key(scope, source, project_id))
        .and_then(|entry| entry.details.clone()))
}

pub(crate) fn write_project_details(
    app: &AppHandle,
    scope: &str,
    source: &str,
    project_id: &str,
    details: CatalogProjectDetails,
) -> Result<(), String> {
    let path = cache_path(app)?;
    let _guard = cache_lock()
        .lock()
        .map_err(|_| "Не удалось заблокировать кэш каталога.".to_string())?;
    let mut cache = load_cache(&path);
    let entry = cache
        .entries
        .entry(cache_key(scope, source, project_id))
        .or_default();
    entry.details = Some(details);
    save_cache(&path, &cache)
}

pub(crate) fn read_install_preview(
    app: &AppHandle,
    scope: &str,
    source: &str,
    project_id: &str,
) -> Result<Option<CatalogInstallPreview>, String> {
    let path = cache_path(app)?;
    let _guard = cache_lock()
        .lock()
        .map_err(|_| "Не удалось заблокировать кэш каталога.".to_string())?;
    let cache = load_cache(&path);
    Ok(cache
        .entries
        .get(&cache_key(scope, source, project_id))
        .and_then(|entry| entry.preview.clone()))
}

pub(crate) fn write_install_preview(
    app: &AppHandle,
    scope: &str,
    source: &str,
    project_id: &str,
    preview: CatalogInstallPreview,
) -> Result<(), String> {
    let path = cache_path(app)?;
    let _guard = cache_lock()
        .lock()
        .map_err(|_| "Не удалось заблокировать кэш каталога.".to_string())?;
    let mut cache = load_cache(&path);
    let entry = cache
        .entries
        .entry(cache_key(scope, source, project_id))
        .or_default();
    entry.preview = Some(preview);
    save_cache(&path, &cache)
}
