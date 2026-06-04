use std::{collections::HashSet, fs, path::PathBuf, time::Duration};

use base64::{engine::general_purpose, Engine};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::bootstrap::{bootstrap_still_active, cancel_active_bootstrap, ensure_task_active};
use crate::catalog;
use crate::covers::{
    cover_dir, cover_ext_from_mime, delete_manual_cover, fetch_mod_cover, remove_cover_variants,
    resolve_cover_state, store_uploaded_cover,
};
use crate::dependencies::{
    filter_reverse_jar_dependency_keys, jar_dependencies_by_key, same_dependency_list,
};
use crate::instance_registry;
use crate::mods::{merge_keys, normalize_side, scan_mods_for_settings, stats_for, ModListPayload};
use crate::mods_watch;
use crate::prefetch::{identify_unknown_sources, sync_mods_unified, SyncFlags};
use crate::provider_labels::{
    fetch_and_store_provider_labels, manual_tags_for, provider_tags_for,
    refresh_provider_labels_bulk, refresh_result_for, RefreshProviderLabelsResult,
};
use crate::providers::{
    CatalogInstallPreview, CatalogInstallPreviewRequest, CatalogInstallRequest,
    CatalogInstallResult, CatalogProjectDetails, CatalogProjectDetailsRequest,
    CatalogSearchRequest, CatalogSearchResponse, InstallProviderVersionRequest,
    InstallProviderVersionResult, ListProviderVersionsRequest, ProviderVersionsPayload,
    SearchProviderRequest, SwitchModSourceRequest, SwitchModSourceResult,
};
use crate::remote::ProviderCandidate;
use crate::remote::{fetch_api_dependencies, http_client};
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
    pub side_mode: Option<String>,
    pub library: Option<bool>,
    pub technical: Option<bool>,
    pub description: Option<String>,
    pub dependencies: Option<Vec<String>>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RefreshProviderLabelsRequest {
    pub key: String,
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

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DeleteModFilesResult {
    pub removed: u32,
    pub filenames: Vec<String>,
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
        mods_watch::sync_mods_watch(&app, paths.all_mods_dirs().map(PathBuf::from).collect());
    } else {
        mods_watch::sync_mods_watch(&app, Vec::new());
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
        labels_synced: None,
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
            mods = scan_mods_for_settings(&settings, catalog_root.clone())?;
        }
        let labels_synced = refresh_provider_labels_bulk(&settings, &mut tags, &mods, true)?;
        if labels_synced > 0 {
            write_tags(&paths.tags_path, &tags)?;
            mods = scan_mods_for_settings(&settings, catalog_root)?;
        }
        let stats = stats_for(&mods);
        Ok(ModListPayload {
            settings: view,
            mods,
            stats,
            labels_synced: if labels_synced > 0 {
                Some(labels_synced)
            } else {
                None
            },
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
        let jar_dependencies = jar_dependencies_by_key(&mods);

        let mut refreshed_settings = settings.clone();
        refreshed_settings.auto_prefetch_dependencies = true;

        let keys = fetch_api_dependencies(
            &item,
            &client,
            &refreshed_settings,
            &modrinth_lookup,
            &curseforge_lookup,
        );
        let keys = filter_reverse_jar_dependency_keys(
            &item.key,
            &item.jar_dependencies,
            &keys,
            &jar_dependencies,
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
    let fingerprint = instance_registry::mods_fingerprint_dirs(paths.all_mods_dirs())?;
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
                // Метки тянем заодно при первом открытии сборки —
                // batch-эндпоинты бесплатные, а пользователь сразу видит side / library / tech.
                // only_missing_labels гарантирует, что повторные открытия не перезаписывают
                // уже скачанные метки.
                let flags = SyncFlags {
                    labels: true,
                    covers: run_covers,
                    dependencies: run_dependencies,
                    force_covers: false,
                    force_labels: false,
                    only_missing_labels: true,
                };
                sync_mods_unified(&settings, &app_handle, flags, Some(bootstrap_token))?;
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
pub(crate) fn cancel_background_task(app: AppHandle) {
    cancel_active_bootstrap(&app);
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ModTagsUpdateResult {
    pub key: String,
    pub side: String,
    pub library: bool,
    pub technical: bool,
    pub side_mode: String,
    pub manual_side: String,
    pub manual_library: bool,
    pub manual_technical: bool,
    pub provider_side: String,
    pub provider_library: bool,
    pub provider_technical: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload: Option<ModListPayload>,
}

fn mod_tags_update_result(tag: &crate::tags::ModTags, key: &str) -> ModTagsUpdateResult {
    let resolved = refresh_result_for(tag, key);
    let (manual_side, manual_library, manual_technical) = manual_tags_for(tag);
    let (provider_side, provider_library, provider_technical) = provider_tags_for(tag);
    ModTagsUpdateResult {
        key: key.to_string(),
        side: resolved.side,
        library: resolved.library,
        technical: resolved.technical,
        side_mode: resolved.side_mode,
        manual_side,
        manual_library,
        manual_technical,
        provider_side,
        provider_library,
        provider_technical,
        description: None,
        payload: None,
    }
}

#[tauri::command]
pub(crate) async fn update_mod_tags(
    app: AppHandle,
    patch: UpdateModTagsRequest,
) -> Result<ModTagsUpdateResult, String> {
    let settings = read_settings(&app)?;
    let paths = resolve_paths(&settings)?;
    let mut tags = read_tags(&paths.tags_path)?;
    let current = tags.mods.entry(patch.key.clone()).or_default();

    if let Some(side_mode) = patch.side_mode {
        current.label_overrides.side_mode = if side_mode.trim() == "manual" {
            "manual".to_string()
        } else {
            "auto".to_string()
        };
    }
    if let Some(side) = patch.side {
        current.side = normalize_side(&side);
        current.label_overrides.side_mode = "manual".to_string();
    }
    if let Some(library) = patch.library {
        current.library = library;
        current.label_overrides.side_mode = "manual".to_string();
    }
    if let Some(technical) = patch.technical {
        current.technical = technical;
        current.label_overrides.side_mode = "manual".to_string();
    }
    let description_changed = patch.description.is_some();
    if let Some(description) = patch.description {
        current.description = description;
    }
    let dependencies_changed = patch.dependencies.is_some();
    if let Some(dependencies) = patch.dependencies {
        current.dependencies = dependencies;
    }
    current.updated_at = now_iso();
    tags.updated_at = now_iso();
    write_tags(&paths.tags_path, &tags)?;

    let tag = tags.mods.get(&patch.key).cloned().unwrap_or_default();
    let mut result = mod_tags_update_result(&tag, &patch.key);
    if description_changed {
        result.description = Some(tag.description);
    }

    if dependencies_changed {
        let catalog_root = catalog::catalog_root(&app).ok();
        let view = settings_view(&app, settings.clone())?;
        let mods = tauri::async_runtime::spawn_blocking(move || {
            scan_mods_for_settings(&settings, catalog_root)
        })
        .await
        .map_err(|error| format!("Сканирование прервано: {error}"))??;
        let stats = stats_for(&mods);
        result.payload = Some(ModListPayload {
            settings: view,
            mods,
            stats,
            labels_synced: None,
        });
    }

    Ok(result)
}

#[tauri::command]
pub(crate) async fn refresh_provider_labels(
    app: AppHandle,
    request: RefreshProviderLabelsRequest,
) -> Result<RefreshProviderLabelsResult, String> {
    let settings = read_settings(&app)?;
    let paths = resolve_paths(&settings)?;
    let catalog_root = catalog::catalog_root(&app).ok();

    tauri::async_runtime::spawn_blocking(move || {
        let mods = scan_mods_for_settings(&settings, catalog_root)?;
        let Some(item) = mods.iter().find(|entry| entry.key == request.key).cloned() else {
            return Err("Мод не найден.".to_string());
        };
        let mut tags = read_tags(&paths.tags_path)?;
        let tag = tags.mods.entry(request.key.clone()).or_default();
        fetch_and_store_provider_labels(tag, &item, &settings)?;
        tag.updated_at = now_iso();
        tags.updated_at = now_iso();
        write_tags(&paths.tags_path, &tags)?;
        let tag = tags.mods.get(&request.key).cloned().unwrap_or_default();
        Ok(refresh_result_for(&tag, &request.key))
    })
    .await
    .map_err(|error| format!("Обновление меток прервано: {error}"))?
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
pub(crate) async fn search_provider_catalog(
    app: AppHandle,
    request: CatalogSearchRequest,
) -> Result<CatalogSearchResponse, String> {
    crate::providers::search_catalog(app, request).await
}

#[tauri::command]
pub(crate) async fn preview_catalog_install(
    app: AppHandle,
    request: CatalogInstallPreviewRequest,
) -> Result<CatalogInstallPreview, String> {
    crate::providers::preview_catalog_install(app, request).await
}

#[tauri::command]
pub(crate) async fn catalog_project_details(
    app: AppHandle,
    request: CatalogProjectDetailsRequest,
) -> Result<CatalogProjectDetails, String> {
    crate::providers::catalog_project_details(app, request).await
}

#[tauri::command]
pub(crate) async fn install_from_catalog(
    app: AppHandle,
    request: CatalogInstallRequest,
) -> Result<CatalogInstallResult, String> {
    crate::providers::install_from_catalog(app, request).await
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
            let path = paths
                .resolve_mod_jar(&item.filename)
                .map(|jar_path| absolute_path(jar_path))
                .unwrap_or_default();
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
pub(crate) async fn delete_mod_files(
    app: AppHandle,
    keys: Vec<String>,
) -> Result<DeleteModFilesResult, String> {
    let app_handle = app.clone();
    tauri::async_runtime::spawn_blocking(move || -> Result<DeleteModFilesResult, String> {
        let key_set: HashSet<String> = keys.into_iter().filter(|key| !key.is_empty()).collect();
        if key_set.is_empty() {
            return Err("Нечего удалять.".to_string());
        }

        let settings = read_settings(&app_handle)?;
        let paths = resolve_paths(&settings)?;
        let catalog_root = catalog::catalog_root(&app_handle).ok();
        let mods = scan_mods_for_settings(&settings, catalog_root)?;
        let mut tags = read_tags(&paths.tags_path)?;
        let cache_cover_dir = cover_dir(&paths.data_root, false);
        let manual_cover_dir = cover_dir(&paths.data_root, true);
        let mut filenames = Vec::new();

        mods_watch::suppress_events_for(Duration::from_secs(4));

        for key in &key_set {
            let Some(item) = mods.iter().find(|entry| entry.key.as_str() == key.as_str()) else {
                continue;
            };
            let blockers: Vec<String> = item
                .used_by
                .iter()
                .filter(|used_by| !key_set.contains(*used_by))
                .map(|used_by| {
                    mods.iter()
                        .find(|entry| entry.key == *used_by)
                        .map(|entry| entry.display_name.clone())
                        .unwrap_or_else(|| used_by.clone())
                })
                .collect();
            if !blockers.is_empty() {
                return Err(format!(
                    "Нельзя удалить {}: используется для {}.",
                    item.display_name,
                    blockers.join(", ")
                ));
            }
        }

        for key in &key_set {
            let Some(item) = mods.iter().find(|entry| entry.key.as_str() == key.as_str()) else {
                continue;
            };
            let Some(path) = paths.resolve_mod_jar(&item.filename) else {
                continue;
            };
            fs::remove_file(&path)
                .map_err(|error| format!("Не удалось удалить {}: {error}", item.filename))?;
            if let Some(index_file) = &item.index_file {
                let index_path = paths.index_dir.join(index_file);
                if index_path.is_file() {
                    fs::remove_file(&index_path).map_err(|error| {
                        format!("Не удалось удалить индекс {}: {error}", index_file)
                    })?;
                }
            }
            filenames.push(item.filename.clone());
            tags.mods.remove(&item.key);
            remove_cover_variants(&cache_cover_dir, &item.key);
            remove_cover_variants(&manual_cover_dir, &item.key);
        }

        if filenames.is_empty() {
            return Err("Не найдено файлов для удаления.".to_string());
        }

        tags.updated_at = now_iso();
        write_tags(&paths.tags_path, &tags)?;

        Ok(DeleteModFilesResult {
            removed: filenames.len() as u32,
            filenames,
        })
    })
    .await
    .map_err(|error| format!("Удаление модов прервано: {error}"))?
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CoverUpdateResult {
    pub key: String,
    pub cover_path: Option<String>,
    pub cover_modified_at: Option<u64>,
    pub cover_manual: bool,
}

#[tauri::command]
pub(crate) fn upload_cover(
    app: AppHandle,
    key: String,
    data_url: String,
) -> Result<CoverUpdateResult, String> {
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

    let path = store_uploaded_cover(&paths, &key, &bytes, ext)?;
    let mtime = file_mtime_millis(&path);
    Ok(CoverUpdateResult {
        key,
        cover_path: Some(path_string(path)),
        cover_modified_at: mtime,
        cover_manual: true,
    })
}

#[tauri::command]
pub(crate) fn delete_custom_cover(
    app: AppHandle,
    key: String,
) -> Result<CoverUpdateResult, String> {
    let settings = read_settings(&app)?;
    let paths = resolve_paths(&settings)?;
    delete_manual_cover(&paths, &key)?;

    let tags = read_tags(&paths.tags_path)?;
    let tag = tags.mods.get(&key);
    let catalog_root = catalog::catalog_root(&app).ok();
    let (cover_path, cover_modified_at, cover_manual) = resolve_cover_state(
        &paths,
        catalog_root.as_deref(),
        &key,
        tag.map(|value| value.modrinth_id.as_str()),
        tag.map(|value| value.curseforge_id.as_str()),
    );
    Ok(CoverUpdateResult {
        key,
        cover_path,
        cover_modified_at,
        cover_manual,
    })
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SyncProviderDataRequest {
    #[serde(default)]
    pub identify: bool,
    #[serde(default)]
    pub labels: bool,
    #[serde(default)]
    pub assets: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SyncProviderDataResult {
    pub linked: u32,
    pub labels_refreshed: u32,
    pub assets_refreshed: u32,
    pub payload: ModListPayload,
}

#[tauri::command]
pub(crate) async fn sync_provider_data(
    app: AppHandle,
    request: SyncProviderDataRequest,
) -> Result<SyncProviderDataResult, String> {
    if !request.identify && !request.labels && !request.assets {
        return Err("Выбери хотя бы одну категорию.".to_string());
    }

    let settings = read_settings(&app)?;
    let paths = resolve_paths(&settings)?;
    let view = settings_view(&app, settings.clone())?;
    let catalog_root = catalog::catalog_root(&app).ok();
    let app_handle = app.clone();
    let task_token = {
        let state = app.state::<crate::bootstrap::BootstrapState>();
        state.cancel_active();
        state.snapshot()
    };

    tauri::async_runtime::spawn_blocking(move || -> Result<SyncProviderDataResult, String> {
        let Some(client) = http_client() else {
            return Err("Не удалось создать HTTP-клиент.".to_string());
        };

        ensure_task_active(&app_handle, task_token)?;

        let mut linked: u32 = 0;
        if request.identify {
            let mods = scan_mods_for_settings(&settings, catalog_root.clone())?;
            let mut tags = read_tags(&paths.tags_path)?;
            let before = mods
                .iter()
                .filter(|item| item.source == "modrinth" || item.source == "curseforge")
                .count();
            if identify_unknown_sources(&settings, &client, &paths, &mut tags, &mods)? {
                write_tags(&paths.tags_path, &tags)?;
                let after_mods = scan_mods_for_settings(&settings, catalog_root.clone())?;
                let after = after_mods
                    .iter()
                    .filter(|item| item.source == "modrinth" || item.source == "curseforge")
                    .count();
                linked = after.saturating_sub(before) as u32;
            }
            ensure_task_active(&app_handle, task_token)?;
        }

        let mut labels_refreshed: u32 = 0;
        let mut assets_refreshed: u32 = 0;
        if request.labels || request.assets {
            ensure_task_active(&app_handle, task_token)?;
            let flags = SyncFlags {
                labels: request.labels,
                covers: request.assets,
                dependencies: request.assets,
                force_covers: request.assets,
                force_labels: request.labels,
                only_missing_labels: false,
            };
            let unified = sync_mods_unified(&settings, &app_handle, flags, Some(task_token))?;
            labels_refreshed = unified.labels_refreshed;
            assets_refreshed = unified.covers_downloaded + unified.dependencies_updated;
            ensure_task_active(&app_handle, task_token)?;
        }

        let mods = scan_mods_for_settings(&settings, catalog_root)?;
        let stats = stats_for(&mods);
        let payload = ModListPayload {
            settings: view,
            mods,
            stats,
            labels_synced: if labels_refreshed > 0 {
                Some(labels_refreshed as usize)
            } else {
                None
            },
        };
        Ok(SyncProviderDataResult {
            linked,
            labels_refreshed,
            assets_refreshed,
            payload,
        })
    })
    .await
    .map_err(|error| format!("Синхронизация прервана: {error}"))?
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DataUsageResult {
    pub total: u64,
    pub covers_cache: u64,
    pub covers_manual: u64,
    pub tags_file: u64,
    pub other_cache: u64,
}

fn dir_size_bytes(path: &std::path::Path) -> u64 {
    if !path.exists() {
        return 0;
    }
    let mut total: u64 = 0;
    let mut stack = vec![path.to_path_buf()];
    while let Some(current) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&current) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(meta) = entry.metadata() else {
                continue;
            };
            if meta.is_dir() {
                stack.push(entry.path());
            } else {
                total = total.saturating_add(meta.len());
            }
        }
    }
    total
}

#[tauri::command]
pub(crate) fn get_data_usage(app: AppHandle) -> Result<DataUsageResult, String> {
    let settings = read_settings(&app)?;
    let paths = resolve_paths(&settings)?;
    let data_root = &paths.data_root;

    let mut covers_cache = dir_size_bytes(&data_root.join("covers").join("cache"));
    if let Ok(app_data) = app.path().app_data_dir() {
        covers_cache =
            covers_cache.saturating_add(dir_size_bytes(&app_data.join("catalog").join("covers")));
    }
    let covers_manual = dir_size_bytes(&data_root.join("covers").join("manual"));
    let tags_file = std::fs::metadata(&paths.tags_path)
        .map(|meta| meta.len())
        .unwrap_or(0);
    let other_cache = dir_size_bytes(&data_root.join("cache"));
    let total = covers_cache + covers_manual + tags_file + other_cache;

    Ok(DataUsageResult {
        total,
        covers_cache,
        covers_manual,
        tags_file,
        other_cache,
    })
}

#[tauri::command]
pub(crate) fn clear_app_data(app: AppHandle) -> Result<instance_registry::ClearDataResult, String> {
    let settings = read_settings(&app)?;
    let mut data_roots = Vec::new();

    if let Some(instance_root) = settings.instance_root.as_deref() {
        data_roots.push(std::path::PathBuf::from(instance_root).join(".mod-manager"));
    }

    for instance_root in &settings.recent_instances {
        data_roots.push(std::path::PathBuf::from(instance_root).join(".mod-manager"));
    }

    instance_registry::clear_all(&app, data_roots)
}

#[tauri::command]
pub(crate) fn refresh_window_shadow(window: tauri::WebviewWindow) {
    crate::window_chrome::invalidate(&window);
}
