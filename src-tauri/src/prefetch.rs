use std::{collections::HashMap, path::PathBuf};

use tauri::AppHandle;

use crate::bootstrap::ensure_task_active;

use crate::catalog;
use crate::dependencies::{
    filter_reverse_jar_dependency_keys, jar_dependencies_by_key, same_dependency_list,
};
use crate::covers::cache_remote_cover;
use crate::events::{
    emit_cover_ready, emit_dependencies_ready, emit_labels_ready, emit_prefetch_done,
    emit_prefetch_progress,
};
use crate::file_identity::read_file_identity;
use crate::mods::{merge_keys, scan_mods_for_settings};
use crate::provider_labels::{build_curseforge_labels, build_modrinth_labels, refresh_result_for};
use crate::remote::{
    curseforge_cover_url_from_payload, curseforge_dependencies_from_payload,
    curseforge_files_batch, curseforge_fingerprint_matches, curseforge_mod_info,
    curseforge_mods_batch, http_client, modrinth_cover_url_from_payload,
    modrinth_dependencies_from_payload, modrinth_projects_batch, modrinth_search_icon,
    modrinth_versions_batch, modrinth_versions_by_sha512,
};
use crate::settings::{resolve_paths, Settings};
use crate::tags::{read_tags, write_tags, ProviderLabelsStore};
use crate::util::{file_mtime_millis, now_iso, path_string};

struct PendingIdentity {
    key: String,
    filename: String,
    sha512: String,
    curseforge_fingerprint: u32,
}

pub(crate) fn identify_unknown_sources(
    settings: &Settings,
    client: &reqwest::blocking::Client,
    paths: &crate::settings::InstancePaths,
    tags: &mut crate::tags::TagFile,
    mods: &[crate::mods::ModEntry],
) -> Result<bool, String> {
    let mut pending = Vec::new();
    for item in mods {
        if matches!(item.source.as_str(), "modrinth" | "curseforge") {
            continue;
        }
        let identity = read_file_identity(&paths.mods_dir.join(&item.filename))?;
        pending.push(PendingIdentity {
            key: item.key.clone(),
            filename: item.filename.clone(),
            sha512: identity.sha512,
            curseforge_fingerprint: identity.curseforge_fingerprint,
        });
    }

    if pending.is_empty() {
        return Ok(false);
    }

    let mut changed = false;
    let sha512_hashes: Vec<String> = pending.iter().map(|item| item.sha512.clone()).collect();
    let modrinth_matches = modrinth_versions_by_sha512(client, &sha512_hashes);
    let mut unmatched_curseforge = Vec::new();

    for item in &pending {
        if let Some(found) = modrinth_matches.get(&item.sha512) {
            let tag = tags.mods.entry(item.key.clone()).or_default();
            tag.source = "modrinth".to_string();
            tag.modrinth_id = found.project_id.clone();
            tag.modrinth_version_id = found.version_id.clone();
            if !tag.aliases.contains(&item.filename) {
                tag.aliases.push(item.filename.clone());
            }
            tag.updated_at = now_iso();
            tags.updated_at = now_iso();
            changed = true;
        } else {
            unmatched_curseforge.push(item);
        }
    }

    if !settings.curseforge_api_key.trim().is_empty() && !unmatched_curseforge.is_empty() {
        let fingerprints: Vec<u32> = unmatched_curseforge
            .iter()
            .map(|item| item.curseforge_fingerprint)
            .collect();
        let curseforge_matches =
            curseforge_fingerprint_matches(client, &settings.curseforge_api_key, &fingerprints);

        for item in unmatched_curseforge {
            let Some(found) = curseforge_matches.get(&item.curseforge_fingerprint) else {
                continue;
            };
            let slug = curseforge_mod_info(client, &settings.curseforge_api_key, &found.project_id)
                .and_then(|project| project.slug)
                .unwrap_or_default();
            let tag = tags.mods.entry(item.key.clone()).or_default();
            tag.source = "curseforge".to_string();
            tag.curseforge_id = found.project_id.clone();
            tag.curseforge_file_id = found.file_id.clone();
            tag.curseforge_slug = slug;
            if !tag.aliases.contains(&item.filename) {
                tag.aliases.push(item.filename.clone());
            }
            tag.updated_at = now_iso();
            tags.updated_at = now_iso();
            changed = true;
        }
    }

    Ok(changed)
}

#[derive(Clone, Copy, Default)]
pub(crate) struct SyncFlags {
    pub labels: bool,
    pub covers: bool,
    pub dependencies: bool,
    pub force_covers: bool,
    pub force_labels: bool,
}

#[derive(Default, Clone, Copy)]
pub(crate) struct UnifiedSyncReport {
    pub covers_downloaded: u32,
    pub labels_refreshed: u32,
    pub dependencies_updated: u32,
}

/// Единый цикл по модам: тянет project/version (или mod/file) **по одному разу** и собирает
/// из payload-ов сразу labels, обложки и зависимости. После каждого мода эмитит события и
/// инкрементально пишет `tags.json`, чтобы UI обновлялся сразу.
pub(crate) fn sync_mods_unified(
    settings: &Settings,
    app: &AppHandle,
    flags: SyncFlags,
    task_token: Option<u64>,
) -> Result<UnifiedSyncReport, String> {
    if !flags.labels && !flags.covers && !flags.dependencies {
        return Ok(UnifiedSyncReport::default());
    }

    let paths = resolve_paths(settings)?;
    let catalog_root: Option<PathBuf> = catalog::catalog_root(app).ok();
    let Some(client) = http_client() else {
        return Err("Не удалось создать HTTP-клиент.".to_string());
    };

    let mut tags = read_tags(&paths.tags_path)?;
    let mut mods = scan_mods_for_settings(settings, catalog_root.clone())?;

    if flags.labels && flags.force_labels {
        for tag in tags.mods.values_mut() {
            tag.provider_labels = ProviderLabelsStore::default();
        }
    }

    if identify_unknown_sources(settings, &client, &paths, &mut tags, &mods)? {
        write_tags(&paths.tags_path, &tags)?;
        mods = scan_mods_for_settings(settings, catalog_root.clone())?;
    }

    let modrinth_lookup: HashMap<String, String> = mods
        .iter()
        .filter_map(|item| {
            item.modrinth_id
                .as_ref()
                .map(|id| (id.clone(), item.key.clone()))
        })
        .collect();
    let curseforge_lookup: HashMap<String, String> = mods
        .iter()
        .filter_map(|item| {
            item.curseforge_id
                .as_ref()
                .map(|id| (id.clone(), item.key.clone()))
        })
        .collect();

    let jar_dependencies = jar_dependencies_by_key(&mods);
    let has_cf_key = !settings.curseforge_api_key.trim().is_empty();
    let total = mods.len() as u32;
    let mut report = UnifiedSyncReport::default();

    let need_any = flags.labels || flags.covers || flags.dependencies;
    let need_versions = flags.labels || flags.dependencies;

    // Префетч всех payload-ов одним пакетом запросов.
    let mut modrinth_projects: HashMap<String, serde_json::Value> = HashMap::new();
    let mut modrinth_versions: HashMap<String, serde_json::Value> = HashMap::new();
    let mut curseforge_mods: HashMap<String, serde_json::Value> = HashMap::new();
    let mut curseforge_files: HashMap<String, serde_json::Value> = HashMap::new();

    if need_any {
        if let Some(token) = task_token {
            ensure_task_active(app, token)?;
        }

        let modrinth_project_ids: Vec<String> = mods
            .iter()
            .filter_map(|item| item.modrinth_id.clone())
            .collect();
        let modrinth_version_ids: Vec<String> = if need_versions {
            mods.iter()
                .filter_map(|item| item.modrinth_version_id.clone())
                .collect()
        } else {
            Vec::new()
        };
        let curseforge_mod_ids: Vec<String> = if has_cf_key {
            mods.iter()
                .filter_map(|item| item.curseforge_id.clone())
                .collect()
        } else {
            Vec::new()
        };
        let curseforge_file_ids: Vec<String> = if has_cf_key && need_versions {
            mods.iter()
                .filter_map(|item| item.curseforge_file_id.clone())
                .collect()
        } else {
            Vec::new()
        };

        emit_prefetch_progress(app, "mods", 0, total, "Загрузка данных…", "fetch", "");

        let client_ref = &client;
        let cf_key = settings.curseforge_api_key.clone();
        let run_versions = need_versions;
        let run_cf = has_cf_key;
        let run_cf_versions = has_cf_key && need_versions;

        let (mr_p, mr_v, cf_m, cf_f) = std::thread::scope(|scope| {
            let mr_p_handle = scope.spawn(move || {
                modrinth_projects_batch(client_ref, &modrinth_project_ids)
            });
            let mr_v_handle = if run_versions {
                Some(scope.spawn(move || {
                    modrinth_versions_batch(client_ref, &modrinth_version_ids)
                }))
            } else {
                None
            };
            let cf_key_for_mods = cf_key.clone();
            let cf_m_handle = if run_cf {
                Some(scope.spawn(move || {
                    curseforge_mods_batch(client_ref, &cf_key_for_mods, &curseforge_mod_ids)
                }))
            } else {
                None
            };
            let cf_key_for_files = cf_key.clone();
            let cf_f_handle = if run_cf_versions {
                Some(scope.spawn(move || {
                    curseforge_files_batch(client_ref, &cf_key_for_files, &curseforge_file_ids)
                }))
            } else {
                None
            };

            (
                mr_p_handle.join().unwrap_or_default(),
                mr_v_handle
                    .map(|handle| handle.join().unwrap_or_default())
                    .unwrap_or_default(),
                cf_m_handle
                    .map(|handle| handle.join().unwrap_or_default())
                    .unwrap_or_default(),
                cf_f_handle
                    .map(|handle| handle.join().unwrap_or_default())
                    .unwrap_or_default(),
            )
        });

        modrinth_projects = mr_p;
        modrinth_versions = mr_v;
        curseforge_mods = cf_m;
        curseforge_files = cf_f;

        if let Some(token) = task_token {
            ensure_task_active(app, token)?;
        }
    }

    struct CoverDownload {
        key: String,
        url: String,
        modrinth_id: Option<String>,
        curseforge_id: Option<String>,
        display_name: String,
    }

    let mut cover_queue: Vec<CoverDownload> = Vec::new();

    // Фаза 1: локальная обработка метаданных (метки + зависимости) — быстро, без сети.
    for (index, item) in mods.iter_mut().enumerate() {
        if let Some(token) = task_token {
            ensure_task_active(app, token)?;
        }

        let step = index as u32 + 1;
        let has_modrinth = item.modrinth_id.is_some();
        let has_curseforge = item.curseforge_id.is_some() && has_cf_key;
        let prefer_curseforge = item.source == "curseforge";

        if !has_modrinth && !has_curseforge {
            continue;
        }

        let mr_project = item
            .modrinth_id
            .as_deref()
            .and_then(|id| modrinth_projects.get(id))
            .cloned();
        let mr_version = item
            .modrinth_version_id
            .as_deref()
            .filter(|value| !value.is_empty())
            .and_then(|id| modrinth_versions.get(id))
            .cloned();
        let cf_project = if has_curseforge {
            item.curseforge_id
                .as_deref()
                .and_then(|id| curseforge_mods.get(id))
                .cloned()
        } else {
            None
        };
        let cf_file = if has_curseforge {
            item.curseforge_file_id
                .as_deref()
                .filter(|value| !value.is_empty())
                .and_then(|id| curseforge_files.get(id))
                .cloned()
        } else {
            None
        };

        let mut tag_dirty = false;

        if flags.labels {
            let active_source = if prefer_curseforge && has_curseforge {
                "curseforge"
            } else if has_modrinth {
                "modrinth"
            } else if has_curseforge {
                "curseforge"
            } else {
                ""
            };

            let next_store = match active_source {
                "modrinth" => mr_project
                    .as_ref()
                    .map(|project| build_modrinth_labels(project, mr_version.as_ref())),
                "curseforge" => cf_project
                    .as_ref()
                    .and_then(|project| build_curseforge_labels(project, cf_file.as_ref())),
                _ => None,
            };

            if let Some(store) = next_store {
                let tag = tags.mods.entry(item.key.clone()).or_default();
                tag.provider_labels = store;
                tag.updated_at = now_iso();
                tag_dirty = true;
                report.labels_refreshed += 1;

                let result = refresh_result_for(tag, &item.key);
                emit_labels_ready(
                    app,
                    &result.key,
                    &result.side,
                    result.library,
                    result.technical,
                    &result.side_mode,
                    &result.manual_side,
                    result.manual_library,
                    result.manual_technical,
                    &result.provider_side,
                    result.provider_library,
                    result.provider_technical,
                );
            }
        }

        if flags.dependencies && settings.auto_prefetch_dependencies {
            let mut keys: Vec<String> = Vec::new();
            if let Some(version) = mr_version.as_ref() {
                for key in modrinth_dependencies_from_payload(version, &modrinth_lookup) {
                    keys.push(key);
                }
            }
            if let Some(file) = cf_file.as_ref() {
                for key in curseforge_dependencies_from_payload(file, &curseforge_lookup) {
                    keys.push(key);
                }
            }
            if !keys.is_empty() {
                let keys = filter_reverse_jar_dependency_keys(
                    &item.key,
                    &item.jar_dependencies,
                    &keys,
                    &jar_dependencies,
                );
                if !keys.is_empty() {
                    let current = tags.mods.entry(item.key.clone()).or_default();
                    let previous = current.dependencies.clone();
                    let merged = merge_keys(&[&previous, &keys]);
                    if !same_dependency_list(&previous, &merged) {
                        current.dependencies = merged.clone();
                        current.updated_at = now_iso();
                        tag_dirty = true;
                        report.dependencies_updated += 1;
                        emit_dependencies_ready(app, &item.key, &merged);
                    }
                }
            }
        }

        if tag_dirty {
            tags.updated_at = now_iso();
        }

        if flags.covers {
            let already_has_cover = !flags.force_covers && item.cover_path.is_some();
            if !already_has_cover && !item.cover_manual {
                if let Some(url) = pick_cover_url(
                    &client,
                    settings,
                    item,
                    mr_project.as_ref(),
                    cf_project.as_ref(),
                ) {
                    cover_queue.push(CoverDownload {
                        key: item.key.clone(),
                        url,
                        modrinth_id: item.modrinth_id.clone(),
                        curseforge_id: item.curseforge_id.clone(),
                        display_name: item.display_name.clone(),
                    });
                }
            }
        }

        emit_prefetch_progress(app, "mods", step, total, &item.display_name, "ok", "");
    }

    // Записываем накопленные изменения tags.json одним write (вместо per-mod).
    if report.labels_refreshed > 0 || report.dependencies_updated > 0 {
        write_tags(&paths.tags_path, &tags)?;
    }

    // Фаза 2: параллельное скачивание обложек с CDN.
    if !cover_queue.is_empty() {
        const COVER_WORKERS: usize = 8;
        let total_covers = cover_queue.len() as u32;
        let queue = std::sync::Mutex::new(cover_queue);
        let progress_counter = std::sync::atomic::AtomicU32::new(0);
        let downloaded_counter = std::sync::atomic::AtomicU32::new(0);

        let client_ref = &client;
        let paths_ref = &paths;
        let catalog_root_ref = catalog_root.as_deref();
        let force_covers = flags.force_covers;
        let app_ref = app;
        let token_ref = task_token;
        let queue_ref = &queue;
        let progress_ref = &progress_counter;
        let downloaded_ref = &downloaded_counter;

        std::thread::scope(|scope| {
            for _ in 0..COVER_WORKERS.min(total_covers as usize) {
                scope.spawn(move || loop {
                    if let Some(token) = token_ref {
                        if !crate::bootstrap::bootstrap_still_active(app_ref, token) {
                            return;
                        }
                    }
                    let task = {
                        let mut q = queue_ref.lock().unwrap();
                        q.pop()
                    };
                    let Some(task) = task else {
                        return;
                    };
                    if let Some(path) = cache_remote_cover(
                        client_ref,
                        paths_ref,
                        catalog_root_ref,
                        &task.key,
                        task.modrinth_id.as_deref(),
                        task.curseforge_id.as_deref(),
                        &task.url,
                        force_covers,
                    ) {
                        let mtime = file_mtime_millis(&path);
                        let stored = path_string(path);
                        emit_cover_ready(app_ref, &task.key, &stored, mtime);
                        downloaded_ref.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    }
                    let done = progress_ref.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                    emit_prefetch_progress(
                        app_ref,
                        "mods",
                        done,
                        total_covers,
                        &task.display_name,
                        "cover",
                        "",
                    );
                });
            }
        });

        report.covers_downloaded = downloaded_counter.load(std::sync::atomic::Ordering::Relaxed);
        if let Some(token) = task_token {
            ensure_task_active(app, token)?;
        }
    }

    emit_prefetch_done(app, "mods");
    Ok(report)
}

fn pick_cover_url(
    client: &reqwest::blocking::Client,
    settings: &Settings,
    item: &crate::mods::ModEntry,
    mr_project: Option<&serde_json::Value>,
    cf_project: Option<&serde_json::Value>,
) -> Option<String> {
    let prefer_curseforge = item.source == "curseforge";

    let modrinth_url = mr_project.and_then(modrinth_cover_url_from_payload);
    let curseforge_url = cf_project.and_then(curseforge_cover_url_from_payload);

    if prefer_curseforge {
        if let Some(url) = curseforge_url.clone() {
            return Some(url);
        }
        if let Some(url) = modrinth_url.clone() {
            return Some(url);
        }
    } else {
        if let Some(url) = modrinth_url.clone() {
            return Some(url);
        }
        if let Some(url) = curseforge_url.clone() {
            return Some(url);
        }
    }

    if item.curseforge_id.is_some() && modrinth_url.is_none() {
        if let Some(url) = modrinth_search_icon(client, &item.display_name) {
            return Some(url);
        }
    }
    let _ = settings;
    None
}
