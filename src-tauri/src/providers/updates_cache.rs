use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::catalog;
use crate::instance_meta::InstanceTarget;
use crate::util::now_millis;

const UPDATES_CACHE_TTL_MS: u64 = 24 * 60 * 60 * 1000;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CachedModUpdateCandidate {
    pub key: String,
    pub id: String,
    pub title: String,
    pub summary: Option<String>,
    pub source: String,
    pub project_id: String,
    pub filename: String,
}

#[derive(Clone, Debug)]
pub(crate) struct UpdatesCacheHit {
    pub target: InstanceTarget,
    pub candidates: Vec<CachedModUpdateCandidate>,
    pub checked_projects: u32,
    pub failed_projects: u32,
    pub checked_at_ms: u64,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct UpdatesCacheFile {
    #[serde(default = "updates_cache_version")]
    version: u8,
    #[serde(default)]
    entries: HashMap<String, UpdatesCacheEntry>,
}

fn updates_cache_version() -> u8 {
    1
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct UpdatesCacheEntry {
    checked_at_ms: u64,
    mods_fingerprint: String,
    target: InstanceTarget,
    candidates: Vec<CachedModUpdateCandidate>,
    checked_projects: u32,
    failed_projects: u32,
}

fn cache_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(catalog::catalog_root(app)?.join("updates-cache.json"))
}

fn cache_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn load_cache(path: &PathBuf) -> UpdatesCacheFile {
    let Ok(text) = fs::read_to_string(path) else {
        return UpdatesCacheFile {
            version: updates_cache_version(),
            entries: HashMap::new(),
        };
    };
    let cache = serde_json::from_str::<UpdatesCacheFile>(&text).unwrap_or_default();
    if cache.version != updates_cache_version() {
        return UpdatesCacheFile {
            version: updates_cache_version(),
            entries: HashMap::new(),
        };
    }
    cache
}

fn save_cache(path: &PathBuf, cache: &UpdatesCacheFile) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let text = serde_json::to_string_pretty(cache).map_err(|error| error.to_string())?;
    fs::write(path, format!("{text}\n")).map_err(|error| error.to_string())
}

fn cache_fresh(checked_at_ms: u64) -> bool {
    now_millis().saturating_sub(checked_at_ms) <= UPDATES_CACHE_TTL_MS
}

pub(crate) fn read_cached_updates(
    app: &AppHandle,
    scope: &str,
    mods_fingerprint: &str,
    target: &InstanceTarget,
) -> Result<Option<UpdatesCacheHit>, String> {
    let path = cache_path(app)?;
    let _guard = cache_lock()
        .lock()
        .map_err(|_| "Не удалось заблокировать кэш обновлений.".to_string())?;
    let cache = load_cache(&path);
    let Some(entry) = cache.entries.get(scope.trim()) else {
        return Ok(None);
    };
    if entry.mods_fingerprint != mods_fingerprint
        || entry.target != *target
        || !cache_fresh(entry.checked_at_ms)
    {
        return Ok(None);
    }
    Ok(Some(UpdatesCacheHit {
        target: entry.target.clone(),
        candidates: entry.candidates.clone(),
        checked_projects: entry.checked_projects,
        failed_projects: entry.failed_projects,
        checked_at_ms: entry.checked_at_ms,
    }))
}

pub(crate) fn write_cached_updates(
    app: &AppHandle,
    scope: &str,
    mods_fingerprint: &str,
    hit: &UpdatesCacheHit,
) -> Result<(), String> {
    let path = cache_path(app)?;
    let _guard = cache_lock()
        .lock()
        .map_err(|_| "Не удалось заблокировать кэш обновлений.".to_string())?;
    let mut cache = load_cache(&path);
    cache.entries.insert(
        scope.trim().to_string(),
        UpdatesCacheEntry {
            checked_at_ms: hit.checked_at_ms,
            mods_fingerprint: mods_fingerprint.to_string(),
            target: hit.target.clone(),
            candidates: hit.candidates.clone(),
            checked_projects: hit.checked_projects,
            failed_projects: hit.failed_projects,
        },
    );
    save_cache(&path, &cache)
}

pub(crate) fn invalidate_cached_updates(app: &AppHandle, scope: &str) -> Result<(), String> {
    let path = cache_path(app)?;
    let _guard = cache_lock()
        .lock()
        .map_err(|_| "Не удалось заблокировать кэш обновлений.".to_string())?;
    let mut cache = load_cache(&path);
    cache.entries.remove(scope.trim());
    save_cache(&path, &cache)
}
