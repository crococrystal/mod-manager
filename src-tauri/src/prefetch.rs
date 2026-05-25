use std::{collections::HashMap, path::PathBuf};

use tauri::AppHandle;

use crate::catalog;
use crate::covers::{apply_existing_cover, fetch_mod_cover};
use crate::dependencies::same_dependency_list;
use crate::events::{
    emit_cover_ready, emit_dependencies_ready, emit_prefetch_done, emit_prefetch_progress,
    PrefetchReport,
};
use crate::file_identity::read_file_identity;
use crate::mods::{merge_keys, scan_mods_for_settings};
use crate::remote::{
    curseforge_fingerprint_matches, curseforge_mod_info, fetch_api_dependencies, http_client,
    modrinth_versions_by_sha512,
};
use crate::settings::{resolve_paths, Settings};
use crate::tags::{read_tags, write_tags};
use crate::util::{file_mtime_millis, now_iso, path_string};

struct PendingIdentity {
    key: String,
    filename: String,
    sha512: String,
    curseforge_fingerprint: u32,
}

fn identify_unknown_sources(
    settings: &Settings,
    client: &reqwest::blocking::Client,
    paths: &crate::settings::InstancePaths,
    tags: &mut crate::tags::TagFile,
    mods: &[crate::mods::ModEntry],
) -> Result<bool, String> {
    let mut pending = Vec::new();
    for item in mods {
        if item.modrinth_id.is_some() || item.curseforge_id.is_some() {
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

    let mut tags = read_tags(&paths.tags_path)?;
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
                if let Some(path) = fetch_mod_cover(
                    &client,
                    &paths,
                    catalog_root.as_deref(),
                    item,
                    settings,
                    false,
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
