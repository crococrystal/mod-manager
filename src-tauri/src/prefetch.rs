use std::{collections::HashMap, path::PathBuf};

use tauri::AppHandle;

use crate::catalog;
use crate::covers::{apply_existing_cover, cache_remote_cover};
use crate::dependencies::same_dependency_list;
use crate::events::{
    emit_cover_ready, emit_dependencies_ready, emit_prefetch_done, emit_prefetch_progress,
    PrefetchReport,
};
use crate::mods::{merge_keys, scan_mods_for_settings};
use crate::remote::{fetch_api_dependencies, http_client, resolve_cover_url};
use crate::settings::{resolve_paths, Settings};
use crate::tags::{read_tags, write_tags};
use crate::util::{file_mtime_millis, now_iso, path_string};

pub(crate) fn prefetch_mod_assets_for_settings(
    settings: &Settings,
    app: &AppHandle,
    run_covers: bool,
    run_dependencies: bool,
) -> Result<PrefetchReport, String> {
    let paths = resolve_paths(settings)?;
    let catalog_root: Option<PathBuf> = catalog::catalog_root(app).ok();
    let mut mods = scan_mods_for_settings(settings, catalog_root.clone())?;
    let Some(client) = http_client() else {
        return Err("Не удалось создать HTTP-клиент.".to_string());
    };

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

    let mut tags = read_tags(&paths.tags_path)?;
    let mut report = PrefetchReport::new();
    let has_cf_key = !settings.curseforge_api_key.trim().is_empty();
    let total = mods.len() as u32;

    for (index, item) in mods.iter_mut().enumerate() {
        let step = index as u32 + 1;
        emit_prefetch_progress(app, "mods", step, total, &item.display_name, "fetch", "");

        if run_covers {
            apply_existing_cover(item, &paths, catalog_root.as_deref());
            if item.cover_path.is_some() {
                report.skipped += 1;
            } else if item.modrinth_id.is_some() || item.curseforge_id.is_some() {
                if let Some(url) = resolve_cover_url(item, &client, &settings.curseforge_api_key) {
                    if let Some(path) = cache_remote_cover(
                        &client,
                        &paths,
                        catalog_root.as_deref(),
                        &item.key,
                        item.modrinth_id.as_deref(),
                        item.curseforge_id.as_deref(),
                        &url,
                    ) {
                        let mtime = file_mtime_millis(&path);
                        let stored = path_string(path);
                        emit_cover_ready(app, &item.key, &stored, mtime);
                        item.cover_modified_at = mtime;
                        item.cover_path = Some(stored);
                        item.cover_manual = false;
                        report.downloaded += 1;
                    } else {
                        report.failed += 1;
                    }
                } else {
                    report.failed += 1;
                }
            } else {
                report.skipped += 1;
            }
        }

        if run_dependencies {
            let can_cf = item.curseforge_id.is_some() && item.curseforge_file_id.is_some();
            let can_mr = item.modrinth_version_id.is_some();

            if !can_cf && !can_mr {
                report.skipped += 1;
            } else if can_cf && !has_cf_key && !can_mr {
                report.skipped += 1;
            } else {
                let keys = fetch_api_dependencies(
                    item,
                    &client,
                    settings,
                    &modrinth_lookup,
                    &curseforge_lookup,
                );
                if keys.is_empty() {
                    report.unchanged += 1;
                } else {
                    let current = tags.mods.entry(item.key.clone()).or_default();
                    let previous = current.dependencies.clone();
                    let merged = merge_keys(&[&previous, &keys]);
                    if same_dependency_list(&previous, &merged) {
                        report.unchanged += 1;
                    } else {
                        let added = keys.iter().filter(|key| !previous.contains(key)).count();
                        current.dependencies = merged.clone();
                        current.updated_at = now_iso();
                        tags.updated_at = now_iso();
                        write_tags(&paths.tags_path, &tags)?;
                        emit_dependencies_ready(app, &item.key, &merged);
                        report.updated += 1;
                        report.added_links += added as u32;
                    }
                }
            }
        }

        emit_prefetch_progress(app, "mods", step, total, &item.display_name, "ok", "");
    }

    emit_prefetch_done(app, "mods");
    Ok(report)
}
