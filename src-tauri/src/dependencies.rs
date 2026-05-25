use std::{collections::HashMap, path::PathBuf, thread::sleep, time::Duration};
use tauri::AppHandle;

use crate::catalog;
use crate::events::{emit_prefetch_done, emit_prefetch_progress, PrefetchReport};
use crate::jar_deps;
use crate::mods::{merge_keys, scan_mods_for_settings, ModEntry};
use crate::remote::{fetch_api_dependencies, http_client};
use crate::settings::{resolve_paths, InstancePaths, Settings};
use crate::tags::read_tags;
use crate::tags::write_tags;
use crate::util::now_iso;

pub(crate) fn apply_jar_dependencies(
    mods: &mut [ModEntry],
    paths: &InstancePaths,
) -> Result<(), String> {
    let refs: Vec<jar_deps::ModRef> = mods
        .iter()
        .map(|item| jar_deps::ModRef {
            key: item.key.clone(),
            filename: item.filename.clone(),
            display_name: item.display_name.clone(),
            base: item.base.clone(),
            modrinth_id: item.modrinth_id.clone(),
        })
        .collect();
    let cache_path = paths.data_root.join("cache").join("jar-dependencies.json");
    let map = jar_deps::jar_deps_for_mods(&paths.mods_dir, &cache_path, &refs)?;
    for item in mods.iter_mut() {
        item.jar_dependencies = map.get(&item.key).cloned().unwrap_or_default();
        item.resolved_dependencies = merge_keys(&[&item.dependencies, &item.jar_dependencies]);
    }
    Ok(())
}

fn same_dependency_list(left: &[String], right: &[String]) -> bool {
    left == right
}

pub(crate) fn prefetch_dependencies_for_settings(
    settings: &Settings,
    app: &AppHandle,
) -> Result<PrefetchReport, String> {
    let paths = resolve_paths(settings)?;
    let catalog_root: Option<PathBuf> = catalog::catalog_root(app).ok();
    let mods = scan_mods_for_settings(settings, catalog_root)?;
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

    for (index, item) in mods.iter().enumerate() {
        let step = index as u32 + 1;
        emit_prefetch_progress(app, "dependencies", step, total, &item.display_name, "fetch", "");

        let can_cf = item.curseforge_id.is_some() && item.curseforge_file_id.is_some();
        let can_mr = item.modrinth_version_id.is_some();

        if !can_cf && !can_mr {
            report.skipped += 1;
            emit_prefetch_progress(app, "dependencies", step, total, &item.display_name, "skip", "нет id в index");
            continue;
        }
        if can_cf && !has_cf_key && !can_mr {
            report.skipped += 1;
            emit_prefetch_progress(app, "dependencies", step, total, &item.display_name, "skip", "нужен API key");
            continue;
        }

        sleep(Duration::from_millis(180));
        let keys =
            fetch_api_dependencies(item, &client, settings, &modrinth_lookup, &curseforge_lookup);
        if keys.is_empty() {
            report.unchanged += 1;
            emit_prefetch_progress(app, "dependencies", step, total, &item.display_name, "skip", "нет зависимостей");
            continue;
        }

        let current = tags.mods.entry(item.key.clone()).or_default();
        let previous = current.dependencies.clone();
        let merged = merge_keys(&[&previous, &keys]);
        if same_dependency_list(&previous, &merged) {
            report.unchanged += 1;
            emit_prefetch_progress(app, "dependencies", step, total, &item.display_name, "skip", "уже актуально");
            continue;
        }

        let added = keys.iter().filter(|key| !previous.contains(key)).count();
        current.dependencies = merged;
        current.updated_at = now_iso();
        report.updated += 1;
        report.added_links += added as u32;
        emit_prefetch_progress(
            app,
            "dependencies",
            step,
            total,
            &item.display_name,
            "ok",
            &format!("+{added}"),
        );
    }

    if report.updated > 0 {
        tags.updated_at = now_iso();
        write_tags(&paths.tags_path, &tags)?;
    }

    emit_prefetch_done(app, "dependencies");
    Ok(report)
}
