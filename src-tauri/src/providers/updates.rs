use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

use crate::catalog;
use crate::instance_meta::{detect_instance_target, InstanceTarget};
use crate::mods::{scan_mods_for_settings, ModEntry};
use crate::remote::http_client;
use crate::settings::{read_settings, Settings};
use crate::util::now_millis;

use super::updates_cache::{
    invalidate_cached_updates, read_cached_updates, write_cached_updates, UpdatesCacheHit,
};

use super::versions::{
    list_curseforge_versions_limited, list_modrinth_versions_limited, ProviderVersion,
};

const VERSION_FETCH_WORKERS: usize = 5;
const VERSION_FETCH_MIN_INTERVAL_MS: u64 = 180;
const VERSION_FETCH_MAX_ATTEMPTS: usize = 3;
const VERSION_FETCH_RETRY_BASE_MS: u64 = 450;
const VERSION_FETCH_RETRY_GAP_MS: u64 = 250;

struct RateLimiter {
    last_start: Mutex<Instant>,
    min_interval: Duration,
}

impl RateLimiter {
    fn new(min_interval: Duration) -> Self {
        Self {
            last_start: Mutex::new(Instant::now() - min_interval),
            min_interval,
        }
    }

    fn wait_turn(&self) {
        loop {
            let wait = {
                let mut guard = self.last_start.lock().unwrap();
                let elapsed = guard.elapsed();
                if elapsed >= self.min_interval {
                    *guard = Instant::now();
                    return;
                }
                self.min_interval - elapsed
            };
            std::thread::sleep(wait);
        }
    }
}

enum FetchOutcome {
    Success(ProviderVersion),
    NotAvailable,
    Failed,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ModUpdateCandidate {
    pub key: String,
    pub id: String,
    pub title: String,
    pub summary: Option<String>,
    pub source: String,
    pub project_id: String,
    pub filename: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CheckModUpdatesResponse {
    pub target: InstanceTarget,
    pub candidates: Vec<ModUpdateCandidate>,
    pub checked_projects: u32,
    pub failed_projects: u32,
    pub checked_at_ms: u64,
    pub cached: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CheckModUpdatesRequest {
    #[serde(default)]
    pub force_refresh: bool,
}

#[derive(Clone)]
struct ProjectRef {
    source: String,
    project_id: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ModUpdatesCheckStartedPayload {
    target: InstanceTarget,
    checked_projects: u32,
}

pub(crate) fn check_mod_updates_blocking(
    app: &AppHandle,
    force_refresh: bool,
) -> Result<CheckModUpdatesResponse, String> {
    let settings = read_settings(app)?;
    let paths = crate::settings::resolve_paths(&settings)?;
    let scope = paths.instance_root.to_string_lossy().to_string();
    let target = detect_instance_target(&paths);
    let catalog_root = catalog::catalog_root(app).ok();
    let mods = scan_mods_for_settings(&settings, catalog_root)?;
    let fingerprint = mods_fingerprint(&mods);

    if !force_refresh {
        if let Some(cached) = read_cached_updates(app, &scope, &fingerprint, &target)? {
            return Ok(response_from_cache_hit(cached));
        }
    } else {
        let _ = invalidate_cached_updates(app, &scope);
    }

    let has_cf_key = !settings.curseforge_api_key.trim().is_empty();

    let jobs = collect_version_jobs(&mods, has_cf_key);
    let checked_projects = jobs.len() as u32;
    let (latest_versions, failed_projects) = if jobs.is_empty() {
        (HashMap::new(), 0)
    } else {
        let _ = app.emit(
            "mod-updates-check-started",
            ModUpdatesCheckStartedPayload {
                target: target.clone(),
                checked_projects,
            },
        );
        let client = http_client().ok_or_else(|| "Не удалось создать HTTP-клиент.".to_string())?;
        fetch_latest_versions_throttled(app, &mods, has_cf_key, &client, &settings, &target, jobs)
    };

    let candidates = build_update_candidates(&mods, has_cf_key, &latest_versions);
    let checked_at_ms = now_millis();
    let response = CheckModUpdatesResponse {
        target,
        candidates,
        checked_projects,
        failed_projects,
        checked_at_ms,
        cached: false,
    };
    if response.failed_projects == 0 {
        write_cached_updates(app, &scope, &fingerprint, &cache_hit_from_response(&response))?;
    } else {
        let _ = invalidate_cached_updates(app, &scope);
    }
    Ok(response)
}

fn response_from_cache_hit(hit: UpdatesCacheHit) -> CheckModUpdatesResponse {
    CheckModUpdatesResponse {
        target: hit.target,
        candidates: hit
            .candidates
            .into_iter()
            .map(|item| ModUpdateCandidate {
                key: item.key,
                id: item.id,
                title: item.title,
                summary: item.summary,
                source: item.source,
                project_id: item.project_id,
                filename: item.filename,
            })
            .collect(),
        checked_projects: hit.checked_projects,
        failed_projects: hit.failed_projects,
        checked_at_ms: hit.checked_at_ms,
        cached: true,
    }
}

fn cache_hit_from_response(response: &CheckModUpdatesResponse) -> UpdatesCacheHit {
    UpdatesCacheHit {
        target: response.target.clone(),
        candidates: response
            .candidates
            .iter()
            .map(|item| super::updates_cache::CachedModUpdateCandidate {
                key: item.key.clone(),
                id: item.id.clone(),
                title: item.title.clone(),
                summary: item.summary.clone(),
                source: item.source.clone(),
                project_id: item.project_id.clone(),
                filename: item.filename.clone(),
            })
            .collect(),
        checked_projects: response.checked_projects,
        failed_projects: response.failed_projects,
        checked_at_ms: response.checked_at_ms,
    }
}

fn mods_fingerprint(mods: &[ModEntry]) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut parts: Vec<String> = mods
        .iter()
        .filter_map(|item| {
            provider_for(item).map(|(source, project_id)| {
                format!(
                    "{}|{}|{}|{}|{}|{}",
                    item.key,
                    item.filename,
                    source,
                    project_id,
                    item.modrinth_version_id.as_deref().unwrap_or(""),
                    item.curseforge_file_id.as_deref().unwrap_or("")
                )
            })
        })
        .collect();
    parts.sort();
    let mut hasher = DefaultHasher::new();
    parts.join("\n").hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn collect_version_jobs(mods: &[ModEntry], has_cf_key: bool) -> Vec<ProjectRef> {
    let mut jobs = Vec::new();
    let mut seen = HashSet::new();
    for item in mods {
        let Some((source, project_id)) = provider_for(item) else {
            continue;
        };
        if source == "curseforge" && !has_cf_key {
            continue;
        }
        if seen.insert((source.clone(), project_id.clone())) {
            jobs.push(ProjectRef {
                source,
                project_id,
            });
        }
    }
    jobs
}

fn candidate_for_mod(
    item: &ModEntry,
    source: String,
    project_id: String,
    latest: &ProviderVersion,
) -> Option<ModUpdateCandidate> {
    if is_installed_version(item, latest) {
        return None;
    }

    let current = item
        .installed_version
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "—".to_string());
    Some(ModUpdateCandidate {
        key: item.key.clone(),
        id: item.key.clone(),
        title: item.display_name.clone(),
        summary: Some(format!("{current} → {}", latest.version_number)),
        source,
        project_id,
        filename: item.filename.clone(),
    })
}

fn emit_candidates_for_project(
    app: &AppHandle,
    mods: &[ModEntry],
    has_cf_key: bool,
    source: &str,
    project_id: &str,
    latest: &ProviderVersion,
) {
    for item in mods {
        let Some((item_source, item_project_id)) = provider_for(item) else {
            continue;
        };
        if item_source != source || item_project_id != project_id {
            continue;
        }
        if item_source == "curseforge" && !has_cf_key {
            continue;
        }
        let Some(candidate) = candidate_for_mod(item, item_source, item_project_id, latest) else {
            continue;
        };
        let _ = app.emit("mod-updates-candidate", candidate);
    }
}

fn build_update_candidates(
    mods: &[ModEntry],
    has_cf_key: bool,
    latest_versions: &HashMap<(String, String), ProviderVersion>,
) -> Vec<ModUpdateCandidate> {
    let mut candidates = Vec::new();
    for item in mods {
        let Some((source, project_id)) = provider_for(item) else {
            continue;
        };
        if source == "curseforge" && !has_cf_key {
            continue;
        }
        let Some(latest) = latest_versions.get(&(source.clone(), project_id.clone())) else {
            continue;
        };
        let Some(candidate) = candidate_for_mod(item, source, project_id, latest) else {
            continue;
        };
        candidates.push(candidate);
    }
    candidates.sort_by(|left, right| left.title.to_lowercase().cmp(&right.title.to_lowercase()));
    candidates
}

fn fetch_latest_versions_throttled(
    app: &AppHandle,
    mods: &[ModEntry],
    has_cf_key: bool,
    client: &reqwest::blocking::Client,
    settings: &Settings,
    target: &InstanceTarget,
    projects: Vec<ProjectRef>,
) -> (HashMap<(String, String), ProviderVersion>, u32) {
    if projects.is_empty() {
        return (HashMap::new(), 0);
    }

    let rate_limiter = Arc::new(RateLimiter::new(Duration::from_millis(
        VERSION_FETCH_MIN_INTERVAL_MS,
    )));
    let queue = Arc::new(Mutex::new(projects));
    let retry_queue = Arc::new(Mutex::new(Vec::<ProjectRef>::new()));
    let results = Arc::new(Mutex::new(HashMap::new()));
    let worker_count = VERSION_FETCH_WORKERS.min({
        let guard = queue.lock().unwrap();
        guard.len().max(1)
    });

    std::thread::scope(|scope| {
        for _ in 0..worker_count {
            let queue = Arc::clone(&queue);
            let rate_limiter = Arc::clone(&rate_limiter);
            let retry_queue = Arc::clone(&retry_queue);
            let results = Arc::clone(&results);
            let app = app.clone();
            scope.spawn(move || {
                loop {
                    let job = {
                        let mut guard = queue.lock().unwrap();
                        guard.pop()
                    };
                    let Some(job) = job else {
                        break;
                    };

                    rate_limiter.wait_turn();
                    match fetch_latest_for_project_once(client, settings, target, &job) {
                        Ok(Some(version)) => {
                            emit_candidates_for_project(
                                &app,
                                mods,
                                has_cf_key,
                                &job.source,
                                &job.project_id,
                                &version,
                            );
                            results.lock().unwrap().insert(
                                (job.source.clone(), job.project_id.clone()),
                                version,
                            );
                        }
                        Ok(None) => {}
                        Err(error) if is_retryable_fetch_error(&error) => {
                            retry_queue.lock().unwrap().push(job);
                        }
                        Err(_) => {}
                    }
                }
            });
        }
    });

    let mut results = Arc::try_unwrap(results)
        .ok()
        .and_then(|mutex| mutex.into_inner().ok())
        .unwrap_or_default();
    let mut pending = Arc::try_unwrap(retry_queue)
        .ok()
        .and_then(|mutex| mutex.into_inner().ok())
        .unwrap_or_default();

    let mut failed = 0_u32;
    for (index, job) in pending.drain(..).enumerate() {
        if index > 0 {
            std::thread::sleep(Duration::from_millis(VERSION_FETCH_RETRY_GAP_MS));
        }
        let key = (job.source.clone(), job.project_id.clone());
        if results.contains_key(&key) {
            continue;
        }
        match fetch_latest_for_project_with_retry(client, settings, target, &job) {
            FetchOutcome::Success(version) => {
                emit_candidates_for_project(
                    app,
                    mods,
                    has_cf_key,
                    &job.source,
                    &job.project_id,
                    &version,
                );
                results.insert(key, version);
            }
            FetchOutcome::NotAvailable => {}
            FetchOutcome::Failed => {
                failed += 1;
            }
        }
    }

    (results, failed)
}

fn fetch_latest_for_project_with_retry(
    client: &reqwest::blocking::Client,
    settings: &Settings,
    target: &InstanceTarget,
    job: &ProjectRef,
) -> FetchOutcome {
    for attempt in 0..VERSION_FETCH_MAX_ATTEMPTS {
        if attempt > 0 {
            std::thread::sleep(Duration::from_millis(
                VERSION_FETCH_RETRY_BASE_MS * attempt as u64,
            ));
        }

        match fetch_latest_for_project_once(client, settings, target, job) {
            Ok(Some(version)) => return FetchOutcome::Success(version),
            Ok(None) => return FetchOutcome::NotAvailable,
            Err(error) if is_retryable_fetch_error(&error) && attempt + 1 < VERSION_FETCH_MAX_ATTEMPTS => {
                continue;
            }
            Err(_) => return FetchOutcome::Failed,
        }
    }

    FetchOutcome::Failed
}

fn fetch_latest_for_project_once(
    client: &reqwest::blocking::Client,
    settings: &Settings,
    target: &InstanceTarget,
    job: &ProjectRef,
) -> Result<Option<ProviderVersion>, String> {
    match job.source.as_str() {
        "modrinth" => {
            let mut versions =
                list_modrinth_versions_limited(client, &job.project_id, target, Some(1))?;
            Ok(versions.pop())
        }
        "curseforge" => {
            let mut versions = list_curseforge_versions_limited(
                client,
                settings,
                &job.project_id,
                target,
                Some(1),
            )?;
            versions.sort_by(|left, right| right.date_published.cmp(&left.date_published));
            Ok(versions.into_iter().next())
        }
        _ => Ok(None),
    }
}

fn is_retryable_fetch_error(error: &str) -> bool {
    let lower = error.to_ascii_lowercase();
    lower.contains("429")
        || lower.contains("too many")
        || lower.contains("rate limit")
        || lower.contains("timeout")
        || lower.contains("timed out")
        || lower.contains("connection")
        || lower.contains("network")
}

pub(crate) fn provider_for(item: &ModEntry) -> Option<(String, String)> {
    if item.source == "modrinth" {
        return item
            .modrinth_id
            .as_ref()
            .map(|id| id.trim())
            .filter(|id| !id.is_empty())
            .map(|id| ("modrinth".to_string(), id.to_string()));
    }
    if item.source == "curseforge" {
        return item
            .curseforge_id
            .as_ref()
            .map(|id| id.trim())
            .filter(|id| !id.is_empty())
            .map(|id| ("curseforge".to_string(), id.to_string()));
    }
    None
}

pub(crate) fn is_installed_version(item: &ModEntry, version: &ProviderVersion) -> bool {
    if item.source == "modrinth" {
        if let Some(id) = item.modrinth_version_id.as_deref() {
            if id == version.id {
                return true;
            }
        }
    }
    if item.source == "curseforge" {
        if let Some(id) = item.curseforge_file_id.as_deref() {
            let file_id = version.file_id.as_deref().unwrap_or(version.id.as_str());
            if id == file_id {
                return true;
            }
        }
    }
    !item.filename.is_empty() && item.filename == version.filename
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_mod() -> ModEntry {
        ModEntry {
            key: "abc".to_string(),
            filename: "mod-1.0.0.jar".to_string(),
            base: "mod-1.0.0.jar".to_string(),
            display_name: "Mod".to_string(),
            display_name_locked: false,
            installed_version: Some("1.0.0".to_string()),
            side: crate::mods::UNKNOWN_SIDE.to_string(),
            library: false,
            technical: false,
            description: String::new(),
            dependencies: Vec::new(),
            resolved_dependencies: Vec::new(),
            jar_dependencies: Vec::new(),
            used_by: Vec::new(),
            cover_url: None,
            cover_path: None,
            cover_manual: false,
            cover_modified_at: None,
            source: "modrinth".to_string(),
            source_url: None,
            has_index: false,
            has_tags: false,
            index_file: None,
            pack_side: None,
            modrinth_id: Some("project".to_string()),
            modrinth_version_id: Some("version-old".to_string()),
            curseforge_id: None,
            curseforge_file_id: None,
            duplicate: false,
            modified_at: String::new(),
            side_mode: "auto".to_string(),
            manual_side: String::new(),
            manual_library: false,
            manual_technical: false,
            provider_side: String::new(),
            provider_library: false,
            provider_technical: false,
            disabled: false,
        }
    }

    #[test]
    fn is_installed_version_matches_modrinth_id() {
        let item = sample_mod();
        let version = ProviderVersion {
            id: "version-old".to_string(),
            file_id: None,
            version_number: "1.0.0".to_string(),
            name: "1.0.0".to_string(),
            filename: "mod-2.0.0.jar".to_string(),
            download_url: None,
            game_versions: Vec::new(),
            loaders: Vec::new(),
            date_published: None,
            downloads: None,
            size: None,
            release_type: None,
        };
        assert!(is_installed_version(&item, &version));
    }

    #[test]
    fn retryable_fetch_error_detects_rate_limit() {
        assert!(is_retryable_fetch_error("HTTP 429 Too Many Requests"));
        assert!(!is_retryable_fetch_error("Modrinth вернул неожиданный ответ."));
    }
}
