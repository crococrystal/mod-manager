use std::path::PathBuf;

use base64::{engine::general_purpose, Engine};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::bootstrap::{bootstrap_still_active, cancel_active_bootstrap};
use crate::catalog;
use crate::covers::{
    cover_ext_from_mime, delete_manual_cover, fetch_mod_cover, store_uploaded_cover,
};
use crate::dependencies::same_dependency_list;
use crate::instance_registry;
use crate::mods::{merge_keys, normalize_side, scan_mods_for_settings, stats_for, ModListPayload};
use crate::mods_watch;
use crate::prefetch::{identify_unknown_sources, prefetch_mod_assets_for_settings};
use crate::providers::{
    InstallProviderVersionRequest, InstallProviderVersionResult, ListProviderVersionsRequest,
    ProviderVersionsPayload, SearchProviderRequest, SwitchModSourceRequest, SwitchModSourceResult,
};
use crate::remote::{fetch_api_dependencies, http_client, ProviderCandidate};
use crate::settings::{
    read_settings, remember_instance, resolve_paths, settings_view, write_settings, Settings,
    SettingsView,
};
use crate::tags::{read_tags, write_tags};
use crate::util::{file_mtime_millis, now_iso, path_string};

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

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RefreshModAssetsRequest {
    pub key: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RefreshModAssetsResult {
    pub key: String,
    pub dependencies: Vec<String>,
    pub resolved_dependencies: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cover_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cover_modified_at: Option<u64>,
}

fn absolute_path(path: PathBuf) -> PathBuf {
    path.canonicalize().unwrap_or(path)
}

fn copy_files_to_clipboard(paths: &[PathBuf]) -> Result<(), String> {
    use clipboard_rs::{Clipboard, ClipboardContext};

    if paths.is_empty() {
        return Err("Нечего копировать.".to_string());
    }

    let file_paths: Vec<String> = paths
        .iter()
        .map(|path| absolute_path(path.clone()).to_string_lossy().into_owned())
        .collect();

    let ctx = ClipboardContext::new().map_err(|error| error.to_string())?;
    ctx.clear().map_err(|error| error.to_string())?;
    ctx.set_files(file_paths)
        .map_err(|_| "Не удалось скопировать файлы в буфер обмена.".to_string())
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
    let previous = read_settings(&app).ok();
    let instance_changed =
        previous.as_ref().map(|item| &item.instance_root) != Some(&settings.instance_root);

    if let Some(root) = settings.instance_root.clone() {
        remember_instance(&mut settings, &root);
    }
    write_settings(&app, &settings)?;
    if instance_changed {
        cancel_active_bootstrap(&app);
    }
    if let Ok(paths) = resolve_paths(&settings) {
        mods_watch::sync_mods_watch(&app, Some(paths.mods_dir));
    } else {
        mods_watch::sync_mods_watch(&app, None);
    }
    settings_view(&app, settings)
}

#[tauri::command]
pub(crate) async fn scan_mods(app: AppHandle) -> Result<ModListPayload, String> {
    let settings = read_settings(&app)?;
    let view = settings_view(&app, settings.clone())?;
    let catalog_root = catalog::catalog_root(&app).ok();
    let mods = tauri::async_runtime::spawn_blocking(move || {
        scan_mods_for_settings(&settings, catalog_root)
    })
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
pub(crate) async fn identify_mod_sources(app: AppHandle) -> Result<ModListPayload, String> {
    let settings = read_settings(&app)?;
    let paths = resolve_paths(&settings)?;
    let view = settings_view(&app, settings.clone())?;
    let catalog_root = catalog::catalog_root(&app).ok();

    tauri::async_runtime::spawn_blocking(move || {
        let mut mods = scan_mods_for_settings(&settings, catalog_root.clone())?;
        let Some(client) = http_client() else {
            return Err("Не удалось создать HTTP-клиент.".to_string());
        };
        let mut tags = read_tags(&paths.tags_path)?;
        if identify_unknown_sources(&settings, &client, &paths, &mut tags, &mods)? {
            write_tags(&paths.tags_path, &tags)?;
            mods = scan_mods_for_settings(&settings, catalog_root)?;
        }
        let stats = stats_for(&mods);
        Ok(ModListPayload {
            settings: view,
            mods,
            stats,
        })
    })
    .await
    .map_err(|error| format!("Определение поставщиков прервано: {error}"))?
}

#[tauri::command]
pub(crate) async fn refresh_mod_assets(
    app: AppHandle,
    request: RefreshModAssetsRequest,
) -> Result<RefreshModAssetsResult, String> {
    let settings = read_settings(&app)?;
    let paths = resolve_paths(&settings)?;
    let catalog_root = catalog::catalog_root(&app).ok();

    tauri::async_runtime::spawn_blocking(move || {
        let Some(client) = http_client() else {
            return Err("Не удалось создать HTTP-клиент.".to_string());
        };

        let mods = scan_mods_for_settings(&settings, catalog_root.clone())?;
        let Some(item) = mods.iter().find(|entry| entry.key == request.key).cloned() else {
            return Err("Мод не найден.".to_string());
        };

        let modrinth_lookup = mods
            .iter()
            .filter_map(|entry| {
                entry
                    .modrinth_id
                    .as_ref()
                    .map(|id| (id.clone(), entry.key.clone()))
            })
            .collect();
        let curseforge_lookup = mods
            .iter()
            .filter_map(|entry| {
                entry
                    .curseforge_id
                    .as_ref()
                    .map(|id| (id.clone(), entry.key.clone()))
            })
            .collect();

        let mut refreshed_settings = settings.clone();
        refreshed_settings.auto_prefetch_dependencies = true;

        let keys = fetch_api_dependencies(
            &item,
            &client,
            &refreshed_settings,
            &modrinth_lookup,
            &curseforge_lookup,
        );
        let mut tags = read_tags(&paths.tags_path)?;
        let current = tags.mods.entry(item.key.clone()).or_default();
        let previous = current.dependencies.clone();
        let dependencies = merge_keys(&[&previous, &keys]);
        if !same_dependency_list(&previous, &dependencies) {
            current.dependencies = dependencies.clone();
            current.updated_at = now_iso();
            tags.updated_at = now_iso();
            write_tags(&paths.tags_path, &tags)?;
        }

        let (cover_path, cover_modified_at) =
            if !item.cover_manual && (item.modrinth_id.is_some() || item.curseforge_id.is_some()) {
                if let Some(path) = fetch_mod_cover(
                    &client,
                    &paths,
                    catalog_root.as_deref(),
                    &item,
                    &settings,
                    true,
                ) {
                    let mtime = file_mtime_millis(&path);
                    (Some(path_string(path)), mtime)
                } else {
                    (None, None)
                }
            } else {
                (None, None)
            };

        let resolved_dependencies = merge_keys(&[&dependencies, &item.jar_dependencies]);

        Ok(RefreshModAssetsResult {
            key: item.key,
            resolved_dependencies,
            dependencies,
            cover_path,
            cover_modified_at,
        })
    })
    .await
    .map_err(|error| format!("Обновление данных мода прервано: {error}"))?
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
    let bootstrap_token = {
        let state = app.state::<crate::bootstrap::BootstrapState>();
        state.cancel_active();
        state.snapshot()
    };

    let result = tauri::async_runtime::spawn_blocking(
        move || -> Result<instance_registry::BootstrapResult, String> {
            if !bootstrap_still_active(&app_handle, bootstrap_token) {
                return Err("Прервано: выбрана другая сборка.".to_string());
            }

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

            let mut covers_prepared = !plan_covers || !settings.auto_prefetch_covers;
            let mut dependencies_prepared =
                !plan_dependencies || !settings.auto_prefetch_dependencies;

            if run_covers || run_dependencies {
                prefetch_mod_assets_for_settings(
                    &settings,
                    &app_handle,
                    run_covers,
                    run_dependencies,
                    bootstrap_token,
                )?;
                if run_covers {
                    covers_prepared = true;
                }
                if run_dependencies {
                    dependencies_prepared = true;
                }
            }

            instance_registry::mark_prepared(
                &mut registry,
                &paths.instance_root,
                &fingerprint,
                covers_prepared,
                dependencies_prepared,
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
    let mods = tauri::async_runtime::spawn_blocking(move || {
        scan_mods_for_settings(&settings, catalog_root)
    })
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
pub(crate) async fn search_provider_candidates(
    app: AppHandle,
    request: SearchProviderRequest,
) -> Result<Vec<ProviderCandidate>, String> {
    crate::providers::search_candidates(app, request).await
}

#[tauri::command]
pub(crate) async fn lookup_provider_fingerprint(
    app: AppHandle,
    request: SearchProviderRequest,
) -> Result<Option<ProviderCandidate>, String> {
    crate::providers::lookup_fingerprint(app, request).await
}

#[tauri::command]
pub(crate) async fn switch_mod_source(
    app: AppHandle,
    request: SwitchModSourceRequest,
) -> Result<SwitchModSourceResult, String> {
    crate::providers::switch_source(app, request).await
}

#[tauri::command]
pub(crate) async fn list_provider_versions(
    app: AppHandle,
    request: ListProviderVersionsRequest,
) -> Result<ProviderVersionsPayload, String> {
    crate::providers::list_versions(app, request).await
}

#[tauri::command]
pub(crate) async fn install_provider_version(
    app: AppHandle,
    request: InstallProviderVersionRequest,
) -> Result<InstallProviderVersionResult, String> {
    crate::providers::install_version(app, request).await
}

#[tauri::command]
pub(crate) async fn copy_mod_files(app: AppHandle, keys: Vec<String>) -> Result<u32, String> {
    let app_handle = app.clone();
    tauri::async_runtime::spawn_blocking(move || -> Result<u32, String> {
        if keys.is_empty() {
            return Err("Нечего копировать.".to_string());
        }

        let settings = read_settings(&app_handle)?;
        let paths = resolve_paths(&settings)?;
        let catalog_root = catalog::catalog_root(&app_handle).ok();
        let mods = scan_mods_for_settings(&settings, catalog_root)?;
        let mut file_paths = Vec::new();

        for key in keys {
            let Some(item) = mods.iter().find(|mod_entry| mod_entry.key == key) else {
                continue;
            };
            let path = absolute_path(paths.mods_dir.join(&item.filename));
            if path.is_file() {
                file_paths.push(path);
            }
        }

        if file_paths.is_empty() {
            return Err("Не найдено файлов для копирования.".to_string());
        }

        copy_files_to_clipboard(&file_paths)?;
        Ok(file_paths.len() as u32)
    })
    .await
    .map_err(|error| format!("Копирование прервано: {error}"))?
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
    let mods = tauri::async_runtime::spawn_blocking(move || {
        scan_mods_for_settings(&settings, catalog_root)
    })
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
pub(crate) async fn delete_custom_cover(
    app: AppHandle,
    key: String,
) -> Result<ModListPayload, String> {
    let settings = read_settings(&app)?;
    let paths = resolve_paths(&settings)?;
    delete_manual_cover(&paths, &key)?;

    let catalog_root = catalog::catalog_root(&app).ok();
    let view = settings_view(&app, settings.clone())?;
    let mods = tauri::async_runtime::spawn_blocking(move || {
        scan_mods_for_settings(&settings, catalog_root)
    })
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
pub(crate) fn clear_app_data(app: AppHandle) -> Result<instance_registry::ClearDataResult, String> {
    let settings = read_settings(&app)?;
    let mut data_roots = Vec::new();
    if let Ok(paths) = resolve_paths(&settings) {
        data_roots.push(paths.data_root);
    }
    instance_registry::clear_all(&app, data_roots)
}
