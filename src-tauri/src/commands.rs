use base64::{engine::general_purpose, Engine};
use serde::Deserialize;
use tauri::AppHandle;

use crate::catalog;
use crate::covers::{cover_ext_from_mime, prefetch_covers_for_settings, store_uploaded_cover};
use crate::dependencies::prefetch_dependencies_for_settings;
use crate::instance_registry;
use crate::mods::{normalize_side, scan_mods_for_settings, stats_for, ModListPayload};
use crate::settings::{
    read_settings, remember_instance, resolve_paths, settings_view, write_settings, Settings,
    SettingsView,
};
use crate::tags::{read_tags, write_tags};
use crate::util::now_iso;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UpdateModTagsRequest {
    pub key: String,
    pub side: Option<String>,
    pub library: Option<bool>,
    pub technical: Option<bool>,
    pub description: Option<String>,
    pub dependencies: Option<Vec<String>>,
}

#[tauri::command]
pub(crate) fn get_settings(app: AppHandle) -> Result<SettingsView, String> {
    let settings = read_settings(&app)?;
    settings_view(&app, settings)
}

#[tauri::command]
pub(crate) fn save_settings(
    app: AppHandle,
    mut settings: Settings,
) -> Result<SettingsView, String> {
    if let Some(root) = settings.instance_root.clone() {
        remember_instance(&mut settings, &root);
    }
    write_settings(&app, &settings)?;
    settings_view(&app, settings)
}

#[tauri::command]
pub(crate) async fn scan_mods(app: AppHandle) -> Result<ModListPayload, String> {
    let settings = read_settings(&app)?;
    let view = settings_view(&app, settings.clone())?;
    let catalog_root = catalog::catalog_root(&app).ok();
    let mods =
        tauri::async_runtime::spawn_blocking(move || scan_mods_for_settings(&settings, catalog_root))
            .await
            .map_err(|error| format!("Сканирование прервано: {error}"))??;
    let stats = stats_for(&mods);
    Ok(ModListPayload {
        settings: view,
        mods,
        stats,
    })
}

#[tauri::command]
pub(crate) async fn bootstrap_instance(
    app: AppHandle,
    force: Option<bool>,
) -> Result<instance_registry::BootstrapResult, String> {
    let settings = read_settings(&app)?;
    let paths = resolve_paths(&settings)?;
    let fingerprint = instance_registry::mods_fingerprint(&paths.mods_dir)?;
    let force = force.unwrap_or(false);
    let app_handle = app.clone();

    let result = tauri::async_runtime::spawn_blocking(
        move || -> Result<instance_registry::BootstrapResult, String> {
            let mut registry = instance_registry::read_registry(&app_handle)?;
            let now = now_iso();
            instance_registry::touch_opened(&mut registry, &paths.instance_root, &now);

            let (plan_covers, plan_dependencies) = instance_registry::plan_bootstrap(
                &registry,
                &paths.instance_root,
                &fingerprint,
                force,
            );

            if !plan_covers && !plan_dependencies {
                instance_registry::write_registry(&app_handle, &registry)?;
                return Ok(instance_registry::BootstrapResult {
                    skipped: true,
                    ran_covers: false,
                    ran_dependencies: false,
                });
            }

            let run_covers = plan_covers && settings.auto_prefetch_covers;
            let run_dependencies = plan_dependencies && settings.auto_prefetch_dependencies;

            if run_covers {
                prefetch_covers_for_settings(&settings, &app_handle)?;
            }
            if run_dependencies {
                prefetch_dependencies_for_settings(&settings, &app_handle)?;
            }

            instance_registry::mark_prepared(
                &mut registry,
                &paths.instance_root,
                &fingerprint,
                run_covers,
                run_dependencies,
                &now,
            );
            instance_registry::write_registry(&app_handle, &registry)?;

            Ok(instance_registry::BootstrapResult {
                skipped: false,
                ran_covers: run_covers,
                ran_dependencies: run_dependencies,
            })
        },
    )
    .await
    .map_err(|error| format!("Подготовка прервана: {error}"))??;

    Ok(result)
}

#[tauri::command]
pub(crate) async fn update_mod_tags(
    app: AppHandle,
    patch: UpdateModTagsRequest,
) -> Result<ModListPayload, String> {
    let settings = read_settings(&app)?;
    let paths = resolve_paths(&settings)?;
    let mut tags = read_tags(&paths.tags_path)?;
    let current = tags.mods.entry(patch.key).or_default();

    if let Some(side) = patch.side {
        current.side = normalize_side(&side);
    }
    if let Some(library) = patch.library {
        current.library = library;
    }
    if let Some(technical) = patch.technical {
        current.technical = technical;
    }
    if let Some(description) = patch.description {
        current.description = description;
    }
    if let Some(dependencies) = patch.dependencies {
        current.dependencies = dependencies;
    }
    current.updated_at = now_iso();
    tags.updated_at = now_iso();
    write_tags(&paths.tags_path, &tags)?;

    let catalog_root = catalog::catalog_root(&app).ok();
    let view = settings_view(&app, settings.clone())?;
    let mods =
        tauri::async_runtime::spawn_blocking(move || scan_mods_for_settings(&settings, catalog_root))
            .await
            .map_err(|error| format!("Сканирование прервано: {error}"))??;
    let stats = stats_for(&mods);
    Ok(ModListPayload {
        settings: view,
        mods,
        stats,
    })
}

#[tauri::command]
pub(crate) async fn upload_cover(
    app: AppHandle,
    key: String,
    data_url: String,
) -> Result<ModListPayload, String> {
    let settings = read_settings(&app)?;
    let paths = resolve_paths(&settings)?;
    let (meta, encoded) = data_url
        .split_once(',')
        .ok_or_else(|| "Не получилось прочитать файл обложки.".to_string())?;
    let mime = meta
        .strip_prefix("data:")
        .unwrap_or(meta)
        .split(';')
        .next()
        .unwrap_or("");
    let ext = cover_ext_from_mime(mime)
        .ok_or_else(|| "Поддерживаются PNG, JPG, WebP и GIF.".to_string())?;
    let bytes = general_purpose::STANDARD
        .decode(encoded)
        .map_err(|_| "Не получилось декодировать обложку.".to_string())?;
    if bytes.is_empty() {
        return Err("Файл обложки пустой.".to_string());
    }

    store_uploaded_cover(&paths, &key, &bytes, ext)?;

    let catalog_root = catalog::catalog_root(&app).ok();
    let view = settings_view(&app, settings.clone())?;
    let mods =
        tauri::async_runtime::spawn_blocking(move || scan_mods_for_settings(&settings, catalog_root))
            .await
            .map_err(|error| format!("Сканирование прервано: {error}"))??;
    let stats = stats_for(&mods);
    Ok(ModListPayload {
        settings: view,
        mods,
        stats,
    })
}

#[tauri::command]
pub(crate) fn clear_app_data(
    app: AppHandle,
) -> Result<instance_registry::ClearDataResult, String> {
    let settings = read_settings(&app)?;
    let mut data_roots = Vec::new();
    if let Ok(paths) = resolve_paths(&settings) {
        data_roots.push(paths.data_root);
    }
    instance_registry::clear_all(&app, data_roots)
}
